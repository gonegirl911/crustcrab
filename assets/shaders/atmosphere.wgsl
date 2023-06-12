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

struct AtmosphereUniform {
    sun_dir: vec3<f32>,
    a: vec3<f32>,
    b: vec3<f32>,
    c: vec3<f32>,
    d: vec3<f32>,
    e: vec3<f32>, 
    z: vec3<f32>,
}

@group(0) @binding(0)
var<uniform> player: PlayerUniform;

@group(1) @binding(0)
var<uniform> a: AtmosphereUniform;

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let dir = dir(in.screen_coords);
    let cos_theta = saturate(dir.y);
    let cos_gamma = dot(dir, a.sun_dir);
    let gamma = acos(cos_gamma);
    return vec4(XYZ_to_RGB(xyY_to_XYZ(a.z * perez(cos_theta, gamma, cos_gamma))) * 0.025, 1.0);
}

fn dir(screen_coords: vec2<f32>) -> vec3<f32> {
    let eye = player.inv_p * vec4(screen_coords, 1.0, 1.0);
    let dir = player.inv_v * vec4(eye.xy, 1.0, 0.0);
    return normalize(dir.xyz);
}

fn perez(cos_theta: f32, gamma: f32, cos_gamma: f32) -> vec3<f32> {
    return (1.0 + a.a * exp(a.b / cos_theta))
        * (1.0 + a.c * exp(a.d * gamma) + a.e * cos_gamma * cos_gamma);
}

fn xyY_to_XYZ(xyY: vec3<f32>) -> vec3<f32> {
    return vec3(xyY.x, xyY.y, 1.0 - xyY.x - xyY.y) * xyY.z / xyY.y;
}

fn XYZ_to_RGB(XYZ: vec3<f32>) -> vec3<f32> {
    return XYZ_TO_RGB * XYZ;
}

const XYZ_TO_RGB = mat3x3<f32>(
    vec3<f32>(3.240479, -0.969256, 0.055648),
    vec3<f32>(-1.537150, 1.875992, -0.204043),
    vec3<f32>(-0.498535, 0.041556, 1.057311),
);
