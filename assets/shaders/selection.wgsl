struct VertexInput {
    @location(0) data: u32,
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
    let coords = pc.coords + vec3(
        mix(-0.001, 1.001, f32(extractBits(vertex.data, 0u, 1u))),
        mix(-0.001, 1.001, f32(extractBits(vertex.data, 1u, 1u))),
        mix(-0.001, 1.001, f32(extractBits(vertex.data, 2u, 1u))),
    );
    return VertexOutput(player.vp * vec4(coords, 1.0));
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    return vec4(vec3(1.0), 0.15);
}
