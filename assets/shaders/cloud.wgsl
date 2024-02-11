struct VertexInput {
    @location(0) data: vec2<u32>,
}

struct InstanceInput {
    @location(1) offset: vec2<f32>,
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

struct PushConstants {
    dims: vec2<f32>,
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

var<push_constant> pc: PushConstants;

@vertex
fn vs_main(vertex: VertexInput, instance: InstanceInput) -> VertexOutput {
    let coords = vec3(
        f32(extractBits(vertex.data[0], 0u, 5u)),
        f32(extractBits(vertex.data[0], 5u, 5u)),
        f32(extractBits(vertex.data[0], 10u, 5u)),
    );
    let face = extractBits(vertex.data[0], 23u, 2u);
    let offset = instance.offset - rem_euclid(player.origin.xz - pc.offset, pc.size.x);
    let light_factor = mix(mix(mix(mix(0.0, 0.6, f32(face == 0u)), 1.0, f32(face == 1u)), 0.5, f32(face == 2u)), 0.8, f32(face == 3u));
    return VertexOutput(
        player.vp * vec4(vec3(pc.size, pc.size.x) * ((-0.5 + coords) * pc.scale_factor + 0.5) + vec3(offset.x, -player.origin.y + 192.0, offset.y), 1.0),
        (player.origin.xz + instance.offset - pc.offset) / pc.size.x / pc.dims,
        light_factor,
    );
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
        return vec4(pc.color * in.light_factor, 1.0);
    } else {
        discard;
    }
}
