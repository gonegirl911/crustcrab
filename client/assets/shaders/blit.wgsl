struct VertexInput {
    @builtin(vertex_index) index: u32,
}

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) input_coords: vec2<f32>,
}

@vertex
fn vs_main(vertex: VertexInput) -> VertexOutput {
    let x = f32((vertex.index << 1u) & 2u);
    let y = f32(vertex.index & 2u);
    let coords = -1.0 + vec2(x, y) * 2.0;
    return VertexOutput(vec4(coords, 0.0, 1.0), vec2(x, 1.0 - y));
}

@group(0) @binding(0)
var t_input: texture_2d<f32>;

@group(0) @binding(1)
var s_input: sampler;

@fragment
fn fs_copy(in: VertexOutput) -> @location(0) vec4<f32> {
    return textureSample(t_input, s_input, in.input_coords);
}

@fragment
fn fs_encode_srgb(in: VertexOutput) -> @location(0) vec4<f32> {
    let color = textureSample(t_input, s_input, in.input_coords);
    return vec4(linear_to_srgb(color.rgb), color.a);
}

@fragment
fn fs_decode_srgb(in: VertexOutput) -> @location(0) vec4<f32> {
    let color = textureSample(t_input, s_input, in.input_coords);
    return vec4(srgb_to_linear(color.rgb), color.a);
}

fn linear_to_srgb(color: vec3<f32>) -> vec3<f32> {
    let linear_segment = color * 12.92;
    let power_segment = 1.055 * pow(color, vec3(1.0 / 2.4)) - 0.055;
    return select(linear_segment, power_segment, color > vec3(0.0031308));
}

fn srgb_to_linear(color: vec3<f32>) -> vec3<f32> {
    let linear_segment = color / 12.92;
    let power_segment = pow((color + 0.055) / 1.055, vec3(2.4));
    return select(linear_segment, power_segment, color > vec3(0.04045));
}
