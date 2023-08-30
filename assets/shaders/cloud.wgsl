struct VertexInput {
    @location(0) coords: vec3<f32>,
    @location(1) light: f32,
}

struct InstanceInput {
    @location(2) offset: vec3<f32>,
}

struct PlayerUniform {
    vp: mat4x4<f32>,
    inv_v: mat4x4<f32>,
    inv_p: mat4x4<f32>,
    origin: vec3<f32>,
    znear: f32,
    zfar: f32,
}

struct SkyUniform {
    light_intensity: vec3<f32>,
    sun_intensity: f32,
}

struct PushConstants {
    offset: vec3<f32>,
}

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) color: vec4<f32>,
}

@group(0) @binding(0)
var<uniform> player: PlayerUniform;

@group(1) @binding(0)
var<uniform> sky: SkyUniform;

@group(2) @binding(0)
var t_clouds: binding_array<texture_2d<f32>>;

@group(2) @binding(1)
var s_clouds: sampler;

var<push_constant> pc: PushConstants;

@vertex
fn vs_main(vertex: VertexInput, instance: InstanceInput) -> VertexOutput {
    
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    return in.color;
}
