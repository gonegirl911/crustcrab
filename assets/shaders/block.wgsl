struct VertexInput {
    @location(0) data: u32,
}

struct PushConstants {
    chunk_coords: vec3<f32>,
}

struct PlayerUniform {
    vp: mat4x4<f32>,
    origin: vec3<f32>,
    render_distance: u32,
}

struct ClockUniform {
    time: f32,
}

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) tex_coords: vec2<f32>,
    @location(1) sky_coords: vec2<f32>,
    @location(2) light_factor: f32,
    @location(3) fog_factor: f32,
}

var<push_constant> pc: PushConstants;

@group(0) @binding(0)
var<uniform> player: PlayerUniform;

@group(1) @binding(0)
var<uniform> clock: ClockUniform;

@vertex
fn vs_main(vertex: VertexInput) -> VertexOutput {
    let coords = pc.chunk_coords * 16.0 + vec3(
        f32(extractBits(vertex.data, 0u, 5u)),
        f32(extractBits(vertex.data, 5u, 5u)),
        f32(extractBits(vertex.data, 10u, 5u)),
    );
    let tex_coords = vec2(
        f32(extractBits(vertex.data, 15u, 1u)),
        f32(extractBits(vertex.data, 16u, 1u)),
    );
    let atlas_coords = vec2(
        f32(extractBits(vertex.data, 17u, 4u)),
        f32(extractBits(vertex.data, 21u, 4u)),
    );
    let ambient_occlusion = f32(extractBits(vertex.data, 25u, 2u));

    let dx = distance(player.origin.xz, coords.xz);
    let dy = coords.y - player.origin.y;
    let fog_height = 0.5 - atan2(dy, dx) / 22.0 * 7.0;

    let light_factor = (0.75 + ambient_occlusion) / 3.75;

    let distance = distance(player.origin, coords);
    let fog_distance = f32(player.render_distance) * 16.0 * 0.8;
    let fog_factor = pow(clamp(distance / fog_distance, 0.0, 1.0), 4.0);

    return VertexOutput(
        player.vp * vec4(coords, 1.0),
        (atlas_coords + tex_coords) / 16.0,
        vec2(clock.time, fog_height),
        light_factor,
        fog_factor,
    );
}

@group(2) @binding(0)
var t_atlas: texture_2d<f32>;

@group(2) @binding(1)
var s_atlas: sampler;

@group(3) @binding(0)
var t_sky: texture_2d<f32>;

@group(3) @binding(1)
var s_sky: sampler;

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    return mix(
        textureSample(t_atlas, s_atlas, in.tex_coords) * in.light_factor,
        textureSample(t_sky, s_sky, in.sky_coords),
        in.fog_factor,
    );
}
