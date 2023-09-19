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
    origin: vec3<f32>,
    forward: vec3<f32>,
    render_distance: u32,
    znear: f32,
    zfar: f32,
}

struct SkyUniform {
    color: vec3<f32>,
    horizon_color: vec3<f32>,
    sun_intensity: f32,
    light_intensity: vec3<f32>,
}

@group(0) @binding(0)
var<uniform> player: PlayerUniform;

@group(1) @binding(0)
var<uniform> sky: SkyUniform;

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let dir = dir(in.screen_coords);
    let theta = deg(asin(dir.y));
    let horizon_factor = exp2(-pow2(max((theta - 4.0) / 3.0, 0.0)));
    return vec4(mix(sky.color, sky.horizon_color, horizon_factor), 1.0);
}

fn dir(screen_coords: vec2<f32>) -> vec3<f32> {
    let eye = player.inv_p * vec4(screen_coords, 1.0, 1.0);
    let dir = player.inv_v * vec4(eye.xy, 1.0, 0.0);
    return normalize(dir.xyz);
}

fn deg(rad: f32) -> f32 {
    return rad * 57.2957795130823208767981548141051703;
}

fn pow2(n: f32) -> f32 {
    return n * n;
}
