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

struct Immediates {
    opacity: f32,
}

@group(0) @binding(0)
var t_input: texture_2d<f32>;

@group(0) @binding(1)
var s_input: sampler;

var<immediate> imm: Immediates;

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    return textureSample(t_input, s_input, in.input_coords) * vec4(vec3(1.0), imm.opacity);
}
