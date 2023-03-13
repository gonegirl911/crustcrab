struct VertexInput {
    @location(0) coords: vec3<f32>,
    @location(1) light_factor: f32,
}

struct PlayerUniform {
    vp: mat4x4<f32>,
    origin: vec3<f32>,
    render_distance: u32,
}

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) color: vec3<f32>,
}

@group(0) @binding(0)
var<uniform> player: PlayerUniform;

@vertex
fn vs_main(vertex: VertexInput) -> VertexOutput {
    return VertexOutput(
        player.vp * vec4(player.origin + vertex.coords, 1.0),
        vec3(vertex.light_factor) + vec3(0.67244, 0.77582, 1.0),
    );
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    return vec4(in.color, 1.0);
}
