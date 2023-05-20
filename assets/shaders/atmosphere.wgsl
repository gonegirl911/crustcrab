struct VertexInput {
    @builtin(vertex_index) index: u32,
}

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) screen_coords: vec2<f32>,
    @location(1) input_coords: vec2<f32>,
}

@vertex
fn vs_main(vertex: VertexInput) -> VertexOutput {
    let x = f32((vertex.index << 1u) & 2u);
    let y = f32(vertex.index & 2u);
    let coords = -1.0 + vec2(x, y) * 2.0;
    return VertexOutput(vec4(coords, 0.0, 1.0), coords, vec2(x, 1.0 - y));
}

struct PlayerUniform {
    vp: mat4x4<f32>,
    inv_v: mat4x4<f32>,
    inv_p: mat4x4<f32>,
    origin: vec3<f32>,
    znear: f32,
    zfar: f32,
}

struct AtmosphereUniform {
    sun_dir: vec3<f32>,
    g: f32,
    sun_intensity: vec3<f32>,
    h_ray: f32,
    b_ray: vec3<f32>,
    h_mie: f32,
    b_mie: vec3<f32>,
    h_ab: f32,
    b_ab: vec3<f32>,
    ab_falloff: f32,
    r_planet: f32,
    r_atmosphere: f32,
    num_samples: u32,
    num_light_samples: u32,
}

@group(0) @binding(0)
var<uniform> player: PlayerUniform;

@group(1) @binding(0)
var<uniform> a: AtmosphereUniform;

@group(2) @binding(0)
var t_input: texture_2d<f32>;

@group(2) @binding(1)
var s_input: sampler;

@group(3) @binding(0)
var t_depth: texture_2d<f32>;

@group(3) @binding(1)
var s_depth: sampler;

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let color = textureSample(t_input, s_input, in.input_coords).xyz;
    let origin = vec3(0.0, a.r_planet + player.origin.y, 0.0);
    let dir = dir(in.screen_coords);
    let depth = linearize(textureSample(t_depth, s_depth, in.input_coords).x);
    return vec4(scatter(color, origin, dir, depth), 1.0);
}

fn linearize(depth: f32) -> f32 {
    return 1.0 / mix(depth, 1.0, player.zfar / player.znear);
}

fn dir(screen_coords: vec2<f32>) -> vec3<f32> {
    let eye = player.inv_p * vec4(screen_coords, 1.0, 1.0);
    let dir = player.inv_v * vec4(eye.xy, 1.0, 0.0);
    return normalize(dir.xyz);
}

fn scatter(color: vec3<f32>, origin: vec3<f32>, dir: vec3<f32>, depth: f32) -> vec3<f32> {
    let cos_theta = dot(dir, a.sun_dir);
    let phase_ray = phase_ray(cos_theta);
    let phase_mie = phase_mie(cos_theta);
    
    var sum_ray = vec3(0.0);
    var sum_mie = vec3(0.0);

    let t = intersect(origin, dir, a.r_atmosphere);
    let length = mix(min(player.zfar * depth, t), t, f32(depth == 1.0));
    let step_size = length / f32(a.num_samples);
    
    var t_curr = step_size * 0.5;
    var opt = vec3(0.0);

    for (var i = 0u; i < a.num_samples; i++) {
        let coords = origin + dir * t_curr;
        let density = density(coords) * step_size;

        t_curr += step_size;
        opt += density;

        let length = intersect(coords, a.sun_dir, a.r_atmosphere);
        let step_size = length / f32(a.num_light_samples);
        
        var t_curr = step_size * 0.5;
        var opt_light = vec3(0.0);

        for (var i = 0u; i < a.num_light_samples; i++) {
            let coords = coords + a.sun_dir * t_curr;
            let density = density(coords) * step_size;
            
            t_curr += step_size;
            opt_light += density;
        }
        
        let attn = exp(-a.b_ray * (opt.x + opt_light.x) - a.b_mie * (opt.y + opt_light.y) - a.b_ab * (opt.z + opt_light.z));
        
        sum_ray += density.x * attn;
        sum_mie += density.y * attn;
    }

    let ex = exp(-a.b_ray * opt.x - a.b_mie * opt.y - a.b_ab * opt.z);
    let in = a.sun_intensity * (sum_ray * a.b_ray * phase_ray + sum_mie * a.b_mie * phase_mie);
    
    return color * ex + in;
}

fn intersect(origin: vec3<f32>, dir: vec3<f32>, radius: f32) -> f32 {
    let a = dot(dir, dir);
    let b = dot(origin, dir);
    let c = dot(origin, origin) - radius * radius;
    let sqrt_d = sqrt(b * b - a * c);
    return (-b + sqrt_d) / a;
}

fn phase_ray(cos_theta: f32) -> f32 {
    let n = 3.0 * (1.0 + cos_theta * cos_theta);
    let d = 16.0 * PI;
    return n / d;
}

fn phase_mie(cos_theta: f32) -> f32 {
    let n = 3.0 * (1.0 - a.g * a.g) * (1.0 + cos_theta * cos_theta);
    let d = 8.0 * PI * (2.0 + a.g * a.g) * pow(1.0 + a.g * a.g - 2.0 * a.g * cos_theta, 1.5);
    return n / d;
}

fn density(coords: vec3<f32>) -> vec3<f32> {
    let height = height(coords);
    let ray = exp(-height / a.h_ray);
    let mie = exp(-height / a.h_mie);
    let denom = (a.h_ab - height) / a.ab_falloff;
    let ab = ray / (1.0 + denom * denom);
    return vec3(ray, mie, ab);
}

fn height(coords: vec3<f32>) -> f32 {
    return length(coords) - a.r_planet;
}

const PI = 3.14159265358979323846264338327950288;
