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

struct SkyUniform {
    sun_dir: vec3<f32>,
    color: vec3<f32>,
    horizon_color: vec3<f32>,
    glow_color: vec4<f32>,
    glow_angle: f32,
    sun_intensity: f32,
    light_intensity: vec3<f32>,
}

struct PushConstants {
    chunk_coords: vec3<f32>,
}

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) tex_index: u32,
    @location(1) tex_coords: vec2<f32>,
    @location(2) light_factor: vec3<f32>,
}

@group(0) @binding(0)
var<uniform> player: PlayerUniform;

@group(1) @binding(0)
var<uniform> sky: SkyUniform;

var<push_constant> pc: PushConstants;

@vertex
fn vs_main(vertex: VertexInput) -> VertexOutput {
    let coords = pc.chunk_coords * 16.0 + vec3(
        f32(extractBits(vertex.data[0], 0u, 5u)),
        f32(extractBits(vertex.data[0], 5u, 5u)),
        f32(extractBits(vertex.data[0], 10u, 5u)),
    );
    let tex_idx = extractBits(vertex.data[0], 15u, 8u);
    let tex_coords = vec2(
        f32(extractBits(vertex.data[0], 27u, 5u)),
        f32(extractBits(vertex.data[1], 27u, 5u)),
    );
    let face = extractBits(vertex.data[0], 23u, 2u);
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
    let face_light = mix(mix(mix(mix(0.0, 0.6, f32(face == 0u)), 1.0, f32(face == 1u)), 0.5, f32(face == 2u)), 0.8, f32(face == 3u));
    let ambient_light = mix(0.2, 1.0, ao / 3.0);
    let global_light = pow(vec3(0.8), (15.0 - skylight)) * sky.light_intensity;
    let local_light = pow(vec3(0.8), (15.0 - torchlight));
    return VertexOutput(
        player.vp * vec4(-player.origin + coords, 1.0),
        tex_idx,
        tex_coords,
        saturate(global_light + local_light) * ambient_light * face_light,
    );
}

@group(2) @binding(0)
var t_blocks: binding_array<texture_2d<f32>>;

@group(2) @binding(1)
var s_block: sampler;

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let color = textureSample(t_blocks[in.tex_index], s_block, in.tex_coords);
    if color.a == 0.0 {
        discard;
    } else {
        return color * vec4(in.light_factor, 1.0);
    }
}
