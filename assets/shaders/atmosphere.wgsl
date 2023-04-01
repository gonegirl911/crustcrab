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
    origin: vec3<f32>,
    znear: f32,
    zfar: f32,
}

struct AtmosphereUniform {
    light_dir: vec3<f32>,
    g: f32,
    light_intensity: vec3<f32>,
    h_ray: f32,
    b_ray: vec3<f32>,
    h_mie: f32,
    b_mie: vec3<f32>,
    h_ab: f32,
    b_ab: vec3<f32>,
    ab_falloff: f32,
    r_planet: f32,
    r_atmosphere: f32,
    primary_steps: u32,
    light_steps: u32,
}

@group(0) @binding(0)
var<uniform> player: PlayerUniform;

@group(1) @binding(0)
var<uniform> a: AtmosphereUniform;

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
    let color = textureSample(t_input, s_input, in.input_coords).xyz;
    let depth = linearize(textureSample(t_depth, s_depth, in.input_coords).x);
    let origin = vec3(0.0, a.r_planet + player.origin.y, 0.0);
    let dir = dir(in.screen_coords);
    return vec4(scatter(color, origin, dir, depth), 1.0);
}

fn linearize(depth: f32) -> f32 {
    return 1.0 / mix(depth, 1.0, player.zfar / player.znear);
}

fn dir(screen_coords: vec2<f32>) -> vec3<f32> {
    let eye = player.inv_p * vec4(screen_coords, 1.0, 1.0);
    let dir = player.inv_v * vec4(eye.xy, 1.0, 0.0);
    return normalize(dir.xyz);
}

fn scatter(color: vec3<f32>, origin: vec3<f32>, dir: vec3<f32>, depth: f32) -> vec3<f32> {
    return color;
}

const PI = 3.14159265358979323846264338327950288;
