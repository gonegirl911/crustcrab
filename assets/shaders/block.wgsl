enable wgpu_binding_array;

struct VertexInput {
    @location(0) data: vec2<u32>,
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

struct ShadingUniform {
    side_factors: vec4<f32>,
}

struct Immediates {
    chunk_coords: vec3<f32>,
}

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) @interpolate(flat) tex_index: u32,
    @location(1) tex_coords: vec2<f32>,
    @location(2) @interpolate(flat) side_shade: u32,
    @location(3) ao: f32,
    @location(4) skylight: vec3<f32>,
    @location(5) torchlight: vec3<f32>,
}

@group(0) @binding(0)
var<uniform> player: PlayerUniform;

@group(1) @binding(0)
var<uniform> shading: ShadingUniform;

var<immediate> imm: Immediates;

@vertex
fn vs_main(vertex: VertexInput) -> VertexOutput {
    let coords = vec3(
        f32(extractBits(vertex.data[0], 0u, 5u)),
        f32(extractBits(vertex.data[0], 5u, 5u)),
        f32(extractBits(vertex.data[0], 10u, 5u)),
    );
    let tex_idx = extractBits(vertex.data[0], 15u, 8u);
    let tex_coords = vec2(
        f32(extractBits(vertex.data[0], 27u, 5u)),
        f32(extractBits(vertex.data[1], 27u, 5u)),
    );
    let side_shade = extractBits(vertex.data[0], 23u, 2u);
    let ao = f32(extractBits(vertex.data[0], 25u, 2u));
    let skylight = vec3(
        f32(extractBits(vertex.data[1], 0u, 4u)),
        f32(extractBits(vertex.data[1], 4u, 4u)),
        f32(extractBits(vertex.data[1], 8u, 4u)),
    );
    let torchlight = vec3(
        f32(extractBits(vertex.data[1], 12u, 4u)),
        f32(extractBits(vertex.data[1], 16u, 4u)),
        f32(extractBits(vertex.data[1], 20u, 4u)),
    );
    return VertexOutput(
        player.vp * vec4(-player.origin + imm.chunk_coords * CHUNK_DIM + coords, 1.0),
        tex_idx,
        tex_coords,
        side_shade,
        ao,
        skylight,
        torchlight,
    );
}

@group(2) @binding(0)
var t_blocks: binding_array<texture_2d<f32>>;

@group(2) @binding(1)
var s_block: sampler;

@group(3) @binding(0)
var t_light: texture_3d<f32>;

@group(3) @binding(1)
var s_light: sampler;

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let color = textureSample(t_blocks[in.tex_index], s_block, in.tex_coords);
    let light_factor = light_factor(in.side_shade, in.ao, in.skylight, in.torchlight);
    return color * vec4(light_factor, 1.0);
}

@fragment
fn fs_cutout(in: VertexOutput) -> @location(0) vec4<f32> {
    let color = textureSample(t_blocks[in.tex_index], s_block, in.tex_coords);
    if color.a == 0.0 {
        discard;
    }
    let light_factor = light_factor(in.side_shade, in.ao, in.skylight, in.torchlight);
    return color * vec4(light_factor, 1.0);
}

fn light_factor(side_shade: u32, ao: f32, skylight: vec3<f32>, torchlight: vec3<f32>) -> vec3<f32> {
    let light = vec3(
        sample_light(ao, skylight.r, torchlight.r).r,
        sample_light(ao, skylight.g, torchlight.g).g,
        sample_light(ao, skylight.b, torchlight.b).b,
    );
    return light * shading.side_factors[side_shade];
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

const CHUNK_DIM = 16.0;
