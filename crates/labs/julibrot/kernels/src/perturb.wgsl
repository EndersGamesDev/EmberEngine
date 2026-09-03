struct PerturbUniform {
    basis_u: vec4<f32>,
    basis_v: vec4<f32>,
    screen_to_plane_row_0: vec4<f32>,
    screen_to_plane_row_1: vec4<f32>,
    screen_to_plane_row_2: vec4<f32>,
    pixel_scale: f32,
    width: u32,
    height: u32,
    max_iter: u32,
    bailout: f32,
    orbit_length: u32,
    level: u32,
    scale_exponent: i32,
}

struct PerturbResult {
    escape: vec4<f32>,
}

struct PerturbMapResult {
    offset: vec2<f32>,
    status: f32,
    sampleable: bool,
}

fn perturb_map_finite(value: f32) -> bool {
    return abs(value) <= 3.402823e38;
}

fn perturb_map(x: f32, y: f32, uniforms: PerturbUniform) -> PerturbMapResult {
    let numerator_u = uniforms.screen_to_plane_row_0.x * x + uniforms.screen_to_plane_row_0.y * y + uniforms.screen_to_plane_row_0.z;
    let numerator_v = uniforms.screen_to_plane_row_1.x * x + uniforms.screen_to_plane_row_1.y * y + uniforms.screen_to_plane_row_1.z;
    let denominator = uniforms.screen_to_plane_row_2.x * x + uniforms.screen_to_plane_row_2.y * y + uniforms.screen_to_plane_row_2.z;
    var result: PerturbMapResult;
    result.offset = vec2<f32>(0.0);
    result.status = 3.0;
    result.sampleable = false;
    if (!perturb_map_finite(denominator)) {
        return result;
    }
    if (denominator <= 0.0) {
        result.status = 2.0;
        return result;
    }
    let error_factor = 0.000000476837158203125;
    let scale_u = abs(uniforms.screen_to_plane_row_0.x) * abs(x) + abs(uniforms.screen_to_plane_row_0.y) * abs(y) + abs(uniforms.screen_to_plane_row_0.z);
    let scale_v = abs(uniforms.screen_to_plane_row_1.x) * abs(x) + abs(uniforms.screen_to_plane_row_1.y) * abs(y) + abs(uniforms.screen_to_plane_row_1.z);
    let scale_w = abs(uniforms.screen_to_plane_row_2.x) * abs(x) + abs(uniforms.screen_to_plane_row_2.y) * abs(y) + abs(uniforms.screen_to_plane_row_2.z);
    let error_u = error_factor * scale_u;
    let error_v = error_factor * scale_v;
    let error_w = error_factor * scale_w;
    let mapped = vec2<f32>(numerator_u / denominator, numerator_v / denominator);
    if (!perturb_map_finite(mapped.x) || !perturb_map_finite(mapped.y)) {
        return result;
    }
    result.offset = mapped;
    result.sampleable = true;
    if (denominator <= error_w) {
        return result;
    }
    let safe_denominator = denominator - error_w;
    let quotient_error = vec2<f32>((error_u + abs(mapped.x) * error_w) / safe_denominator, (error_v + abs(mapped.y) * error_w) / safe_denominator);
    if (quotient_error.x * quotient_error.x + quotient_error.y * quotient_error.y > 0.0625) {
        return result;
    }
    result.status = 0.0;
    return result;
}

struct ScaledState {
    delta: vec2<f32>,
    delta_c: vec2<f32>,
    exponent: i32,
    glitch: bool,
}

fn perturb_mul(left: vec2<f32>, right: vec2<f32>) -> vec2<f32> {
    let real = left.x * right.x - left.y * right.y;
    let imaginary = left.x * right.y + left.y * right.x;
    return vec2<f32>(real, imaginary);
}

fn perturb_ldexp(value: f32, exponent: i32) -> f32 {
    let value_bits = bitcast<u32>(value);
    if (value == 0.0 || (value_bits & 0x7f800000u) == 0x7f800000u) {
        return value;
    }
    let sign_bit = value_bits & 0x80000000u;
    if (exponent > 512i) {
        return bitcast<f32>(sign_bit | 0x7f800000u);
    }
    if (exponent < -512i) {
        return bitcast<f32>(sign_bit);
    }
    var result = value;
    var remaining = exponent;
    loop {
        if (remaining == 0i) {
            break;
        }
        let step = clamp(remaining, -126i, 127i);
        let factor = bitcast<f32>(u32(step + 127i) << 23u);
        result *= factor;
        remaining -= step;
    }
    return result;
}

fn perturb_scale(value: vec2<f32>, exponent: i32) -> vec2<f32> {
    return vec2<f32>(perturb_ldexp(value.x, exponent), perturb_ldexp(value.y, exponent));
}

fn perturb_norm(value: vec2<f32>) -> f32 {
    let scale = max(abs(value.x), abs(value.y));
    if (scale == 0.0) {
        return 0.0;
    }
    let normalized = value / scale;
    return scale * sqrt(dot(normalized, normalized));
}

fn perturb_finite(value: vec2<f32>) -> bool {
    return (bitcast<u32>(value.x) & 0x7f800000u) != 0x7f800000u
        && (bitcast<u32>(value.y) & 0x7f800000u) != 0x7f800000u;
}

fn perturb_reference(record: vec4<f32>) -> vec2<f32> {
    return record.xy;
}

fn perturb_smooth(iteration: u32, value: vec2<f32>) -> f32 {
    return f32(iteration) + 1.0 - log2(log2(perturb_norm(value)));
}

fn perturb_normalize(
    delta: vec2<f32>,
    delta_c: vec2<f32>,
    exponent: i32,
) -> ScaledState {
    var state: ScaledState;
    state.delta = delta;
    state.delta_c = delta_c;
    state.exponent = exponent;
    state.glitch = false;
    let low = perturb_ldexp(1.0, -64i);
    let high = perturb_ldexp(1.0, 64i);
    var steps = 0u;
    loop {
        let magnitude = perturb_norm(state.delta);
        if (magnitude == 0.0 || (magnitude >= low && magnitude <= high)) {
            break;
        }
        if (magnitude > high) {
            if (state.exponent > 2147483583i) {
                state.glitch = true;
                return state;
            }
            state.delta *= low;
            state.delta_c *= low;
            state.exponent += 64i;
        } else {
            if (state.exponent < -2147483584i) {
                state.glitch = true;
                return state;
            }
            state.delta *= high;
            state.delta_c *= high;
            state.exponent -= 64i;
        }
        steps += 1u;
        // Checked exponent arithmetic refuses a further step before this finite-range bound.
        if (steps > 67108863u) {
            state.glitch = true;
            return state;
        }
        if (!perturb_finite(state.delta) || !perturb_finite(state.delta_c)) {
            state.glitch = true;
            return state;
        }
    }
    return state;
}

fn perturb_glitch(rebases: u32) -> PerturbResult {
    var result: PerturbResult;
    result.escape = vec4<f32>(-1.0, 0.0, f32(rebases), 1.0);
    return result;
}

fn kernel(index: u32, uniforms: PerturbUniform) -> PerturbResult {
    let column = index % uniforms.width;
    let row = index / uniforms.width;
    let x = f32(column) + 0.5 - 0.5 * f32(uniforms.width);
    let y = f32(row) + 0.5 - 0.5 * f32(uniforms.height);
    let mapped = perturb_map(x, y, uniforms);
    if (mapped.status == 2.0) {
        var terminal: PerturbResult;
        terminal.escape = vec4<f32>(-1.0, 0.0, 0.0, mapped.status);
        return terminal;
    }
    if (!mapped.sampleable) {
        var immediate: PerturbResult;
        immediate.escape = vec4<f32>(0.0, 1.0, 0.0, 3.0);
        return immediate;
    }
    let offset_prime = (mapped.offset.x * uniforms.basis_u + mapped.offset.y * uniforms.basis_v) * uniforms.pixel_scale;
    if (!perturb_map_finite(offset_prime.x) || !perturb_map_finite(offset_prime.y) || !perturb_map_finite(offset_prime.z) || !perturb_map_finite(offset_prime.w)) {
        var immediate: PerturbResult;
        immediate.escape = vec4<f32>(0.0, 1.0, 0.0, 3.0);
        return immediate;
    }
    var delta_prime = offset_prime.xy;
    var delta_c_prime = offset_prime.zw;
    var exponent = uniforms.scale_exponent;
    var reference_index = 0u;
    var iteration = 0u;
    var rebases = 0u;
    let z_zero = perturb_reference(load_reference(0u));
    let initial = perturb_normalize(delta_prime, delta_c_prime, exponent);
    if (initial.glitch) {
        return perturb_glitch(rebases);
    }
    delta_prime = initial.delta;
    delta_c_prime = initial.delta_c;
    exponent = initial.exponent;
    loop {
        if (iteration >= uniforms.max_iter) {
            break;
        }
        if (reference_index >= uniforms.orbit_length) {
            return perturb_glitch(rebases);
        }
        let reference = perturb_reference(load_reference(reference_index));
        let represented_delta = perturb_scale(delta_prime, exponent);
        let z = reference + represented_delta;
        if (!perturb_finite(z)) {
            return perturb_glitch(rebases);
        }
        if (dot(z, z) > uniforms.bailout) {
            var escaped: PerturbResult;
            escaped.escape = vec4<f32>(perturb_smooth(iteration, z), 1.0, f32(rebases), mapped.status);
            return escaped;
        }
        if (iteration + 1u >= uniforms.max_iter) {
            break;
        }
        var advance_reference = reference;
        if (perturb_norm(z) < perturb_norm(represented_delta)) {
            if (rebases >= 16777216u) {
                return perturb_glitch(rebases);
            }
            let minimum_exponent = -2147483647i - 1i;
            if (exponent == minimum_exponent) {
                return perturb_glitch(rebases);
            }
            delta_prime = perturb_scale(z - z_zero, -exponent);
            reference_index = 0u;
            rebases += 1u;
            let converted = perturb_normalize(delta_prime, delta_c_prime, exponent);
            if (converted.glitch) {
                return perturb_glitch(rebases);
            }
            delta_prime = converted.delta;
            delta_c_prime = converted.delta_c;
            exponent = converted.exponent;
            advance_reference = z_zero;
        }
        let linear = 2.0 * perturb_mul(advance_reference, delta_prime);
        let quadratic = perturb_scale(perturb_mul(delta_prime, delta_prime), exponent);
        delta_prime = linear + quadratic + delta_c_prime;
        reference_index += 1u;
        iteration += 1u;
        let normalized = perturb_normalize(delta_prime, delta_c_prime, exponent);
        if (normalized.glitch) {
            return perturb_glitch(rebases);
        }
        delta_prime = normalized.delta;
        delta_c_prime = normalized.delta_c;
        exponent = normalized.exponent;
    }
    var capped: PerturbResult;
    capped.escape = vec4<f32>(-1.0, 0.0, f32(rebases), mapped.status);
    return capped;
}
