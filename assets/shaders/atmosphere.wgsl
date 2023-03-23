struct VertexInput {
    @builtin(vertex_index) index: u32,
}

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) screen_coords: vec2<f32>,
}

@vertex
fn vs_main(vertex: VertexInput) -> VertexOutput {
    let x = f32((vertex.index << 1u) & 2u);
    let y = f32(vertex.index & 2u);
    let coords = -1.0 + vec2(x, y) * 2.0;
    return VertexOutput(vec4(coords, 0.0, 1.0), coords);
}

struct PlayerUniform {
    vp: mat4x4<f32>,
    inv_v: mat4x4<f32>,
    inv_p: mat4x4<f32>,
}

struct SkyUniform {
    sun_coords: vec3<f32>,
    light_intensity: vec3<f32>,
}

struct AtmosphereUniform {
    sun_intensity: vec3<f32>,
    sc_air: vec3<f32>,
    sc_haze: vec3<f32>,
    ex_air: vec3<f32>,
    ex_haze: vec3<f32>,
    ex: vec3<f32>,
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

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    return vec4(sky_color(dot(dir(in.screen_coords), sky.sun_coords)), 1.0);
}

fn dir(screen_coords: vec2<f32>) -> vec3<f32> {
    let eye = player.inv_p * vec4(screen_coords, 1.0, 1.0);
    let dir = player.inv_v * vec4(eye.xy, 1.0, 0.0);
    return normalize(dir.xyz);
}

fn sky_color(cos_theta: f32) -> vec3<f32> {
    let f_air = f_air(cos_theta);
    let f_haze = f_haze(cos_theta);
    let inner = in_scattering(a.sc_air * f_air + a.sc_haze * f_haze, a.ex, a.s_haze, cos_theta);
    let outer = in_scattering(a.sc_air * f_air, a.ex_air, a.s_air - a.s_haze, cos_theta);
    return a.sun_intensity * inner * outer * exp(-a.ex * a.s_haze);
}

fn in_scattering(sc: vec3<f32>, ex: vec3<f32>, s: f32, cos_theta: f32) -> vec3<f32> {
    let n = sc * (1.0 - exp(-ex * s));
    let d = ex;
    return n / d;
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
