struct ShallowUniform {
    basis_u: vec4<f32>,
    basis_v: vec4<f32>,
    screen_to_plane_row_0: vec4<f32>,
    screen_to_plane_row_1: vec4<f32>,
    screen_to_plane_row_2: vec4<f32>,
    centre_hi: vec4<f32>,
    centre_lo: vec4<f32>,
    pixel_scale: f32,
    width: u32,
    height: u32,
    max_iter: u32,
    bailout: f32,
    level: u32,
    padding: vec2<u32>,
}

struct ShallowResult {
    escape: vec4<f32>,
}

struct ShallowMapResult {
    offset: vec2<f32>,
    status: f32,
    sampleable: bool,
}

fn shallow_finite(value: f32) -> bool {
    return abs(value) <= 3.402823e38;
}

fn shallow_map(x: f32, y: f32, uniforms: ShallowUniform) -> ShallowMapResult {
    let numerator_u = uniforms.screen_to_plane_row_0.x * x + uniforms.screen_to_plane_row_0.y * y + uniforms.screen_to_plane_row_0.z;
    let numerator_v = uniforms.screen_to_plane_row_1.x * x + uniforms.screen_to_plane_row_1.y * y + uniforms.screen_to_plane_row_1.z;
    let denominator = uniforms.screen_to_plane_row_2.x * x + uniforms.screen_to_plane_row_2.y * y + uniforms.screen_to_plane_row_2.z;
    var result: ShallowMapResult;
    result.offset = vec2<f32>(0.0);
    result.status = 3.0;
    result.sampleable = false;
    if (!shallow_finite(denominator)) {
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
    if (!shallow_finite(mapped.x) || !shallow_finite(mapped.y)) {
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

fn shallow_square(value: vec2<f32>) -> vec2<f32> {
    let real = value.x * value.x - value.y * value.y;
    let imaginary = 2.0 * value.x * value.y;
    return vec2<f32>(real, imaginary);
}

fn shallow_log2_norm(value: vec2<f32>) -> f32 {
    let scale = max(abs(value.x), abs(value.y));
    let normalized = value / scale;
    return log2(scale) + 0.5 * log2(dot(normalized, normalized));
}

fn shallow_smooth(iteration: u32, value: vec2<f32>) -> f32 {
    return f32(iteration) + 1.0 - log2(shallow_log2_norm(value));
}

fn kernel(index: u32, uniforms: ShallowUniform) -> ShallowResult {
    let column = index % uniforms.width;
    let row = index / uniforms.width;
    let x = f32(column) + 0.5 - 0.5 * f32(uniforms.width);
    let y = f32(row) + 0.5 - 0.5 * f32(uniforms.height);
    let mapped = shallow_map(x, y, uniforms);
    if (mapped.status == 2.0) {
        var terminal: ShallowResult;
        terminal.escape = vec4<f32>(-1.0, 0.0, 0.0, mapped.status);
        return terminal;
    }
    if (!mapped.sampleable) {
        var immediate: ShallowResult;
        immediate.escape = vec4<f32>(0.0, 1.0, 0.0, 3.0);
        return immediate;
    }
    let offset = (mapped.offset.x * uniforms.basis_u + mapped.offset.y * uniforms.basis_v) * uniforms.pixel_scale;
    if (!shallow_finite(offset.x) || !shallow_finite(offset.y) || !shallow_finite(offset.z) || !shallow_finite(offset.w)) {
        var immediate: ShallowResult;
        immediate.escape = vec4<f32>(0.0, 1.0, 0.0, 3.0);
        return immediate;
    }
    let point = uniforms.centre_hi + (uniforms.centre_lo + offset);
    if (!shallow_finite(point.x) || !shallow_finite(point.y) || !shallow_finite(point.z) || !shallow_finite(point.w)) {
        var immediate: ShallowResult;
        immediate.escape = vec4<f32>(0.0, 1.0, 0.0, 3.0);
        return immediate;
    }
    var z = point.xy;
    let c = point.zw;
    var iteration = 0u;
    loop {
        if (iteration >= uniforms.max_iter) {
            break;
        }
        if (dot(z, z) > uniforms.bailout) {
            var escaped: ShallowResult;
            escaped.escape = vec4<f32>(shallow_smooth(iteration, z), 1.0, 0.0, mapped.status);
            return escaped;
        }
        if (iteration + 1u >= uniforms.max_iter) {
            break;
        }
        z = shallow_square(z) + c;
        iteration += 1u;
    }
    var capped: ShallowResult;
    capped.escape = vec4<f32>(-1.0, 0.0, 0.0, mapped.status);
    return capped;
}
