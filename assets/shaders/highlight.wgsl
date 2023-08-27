struct VertexInput {
    @location(0) coords: vec3<f32>,
}

struct PlayerUniform {
    vp: mat4x4<f32>,
    inv_v: mat4x4<f32>,
    inv_p: mat4x4<f32>,
    origin: vec3<f32>,
    znear: f32,
    zfar: f32,
}

struct SkyUniform {
    light_intensity: vec3<f32>,
}

struct PushConstants {
    coords: vec3<f32>,
    brightness: u32,
}

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) color: vec4<f32>,
}

@group(0) @binding(0)
var<uniform> player: PlayerUniform;

@group(1) @binding(0)
var<uniform> sky: SkyUniform;

var<push_constant> pc: PushConstants;

@vertex
fn vs_main(vertex: VertexInput) -> VertexOutput {
    // let skylight = vec3(
    //     f32(extractBits(pc.brightness, 0u, 4u)),
    //     f32(extractBits(pc.brightness, 4u, 4u)),
    //     f32(extractBits(pc.brightness, 8u, 4u)),
    // );
    let skylight = vec3(15.0);
    let torchlight = vec3(
        f32(extractBits(pc.brightness, 12u, 4u)),
        f32(extractBits(pc.brightness, 16u, 4u)),
        f32(extractBits(pc.brightness, 20u, 4u)),
    );
    let global_light = pow(vec3(0.8), (15.0 - skylight)) * sky.light_intensity;
    let local_light = pow(vec3(0.8), (15.0 - torchlight));
    return VertexOutput(
        player.vp * vec4(pc.coords + vertex.coords, 1.0),
        vec4(vec3(1.0), 0.1 * luminance(saturate(global_light + local_light))),
    );
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    return in.color;
}

fn luminance(color: vec3<f32>) -> f32 {
    return dot(color, vec3(0.299, 0.587, 0.114));
}
