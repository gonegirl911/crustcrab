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
    inv_v: mat4x4<f32>,
    inv_p: mat4x4<f32>,
    znear: f32,
    zfar: f32,
}

struct SkyUniform {
    sun_coords: vec3<f32>,
    light_intensity: vec3<f32>,
}

struct AtmosphereUniform {
    sun_intensity: vec3<f32>,
    sc_air: vec3<f32>,
    sc_haze: vec3<f32>,
    ex: vec3<f32>,
    ex_air: vec3<f32>,
    ex_haze: vec3<f32>,
    s_air: f32,
    s_haze: f32,
    g: f32,
}

@group(0) @binding(0)
var<uniform> player: PlayerUniform;

@group(1) @binding(0)
var<uniform> sky: SkyUniform;

@group(2) @binding(0)
var<uniform> a: AtmosphereUniform;

@group(3) @binding(0)
var t_input: texture_2d<f32>;

@group(3) @binding(1)
var s_input: sampler;

@group(4) @binding(0)
var t_depth: texture_2d<f32>;

@group(4) @binding(1)
var s_depth: sampler;

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let color = textureSample(t_input, s_input, in.input_coords).xyz;
    let depth = linearize(textureSample(t_depth, s_depth, in.input_coords).x);
    let cos_theta = dot(dir(in.screen_coords), -sky.sun_coords);
    let sky_color = sky_color(cos_theta);
    let perspective = aerial_perspective(color, player.zfar * depth, cos_theta);
    return vec4(mix(sky_color, perspective, f32(depth < 1.0)), 1.0);
}

fn linearize(depth: f32) -> f32 {
    return 1.0 / mix(depth, 1.0, player.zfar / player.znear);
}

fn dir(screen_coords: vec2<f32>) -> vec3<f32> {
    let eye = player.inv_p * vec4(screen_coords, 1.0, 1.0);
    let dir = player.inv_v * vec4(eye.xy, 1.0, 0.0);
    return normalize(dir.xyz);
}

fn sky_color(cos_theta: f32) -> vec3<f32> {
    let inner = in_scattering(sc(cos_theta), a.ex, a.s_haze);
    let outer = in_scattering(sc_air(cos_theta), a.ex_air, a.s_air - a.s_haze);
    return a.sun_intensity * inner * outer * exp(-a.ex * a.s_haze);
}

fn aerial_perspective(color: vec3<f32>, s: f32, cos_theta: f32) -> vec3<f32> {
    let ex = extinction(s);
    let in = a.sun_intensity * in_scattering(sc(cos_theta), a.ex, s);
    return color * ex + in;
}

fn extinction(s: f32) -> vec3<f32> {
    return exp(-a.ex * s);
}

fn in_scattering(sc: vec3<f32>, ex: vec3<f32>, s: f32) -> vec3<f32> {
    let n = sc * (1.0 - exp(-ex * s));
    let d = ex;
    return n / d;
}

fn sc(cos_theta: f32) -> vec3<f32> {
    return sc_air(cos_theta) + sc_haze(cos_theta);
}

fn sc_air(cos_theta: f32) -> vec3<f32> {
    return a.sc_air * f_air(cos_theta);
}

fn sc_haze(cos_theta: f32) -> vec3<f32> {
    return a.sc_haze * f_haze(cos_theta);
}

fn f_air(cos_theta: f32) -> f32 {
    let n = 3.0 * (1.0 + cos_theta * cos_theta);
    let d = 16.0 * PI;
    return n / d;
}

fn f_haze(cos_theta: f32) -> f32 {
    let n = (1.0 - a.g) * (1.0 - a.g);
    let d = 4.0 * PI * pow(1.0 + a.g * a.g - 2.0 * a.g * cos_theta, 1.5);
    return n / d;
}

const PI = 3.14159265358979323846264338327950288;
