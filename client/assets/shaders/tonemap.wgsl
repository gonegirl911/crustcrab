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
    return vec4(aces(color.rgb), 1.0);
}

fn aces(color: vec3<f32>) -> vec3<f32> {
    let rrt = RGB_TO_RRT * color;
    let a = rrt * (rrt + 0.0245786) - 0.000090537;
    let b = rrt * (0.983729 * rrt + 0.4329510) + 0.238081;
    return saturate(ODT_TO_RGB * (a / b));
}

const RGB_TO_RRT = mat3x3(
        vec3(0.59719, 0.07600, 0.02840),
        vec3(0.35458, 0.90834, 0.13383),
        vec3(0.04823, 0.01566, 0.83777),
    );

const ODT_TO_RGB = mat3x3(
        vec3(1.60475, -0.10208, -0.00327),
        vec3(-0.53108, 1.10813, -0.07276),
        vec3(-0.07367, -0.00605, 1.07602),
    );
