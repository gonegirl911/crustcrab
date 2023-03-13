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
    return VertexOutput(-1.0 + vec4(x, y, 0.5, 1.0) * 2.0, vec2(x, 1.0 - y));
}

@group(0) @binding(0)
var t1: texture_2d<f32>;

@group(0) @binding(1)
var s1: sampler;

@group(1) @binding(0)
var t2: texture_2d<f32>;

@group(1) @binding(1)
var s2: sampler;

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    return textureSample(t1, s1, in.input_coords) + textureSample(t2, s2, in.input_coords);
}
