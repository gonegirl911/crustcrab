struct VertexInput {
    @location(0) coords: vec3<f32>,
    @location(1) tex_v: f32,
}

struct PlayerUniform {
    vp: mat4x4<f32>,
    origin: vec3<f32>,
    render_distance: u32,
}

struct ClockUniform {
    time: f32,
}

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) tex_coords: vec2<f32>,
}

@group(0) @binding(0)
var<uniform> player: PlayerUniform;

@group(1) @binding(0)
var<uniform> clock: ClockUniform;

@vertex
fn vs_main(vertex: VertexInput) -> VertexOutput {
    return VertexOutput(
        player.vp * vec4(player.origin + vertex.coords, 1.0),
        vec2(clock.time, vertex.tex_v),
    );
}

@group(2) @binding(0)
var t_sky: texture_2d<f32>;

@group(2) @binding(1)
var s_sky: sampler;

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    return textureSample(t_sky, s_sky, in.tex_coords);
}
