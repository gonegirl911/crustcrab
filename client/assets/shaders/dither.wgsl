struct VertexInput {
    @builtin(vertex_index) index: u32,
}

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) input_coords: vec2<f32>,
}

@vertex
fn vs_main(vertex: VertexInput) -> VertexOutput {
    let x = f32((vertex.index << 1u) & 2u);
    let y = f32(vertex.index & 2u);
    let coords = -1.0 + vec2(x, y) * 2.0;
    return VertexOutput(vec4(coords, 0.0, 1.0), vec2(x, 1.0 - y));
}

@group(0) @binding(0)
var t_input: texture_2d<f32>;

@group(0) @binding(1)
var s_input: sampler;

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let color = textureSample(t_input, s_input, in.input_coords);
    let pixel_coords = floor(in.clip_position.xy);
    let noise = interleaved_gradient_noise(pixel_coords);
    let offset = (noise - 0.5) * DITHER_STRENGTH * QUANTIZATION_STEP;
    return vec4(saturate(color.rgb + vec3(offset)), color.a);
}

fn interleaved_gradient_noise(coords: vec2<f32>) -> f32 {
    return fract(52.9829189 * fract(dot(coords, vec2(0.06711056, 0.00583715))));
}

const DITHER_STRENGTH = 1.0;
const QUANTIZATION_STEP = 1.0 / 255.0;
