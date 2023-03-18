struct VertexInput {
    @builtin(vertex_index) index: u32,
}

struct PushConstants {
    mvp: mat4x4<f32>,
}

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) tex_coords: vec2<f32>,
}

var<push_constant> pc: PushConstants;

@vertex
fn vs_main(vertex: VertexInput) -> VertexOutput {
    let x = f32(((vertex.index + 2u) / 3u) % 2u);
    let y = f32(((vertex.index + 1u) / 3u) % 2u);
    return VertexOutput(pc.mvp * vec4(x - 0.5, y - 0.5, 0.0, 1.0), vec2(x, 1.0 - y));
}

@group(0) @binding(0)
var t_object: texture_2d<f32>;

@group(0) @binding(1)
var s_object: sampler;

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    return textureSample(t_object, s_object, in.tex_coords);
}
