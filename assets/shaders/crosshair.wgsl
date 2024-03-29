struct VertexInput {
    @builtin(vertex_index) index: u32,
}

struct CrosshairUniform {
    transform: mat4x4<f32>, 
}

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) input_coords: vec2<f32>,
    @location(1) tex_coords: vec2<f32>,
}

@group(0) @binding(0)
var<uniform> crosshair: CrosshairUniform; 

@vertex
fn vs_main(vertex: VertexInput) -> VertexOutput {
    let x = f32(((vertex.index + 2u) / 3u) % 2u);
    let y = f32(((vertex.index + 1u) / 3u) % 2u);
    let coords = crosshair.transform * vec4(x - 0.5, y - 0.5, 0.0, 1.0);
    return VertexOutput(coords, (1.0 + vec2(coords.x, -coords.y)) * 0.5, vec2(x, 1.0 - y));
}

@group(1) @binding(0)
var t_crosshair: texture_2d<f32>;

@group(1) @binding(1)
var s_crosshair: sampler;

@group(2) @binding(0)
var t_input: texture_2d<f32>;

@group(2) @binding(1)
var s_input: sampler;

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    return vec4(
        1.0 - textureSample(t_input, s_input, in.input_coords).rgb,
        textureSample(t_crosshair, s_crosshair, in.tex_coords).a,
    );
}
