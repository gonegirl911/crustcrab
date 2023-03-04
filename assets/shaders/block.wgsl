struct VertexInput {
    @location(0) data: u32,
    @location(1) light: u32,
}

struct PlayerUniform {
    vp: mat4x4<f32>,
    origin: vec3<f32>,
    render_distance: u32,
}

struct ClockUniform {
    time: f32,
}

struct SkylightUniform {
    intensity: f32,
}

struct PushConstants {
    chunk_coords: vec3<f32>,
}

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) tex_coords: vec2<f32>,
    @location(1) sky_coords: vec2<f32>,
    @location(2) light_factor: vec3<f32>,
    @location(3) fog_factor: f32,
}

@group(0) @binding(0)
var<uniform> player: PlayerUniform;

@group(1) @binding(0)
var<uniform> clock: ClockUniform;

@group(2) @binding(0)
var<uniform> skylight: SkylightUniform;

var<push_constant> pc: PushConstants;

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
    let face = extractBits(vertex.data, 25u, 2u);
    let ao = f32(extractBits(vertex.data, 27u, 2u));
    let skylight_intensity = skylight.intensity;
    // let skylight = vec3(
    //     f32(extractBits(vertex.light, 0u, 4u)),
    //     f32(extractBits(vertex.light, 4u, 4u)),
    //     f32(extractBits(vertex.light, 8u, 4u)),
    // );
    let skylight = vec3(15.0);
    let torchlight = vec3(
        f32(extractBits(vertex.light, 12u, 4u)),
        f32(extractBits(vertex.light, 16u, 4u)),
        f32(extractBits(vertex.light, 20u, 4u)),
    );

    let dx = distance(player.origin.xz, coords.xz);
    let dy = coords.y - player.origin.y;
    let fog_height = 0.5 - atan2(dy, dx) / 22.0 * 7.0;

    let global_light = pow(vec3(0.8), (15.0 - skylight)) * skylight_intensity;
    let local_light = pow(vec3(0.8), (15.0 - torchlight));
    let face_light = mix(mix(mix(mix(0.0, 0.8, f32(face == 3u)), 0.5, f32(face == 2u)), 1.0, f32(face == 1u)), 0.6, f32(face == 0u));
    let ambient_light = mix(0.2, 1.0, ao / 3.0);
    let light_factor = clamp(global_light + local_light, vec3(0.0), vec3(1.0)) * face_light * ambient_light;

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

@group(3) @binding(0)
var t_atlas: texture_2d<f32>;

@group(3) @binding(1)
var s_atlas: sampler;

@group(4) @binding(0)
var t_sky: texture_2d<f32>;

@group(4) @binding(1)
var s_sky: sampler;

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    return mix(
        textureSample(t_atlas, s_atlas, in.tex_coords) * vec4(in.light_factor, 1.0),
        textureSample(t_sky, s_sky, in.sky_coords),
        in.fog_factor,
    );
}
