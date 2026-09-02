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
    glow_color: vec3<f32>,
    glow_opacity: f32,
    arc_angle: f32,
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
    let depth = player.zfar * linearize(textureSample(t_depth, s_depth, in.input_coords).x);
    let ray_distance = depth / cos_theta;
    let distance = cylinder_distance(dir, ray_distance);
    let fog_start = f32(player.render_distance * CHUNK_DIM - FOG_PADDING);
    let bg_factor = smooth_falloff(distance - fog_start);
    let sun_alignment = max(dot(player.forward, sky.sun_dir), 0.0);
    let glow_factor = sun_alignment * sky.glow_opacity;
    let fog_color = mix(sky.horizon_color, sky.glow_color, glow_factor);
    let bg_color = textureSample(t_input, s_input, in.input_coords);
    let color = mix(vec4(fog_color, 1.0), bg_color, bg_factor);
    return color * f32(bg_color.a != 0.0);
}

fn linearize(depth: f32) -> f32 {
    return player.znear / (player.zfar - depth * (player.zfar - player.znear));
}

fn cylinder_distance(dir: vec3<f32>, ray_distance: f32) -> f32 {
    let r = sqrt(1.0 - pow2(dir.y)) * ray_distance;
    let h = abs(dir.y) * ray_distance;
    return (r + h + sqrt(pow2(r - h) + pow2(FALLOFF_BANDWIDTH))) * 0.5;
}

fn smooth_falloff(x: f32) -> f32 {
    return exp2(-pow2(max(x / FALLOFF_BANDWIDTH, 0.0)));
}

fn pow2(n: f32) -> f32 {
    return n * n;
}

const CHUNK_DIM = 16u;
const FOG_PADDING = 3u * CHUNK_DIM;
const FALLOFF_BANDWIDTH = 16.0;
