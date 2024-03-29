struct VertexInput {
    @location(0) data: vec2<u32>,
}

struct InventoryUniform {
    transform: mat4x4<f32>,
}

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) tex_index: u32,
    @location(1) tex_coords: vec2<f32>,
    @location(2) light_factor: f32,
}

@group(0) @binding(0)
var<uniform> inventory: InventoryUniform;

@vertex
fn vs_main(vertex: VertexInput) -> VertexOutput {
    let coords = inventory.transform * vec4(
        f32(extractBits(vertex.data[0], 0u, 5u)),
        f32(extractBits(vertex.data[0], 5u, 5u)),
        f32(extractBits(vertex.data[0], 10u, 5u)),
        1.0,
    );
    let tex_idx = extractBits(vertex.data[0], 15u, 8u);
    let tex_coords = vec2(
        f32(extractBits(vertex.data[0], 27u, 5u)),
        f32(extractBits(vertex.data[1], 27u, 5u)),
    );
    let face = extractBits(vertex.data[0], 23u, 2u);
    let face_light = mix(mix(mix(mix(0.0, 0.6, f32(face == 0u)), 1.0, f32(face == 1u)), 0.5, f32(face == 2u)), 0.8, f32(face == 3u));
    return VertexOutput(coords, tex_idx, tex_coords, face_light);
}

@group(1) @binding(0)
var t_blocks: binding_array<texture_2d<f32>>;

@group(1) @binding(1)
var s_block: sampler;

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let color = textureSample(t_blocks[in.tex_index], s_block, in.tex_coords);
    return color * vec4(vec3(in.light_factor), 1.0);
}
