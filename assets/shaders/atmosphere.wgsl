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
    let horizon_factor = smooth_falloff(dir.y - HORIZON_OFFSET);
    let theta = -sign(sky.sun_dir.x) * radians(sky.glow_angle);
    let rotated_y = dir.x * sin(theta) + dir.y * cos(theta);
    let arc_factor = smooth_falloff(rotated_y + GLOW_OFFSET);
    let sun_alignment = max(dot(player.forward, sky.sun_dir), 0.0);
    let horizon_glow_factor = sun_alignment * horizon_factor;
    let glow_factor = max(arc_factor, horizon_glow_factor) * sky.glow_color.a;
    let sky_gradient = mix(sky.color, sky.horizon_color, horizon_factor);
    let color = mix(sky_gradient, sky.glow_color.rgb, glow_factor);
    return vec4(color, 1.0);
}

fn smooth_falloff(x: f32) -> f32 {
    return exp2(-pow2(max(x / FALLOFF_BANDWIDTH, 0.0)));
}

fn pow2(n: f32) -> f32 {
    return n * n;
}

const HORIZON_OFFSET = sin(radians(2.0));
const FALLOFF_BANDWIDTH = sin(radians(6.0));
const GLOW_OFFSET = sin(radians(8.0));
