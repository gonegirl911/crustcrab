struct VertexInput {
    @location(0) coords: vec3<f32>,
}

struct PlayerUniform {
    vp: mat4x4<f32>,
    origin: vec3<f32>,
    render_distance: u32,
}

struct PushConstants {
    coords: vec3<f32>,
}

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
}

@group(0) @binding(0)
var<uniform> player: PlayerUniform;

var<push_constant> pc: PushConstants;

@vertex
fn vs_main(vertex: VertexInput) -> VertexOutput {
    return VertexOutput(player.vp * vec4(pc.coords + vertex.coords, 1.0));
}

struct SkylightUniform {
    intensity: f32,
}

@group(1) @binding(0)
var<uniform> skylight: SkylightUniform;

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    return vec4(vec3(1.0), 0.1 * skylight.intensity);
}
