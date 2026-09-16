enable wgpu_binding_array;

struct VertexInput {
    @builtin(vertex_index) index: u32,
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
    dir: vec3<f32>,
    up: vec3<f32>,
    size: f32,
    tex_index: u32,
    brightness: f32,
}

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) tex_coords: vec2<f32>,
}

@group(0) @binding(0)
var<uniform> player: PlayerUniform;

var<immediate> imm: Immediates;

@vertex
fn vs_main(vertex: VertexInput) -> VertexOutput {
    let x = f32(((vertex.index + 2u) / 3u) % 2u);
    let y = f32(((vertex.index + 1u) / 3u) % 2u);
    let m = billboard(imm.dir, vec3(0.0), imm.up);
    let scaling = vec3(imm.size, imm.size, 1.0);
    let coords = player.vp * m * (vec4(x - 0.5, y - 0.5, 0.0, 1.0) * vec4(scaling, 1.0));
    return VertexOutput(coords, vec2(x, 1.0 - y));
}

fn billboard(eye: vec3<f32>, towards: vec3<f32>, up: vec3<f32>) -> mat4x4<f32> {
    let z_axis = normalize(towards - eye);
    let x_axis = normalize(cross(up, z_axis));
    let y_axis = cross(z_axis, x_axis);
    return mat4x4(
        vec4(-x_axis, 0.0),
        vec4(y_axis, 0.0),
        vec4(z_axis, 0.0),
        vec4(eye, 1.0),
    );
}

@group(2) @binding(0)
var t_object: binding_array<texture_2d<f32>>;

@group(2) @binding(1)
var s_object: sampler;

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let color = textureSample(t_object[imm.tex_index], s_object, in.tex_coords);
    return color * vec4(vec3(imm.brightness), 1.0);
}
