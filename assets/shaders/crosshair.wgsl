struct VertexInput {
    @location(0) coords: vec2<f32>,
}

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) tex_coords: vec2<f32>,
}

@vertex
fn vs_main(vertex: VertexInput) -> VertexOutput {
    return VertexOutput(vec4(vertex.coords, 0.0, 1.0), (1.0 + vertex.coords) * 0.5);
}

@group(0) @binding(0)
var t_output: texture_2d<f32>;

@group(0) @binding(1)
var s_output: sampler;

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    return 1.0 - textureSample(t_output, s_output, in.tex_coords);
}
