struct VertexInput {
    @builtin(vertex_index) index: u32,
}

struct PlayerUniform {
    vp: mat4x4<f32>,
    inv_vp: mat4x4<f32>,
    origin: vec3<f32>,
    forward: vec3<f32>,
    render_distance: u32,
    znear: f32,
    zfar: f32,
}

struct PushConstants {
    m: mat4x4<f32>,
    tex_index: u32,
}

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) tex_coords: vec2<f32>,
}

@group(0) @binding(0)
var<uniform> player: PlayerUniform;

var<push_constant> pc: PushConstants;

@vertex
fn vs_main(vertex: VertexInput) -> VertexOutput {
    let x = f32(((vertex.index + 2u) / 3u) % 2u);
    let y = f32(((vertex.index + 1u) / 3u) % 2u);
    let coords = player.vp * pc.m * vec4(x - 0.5, y - 0.5, 0.0, 1.0);
    return VertexOutput(coords, vec2(x, 1.0 - y));
}

struct SkyUniform {
    sun_dir: vec3<f32>,
    color: vec3<f32>,
    horizon_color: vec3<f32>,
    glow_color: vec4<f32>,
    glow_angle: f32,
    sun_intensity: f32,
    light_intensity: vec3<f32>,
}

@group(1) @binding(0)
var<uniform> sky: SkyUniform;

@group(2) @binding(0)
var t_object: binding_array<texture_2d<f32>>;

@group(2) @binding(1)
var s_object: sampler;

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let color = textureSample(t_object[pc.tex_index], s_object, in.tex_coords);
    return color * vec4(vec3(sky.sun_intensity), 1.0);
}
