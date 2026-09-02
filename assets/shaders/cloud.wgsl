struct VertexInput {
    @location(0) data: vec2<u32>,
}

struct InstanceInput {
    @location(1) offset: vec2<f32>,
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
    tex_dims: vec2<f32>,
    size: vec2<f32>,
    scale_factor: vec3<f32>,
    color: vec3<f32>,
    offset: vec2<f32>,
}

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) tex_coords: vec2<f32>,
    @location(1) light_factor: f32,
}

@group(0) @binding(0)
var<uniform> player: PlayerUniform;

var<immediate> imm: Immediates;

@vertex
fn vs_main(vertex: VertexInput, instance: InstanceInput) -> VertexOutput {
    let coords = vec3(
        f32(extractBits(vertex.data[0], 0u, 5u)),
        f32(extractBits(vertex.data[0], 5u, 5u)),
        f32(extractBits(vertex.data[0], 10u, 5u)),
    );
    let face = extractBits(vertex.data[0], 23u, 2u);
    let offset = instance.offset - rem_euclid(player.origin.xz - imm.offset, imm.size.x);
    let scaled_coords = (coords - 0.5) * imm.scale_factor + 0.5;
    let cloud_dims = vec3(imm.size, imm.size.x);
    let world_pos = scaled_coords * cloud_dims + vec3(offset.x, -player.origin.y + 192.0, offset.y);
    let scroll_xz = player.origin.xz + instance.offset - imm.offset;
    let tex_coords = scroll_xz / imm.size.x / imm.tex_dims;
    let light_factor = array(0.6, 1.0, 0.5, 0.8)[face];
    return VertexOutput(player.vp * vec4(world_pos, 1.0), tex_coords, light_factor);
}

fn rem_euclid(a: vec2<f32>, b: f32) -> vec2<f32> {
    let r = a % b;
    return mix(r, r + abs(b), vec2<f32>(r < vec2(0.0)));
}

@group(1) @binding(0)
var t_clouds: texture_2d<f32>;

@group(1) @binding(1)
var s_clouds: sampler;

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    if textureSample(t_clouds, s_clouds, in.tex_coords).a == 1.0 {
        return vec4(imm.color * in.light_factor, 1.0);
    } else {
        discard;
    }
}
