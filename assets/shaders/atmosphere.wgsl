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

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let dir = normalize((player.inv_vp * vec4(in.screen_coords, 1.0, 1.0)).xyz);
    let theta = -sign(sky.sun_dir.x) * radians(sky.glow_angle);
    let horizon_factor = factor(degrees(asin(dir.y)) - 2.0);
    let horizon_glow_factor = max(mix(1.0, -1.0, acos(dot(player.forward, sky.sun_dir)) * FRAC_1_PI), 0.0) * horizon_factor;
    let glow_factor = max(factor(degrees(asin(rotate_z(dir, theta).y)) + 8.0), horizon_glow_factor) * sky.glow_color.a;
    let color = mix(mix(sky.color, sky.horizon_color, horizon_factor), sky.glow_color.rgb, glow_factor);
    return vec4(color, 1.0);
}

fn factor(theta: f32) -> f32 {
    return exp2(-pow2(max(theta / 6.0, 0.0)));
}

fn rotate_z(dir: vec3<f32>, theta: f32) -> vec3<f32> {
    let sin_theta = sin(theta);
    let cos_theta = cos(theta);
    return vec3(
        dir.x * cos_theta - dir.y * sin_theta,
        dir.x * sin_theta + dir.y * cos_theta,
        dir.z,
    );
}

fn pow2(n: f32) -> f32 {
    return n * n;
}

const FRAC_1_PI = 0.318309886183790671537767526745028724;
