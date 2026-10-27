struct VertexInput {
    @location(0) coords: vec3<f32>,
    @location(1) side_delta_xz: vec2<f32>,
    @location(2) side_shade: u32,
}

struct InstanceInput {
    @location(3) offset: vec2<f32>,
}

struct PlayerUniform {
    vp: mat4x4<f32>,
    inv_vp: mat4x4<f32>,
    origin: vec3<f32>,
    forward: vec3<f32>,
    znear: f32,
    zfar: f32,
    render_distance: u32,
}

struct ShadingUniform {
    side_factors: vec4<f32>,
}

struct Immediates {
    occupancy_dims: vec2<f32>,
    size: vec2<f32>,
    color: vec3<f32>,
    phase: vec2<f32>,
    altitude: f32,
}

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) light_factor: f32,
}

@group(0) @binding(0)
var<uniform> player: PlayerUniform;

@group(1) @binding(0)
var<uniform> shading: ShadingUniform;

@group(2) @binding(0)
var t_occupancy: texture_2d<f32>;

@group(2) @binding(1)
var s_occupancy: sampler;

var<immediate> imm: Immediates;

@vertex
fn vs_main(vertex: VertexInput, instance: InstanceInput) -> VertexOutput {
    let scroll_xz = imm.phase + instance.offset;
    let tex_coords = scroll_xz / imm.size.x / imm.occupancy_dims;
    let is_visible = is_visible(tex_coords, vertex.side_delta_xz);
    let cloud_dims = vec3(imm.size, imm.size.x);
    let offset = instance.offset - rem_euclid(imm.phase, imm.size.x);
    let world_pos = vertex.coords * cloud_dims + vec3(offset.x, imm.altitude, offset.y);
    let clip_position = select(CULLED_CLIP_POSITION, player.vp * vec4(world_pos, 1.0), is_visible);
    let side_factor = shading.side_factors[vertex.side_shade];
    return VertexOutput(clip_position, side_factor);
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    return vec4(imm.color * in.light_factor, 1.0);
}

fn is_visible(tex_coords: vec2<f32>, side_delta: vec2<f32>) -> bool {
    let is_current_occupied = is_occupied(tex_coords);
    let is_neighbor_occupied = is_occupied(tex_coords + side_delta / imm.occupancy_dims);
    let is_internal = any(side_delta != vec2(0.0)) && is_neighbor_occupied;
    return is_current_occupied && !is_internal;
}

fn rem_euclid(a: vec2<f32>, b: f32) -> vec2<f32> {
    let r = a % b;
    return select(r, r + abs(b), r < vec2(0.0));
}

fn is_occupied(tex_coords: vec2<f32>) -> bool {
    return textureSampleLevel(t_occupancy, s_occupancy, tex_coords, 0.0).r == 1.0;
}

const CULLED_CLIP_POSITION = vec4(0.0, 0.0, 2.0, 1.0);
