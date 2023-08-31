struct VertexInput {
    @location(0) data: u32,
}

struct InstanceInput {
    @location(1) offset: vec2<f32>,
}

struct PlayerUniform {
    vp: mat4x4<f32>,
    inv_v: mat4x4<f32>,
    inv_p: mat4x4<f32>,
    origin: vec3<f32>,
    znear: f32,
    zfar: f32,
}

struct PushConstants {
    offset: vec2<f32>,
}

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) offset: vec2<f32>,
    @location(1) light: f32,
}

@group(0) @binding(0)
var<uniform> player: PlayerUniform;

var<push_constant> pc: PushConstants;

@vertex
fn vs_main(vertex: VertexInput, instance: InstanceInput) -> VertexOutput {
    let coords = vec3(12.0, 4.0, 12.0) * vec3(
        f32(extractBits(vertex.data, 0u, 5u)),
        f32(extractBits(vertex.data, 5u, 5u)),
        f32(extractBits(vertex.data, 10u, 5u)),
    );
    let face = extractBits(vertex.data, 25u, 2u);
    let offset = instance.offset + pc.offset - player.origin.xz;
    let light = mix(mix(mix(mix(0.0, 0.6, f32(face == 0u)), 1.0, f32(face == 1u)), 0.5, f32(face == 2u)), 0.8, f32(face == 3u));
    return VertexOutput(
        player.vp * vec4(coords + vec3(offset.x, 192.0, offset.y), 1.0) + vec4(player.origin, 0.0),
        floor(offset / 12.0) / 255.0,
        light,
    );
}

struct SkyUniform {
    light_intensity: vec3<f32>,
    sun_intensity: f32,
}

@group(1) @binding(0)
var<uniform> sky: SkyUniform;

@group(2) @binding(0)
var t_clouds: texture_2d<f32>;

@group(2) @binding(1)
var s_clouds: sampler;

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let is_visible = textureSample(t_clouds, s_clouds, in.offset).w == 1.0;
    if is_visible {
        return vec4(sky.light_intensity * in.light, 1.0);
    } else {
        discard;
    }
}
