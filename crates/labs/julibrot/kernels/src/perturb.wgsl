struct PerturbUniform {
    basis_u: vec4<f32>,
    basis_v: vec4<f32>,
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

fn perturb_scale(value: vec2<f32>, exponent: i32) -> vec2<f32> {
    return ldexp(value, vec2<i32>(exponent));
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
    let maximum = vec2<f32>(3.402823466e38);
    return all(value == value) && all(abs(value) <= maximum);
}

fn perturb_reference(record: vec4<f32>) -> vec2<f32> {
    return vec2<f32>(record.x + record.z, record.y + record.w);
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
    let low = ldexp(1.0, -64i);
    let high = ldexp(1.0, 64i);
    for (var step = 0u; step < 4u; step += 1u) {
        let magnitude = perturb_norm(state.delta);
        if (magnitude == 0.0 || (magnitude >= low && magnitude <= high)) {
            return state;
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
        if (!perturb_finite(state.delta) || !perturb_finite(state.delta_c)) {
            state.glitch = true;
            return state;
        }
    }
    state.glitch = true;
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
    let offset_prime = (x * uniforms.basis_u + y * uniforms.basis_v) * uniforms.pixel_scale;
    var delta_prime = offset_prime.xy;
    var delta_c_prime = offset_prime.zw;
    var exponent = uniforms.scale_exponent;
    var reference_index = 0u;
    var iteration = 0u;
    var rebases = 0u;
    let z_zero = perturb_reference(load_reference(0u));
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
            escaped.escape = vec4<f32>(perturb_smooth(iteration, z), 1.0, f32(rebases), 0.0);
            return escaped;
        }
        if (iteration + 1u >= uniforms.max_iter) {
            break;
        }
        var advance_reference = reference;
        if (perturb_norm(z) < perturb_norm(represented_delta)) {
            if (rebases >= 16777215u) {
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
    capped.escape = vec4<f32>(-1.0, 0.0, f32(rebases), 0.0);
    return capped;
}
