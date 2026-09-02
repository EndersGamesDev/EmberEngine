struct ShallowUniform {
    basis_u: vec4<f32>,
    basis_v: vec4<f32>,
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
    let offset = (x * uniforms.basis_u + y * uniforms.basis_v) * uniforms.pixel_scale;
    let point = uniforms.centre_hi + (uniforms.centre_lo + offset);
    var z = point.xy;
    let c = point.zw;
    var iteration = 0u;
    loop {
        if (iteration >= uniforms.max_iter) {
            break;
        }
        if (dot(z, z) > uniforms.bailout) {
            var escaped: ShallowResult;
            escaped.escape = vec4<f32>(shallow_smooth(iteration, z), 1.0, 0.0, 0.0);
            return escaped;
        }
        if (iteration + 1u >= uniforms.max_iter) {
            break;
        }
        z = shallow_square(z) + c;
        iteration += 1u;
    }
    var capped: ShallowResult;
    capped.escape = vec4<f32>(-1.0, 0.0, 0.0, 0.0);
    return capped;
}
