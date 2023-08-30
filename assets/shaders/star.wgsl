struct VertexInput {
    @builtin(vertex_index) index: u32,
}

struct InstanceInput {
    @location(0) m0: vec4<f32>,
    @location(1) m1: vec4<f32>,
    @location(2) m2: vec4<f32>,
    @location(3) m3: vec4<f32>,
}

struct PlayerUniform {
    vp: mat4x4<f32>,
    inv_v: mat4x4<f32>,
    inv_p: mat4x4<f32>,
    origin: vec3<f32>,
    znear: f32,
    zfar: f32,
}

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
}

@group(0) @binding(0)
var<uniform> player: PlayerUniform;

@vertex
fn vs_main(vertex: VertexInput, instance: InstanceInput) -> VertexOutput {
    let x = f32(((vertex.index + 2u) / 3u) % 2u);
    let y = f32(((vertex.index + 1u) / 3u) % 2u);
    let m = mat4x4(instance.m0, instance.m1, instance.m2, instance.m3);
    let coords = player.vp * (m * vec4(x - 0.5, y - 0.5, 0.0, 1.0) + vec4(player.origin, 0.0));
    return VertexOutput(coords);
}

struct SkyUniform {
    light_intensity: vec3<f32>,
    sun_intensity: f32,
}

@group(1) @binding(0)
var<uniform> sky: SkyUniform;

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    return vec4(vec3(0.15), 1.0 - min(sky.sun_intensity, 10.0) / 10.0);
}
