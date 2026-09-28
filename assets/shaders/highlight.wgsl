struct VertexInput {
    @location(0) coords: vec3<f32>,
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

struct Immediates {
    m: mat4x4<f32>,
    brightness: u32,
}

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) opacity: f32,
}

@group(0) @binding(0)
var<uniform> player: PlayerUniform;

@group(1) @binding(0)
var t_light: texture_3d<f32>;

@group(1) @binding(1)
var s_light: sampler;

var<immediate> imm: Immediates;

@vertex
fn vs_main(vertex: VertexInput) -> VertexOutput {
    let skylight = vec3(
        f32(extractBits(imm.brightness, 0u, 4u)),
        f32(extractBits(imm.brightness, 4u, 4u)),
        f32(extractBits(imm.brightness, 8u, 4u)),
    );
    let torchlight = vec3(
        f32(extractBits(imm.brightness, 12u, 4u)),
        f32(extractBits(imm.brightness, 16u, 4u)),
        f32(extractBits(imm.brightness, 20u, 4u)),
    );
    let light = vec3(
        sample_light(0.0, skylight.r, torchlight.r).r,
        sample_light(0.0, skylight.g, torchlight.g).g,
        sample_light(0.0, skylight.b, torchlight.b).b,
    );
    return VertexOutput(
        player.vp * (vec4(-player.origin, 0.0) + imm.m * vec4(vertex.coords, 1.0)),
        max(OPACITY_MULTIPLIER * lum(light), OPACITY_MIN),
    );
}

fn sample_light(ao: f32, skylight: f32, torchlight: f32) -> vec3<f32> {
    const AO_LEVELS = 4.0;
    const LIGHT_LEVELS = 16.0;

    let light_coords = vec3(
        (ao + 0.5) / AO_LEVELS,
        (skylight + 0.5) / LIGHT_LEVELS,
        (torchlight + 0.5) / LIGHT_LEVELS,
    );
    return textureSampleLevel(t_light, s_light, light_coords, 0.0).xyz;
}

fn lum(color: vec3<f32>) -> f32 {
    return dot(color, vec3(0.2126, 0.7152, 0.0722));
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    return vec4(vec3(1.0), in.opacity);
}

const OPACITY_MULTIPLIER = 0.1;
const OPACITY_MIN = 0.02;
