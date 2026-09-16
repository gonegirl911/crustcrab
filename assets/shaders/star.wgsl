struct VertexInput {
    @builtin(vertex_index) index: u32,
}

struct InstanceInput {
    @location(0) coords: vec3<f32>,
    @location(1) rotation: f32,
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
    sky_rotation: mat4x4<f32>,
    size: f32,
    opacity: f32,
}

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
}

@group(0) @binding(0)
var<uniform> player: PlayerUniform;

var<immediate> imm: Immediates;

@vertex
fn vs_main(vertex: VertexInput, instance: InstanceInput) -> VertexOutput {
    let x = f32(((vertex.index + 2u) / 3u) % 2u);
    let y = f32(((vertex.index + 1u) / 3u) % 2u);
    let eye = imm.sky_rotation * vec4(instance.coords, 1.0);
    let m = billboard(eye.xyz, vec3(0.0), vec3(0.0, 1.0, 0.0)) * rotation_z(instance.rotation);
    let scaling = vec3(imm.size, imm.size, 1.0);
    return VertexOutput(player.vp * m * (vec4(x - 0.5, y - 0.5, 0.0, 1.0) * vec4(scaling, 1.0)));
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

fn rotation_z(theta: f32) -> mat4x4<f32> {
    let sin_theta = sin(theta);
    let cos_theta = cos(theta);
    return mat4x4(
        vec4(cos_theta, sin_theta, 0.0, 0.0),
        vec4(-sin_theta, cos_theta, 0.0, 0.0),
        vec4(0.0, 0.0, 1.0, 0.0),
        vec4(0.0, 0.0, 0.0, 1.0),
    );
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    return vec4(vec3(1.0), imm.opacity);
}
