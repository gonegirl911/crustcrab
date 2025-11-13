struct VertexInput {
    @builtin(vertex_index) index: u32,
}

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) screen_coords: vec2<f32>,
    @location(1) input_coords: vec2<f32>,
}

@vertex
fn vs_main(vertex: VertexInput) -> VertexOutput {
    let x = f32((vertex.index << 1u) & 2u);
    let y = f32(vertex.index & 2u);
    let coords = -1.0 + vec2(x, y) * 2.0;
    return VertexOutput(vec4(coords, 0.0, 1.0), coords, vec2(x, 1.0 - y));
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

struct SkyUniform {
    sun_dir: vec3<f32>,
    color: vec3<f32>,
    horizon_color: vec3<f32>,
    glow_color: vec4<f32>,
    glow_angle: f32,
    sun_intensity: f32,
    light_intensity: vec3<f32>,
}

@group(0) @binding(0)
var<uniform> player: PlayerUniform;

@group(1) @binding(0)
var<uniform> sky: SkyUniform;

@group(2) @binding(0)
var t_input: texture_2d<f32>;

@group(2) @binding(1)
var s_input: sampler;

@group(3) @binding(0)
var t_depth: texture_2d<f32>;

@group(3) @binding(1)
var s_depth: sampler;

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let dir = normalize((player.inv_vp * vec4(in.screen_coords, 1.0, 1.0)).xyz);
    let cos_theta = dot(dir, player.forward);
    let sin_gamma = max(abs(dir.y), sqrt(1.0 - dir.y * dir.y));
    let distance = player.zfar * linearize(textureSample(t_depth, s_depth, in.input_coords).x) / cos_theta * sin_gamma;
    let fog_start = f32((player.render_distance - 3u) * 16u);
    let fog_factor = exp2(-pow2(max((distance - fog_start) / 16.0, 0.0)));
    let glow_factor = max(mix(1.0, -1.0, acos(dot(player.forward, sky.sun_dir)) * FRAC_1_PI), 0.0) * sky.glow_color.a;
    let fog_color = mix(sky.horizon_color, sky.glow_color.rgb, glow_factor);
    let bg_color = textureSample(t_input, s_input, in.input_coords);
    return mix(vec4(fog_color, 1.0), bg_color, fog_factor) * f32(bg_color.a != 0.0);
}

fn linearize(depth: f32) -> f32 {
    let znear = player.znear;
    let zfar = player.zfar;
    return znear * zfar / (zfar - depth * (zfar - znear));
}

fn pow2(n: f32) -> f32 {
    return n * n;
}

const FRAC_1_PI = 0.318309886183790671537767526745028724;
