struct VertexInput {
    @builtin(vertex_index) index: u32,
}

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
}

@vertex
fn vs_main(vertex: VertexInput) -> VertexOutput {
    let x = f32((vertex.index << 1u) & 2u);
    let y = f32(vertex.index & 2u);
    return VertexOutput(-1.0 + vec4(x, y, 0.5, 1.0) * 2.0);
}

struct SkyUniform {
    light_intensity: vec3<f32>,
    sun_intensity: f32,
    color: vec3<f32>,
}

@group(0) @binding(0)
var<uniform> sky: SkyUniform;

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    return vec4(sky.color, 1.0);
}
