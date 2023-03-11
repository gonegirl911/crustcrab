struct VertexInput {
    @location(0) coords: vec3<f32>,
}

struct PlayerUniform {
    vp: mat4x4<f32>,
    origin: vec3<f32>,
    render_distance: u32,
}

struct SkyUniform {
    color: vec3<f32>,
    light_intensity: vec3<f32>,
}

struct PushConstants {
    coords: vec3<f32>,
}

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) color: vec4<f32>,
}

@group(0) @binding(0)
var<uniform> player: PlayerUniform;

@group(1) @binding(0)
var<uniform> sky: SkyUniform;

var<push_constant> pc: PushConstants;

@vertex
fn vs_main(vertex: VertexInput) -> VertexOutput {
    let coords = -0.001 + vertex.coords * 1.002;
    return VertexOutput(
        player.vp * vec4(pc.coords + coords, 1.0),
        vec4(vec3(1.0), 0.15 * min3(sky.light_intensity)),
    );
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    return in.color;
}

fn min3(v: vec3<f32>) -> f32 {
    return min(min(v.x, v.y), v.z);
}
