use crate::{
    client::{
        event_loop::{Event, EventHandler},
        renderer::{effect::PostProcessor, program::Program, uniform::Uniform, Renderer},
    },
    server::{game::clock::Time, ServerEvent},
    shared::color::Float3,
};
use bytemuck::{Pod, Zeroable};
use nalgebra::{matrix, vector, Matrix3x4, Vector3};
use serde::Deserialize;
use std::{f32::consts::PI, fs};

pub struct Atmosphere {
    uniform: Uniform<AtmosphereUniformData>,
    program: Program,
    settings: AtmosphereSettings,
    updated_time: Option<Time>,
}

impl Atmosphere {
    pub fn new(renderer: &Renderer, player_bind_group_layout: &wgpu::BindGroupLayout) -> Self {
        let uniform = Uniform::uninit_mut(renderer, wgpu::ShaderStages::FRAGMENT);
        let program = Program::new(
            renderer,
            wgpu::include_wgsl!("../../../assets/shaders/atmosphere.wgsl"),
            &[],
            &[player_bind_group_layout, uniform.bind_group_layout()],
            &[],
            PostProcessor::FORMAT,
            None,
            None,
            None,
        );
        let settings = AtmosphereSettings::new();
        Self {
            uniform,
            program,
            settings,
            updated_time: Some(Default::default()),
        }
    }

    pub fn draw(
        &self,
        view: &wgpu::TextureView,
        encoder: &mut wgpu::CommandEncoder,
        player_bind_group: &wgpu::BindGroup,
    ) {
        let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: None,
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Load,
                    store: true,
                },
            })],
            depth_stencil_attachment: None,
        });
        self.program.bind(
            &mut render_pass,
            [player_bind_group, self.uniform.bind_group()],
        );
        render_pass.draw(0..3, 0..1);
    }
}

impl EventHandler for Atmosphere {
    type Context<'a> = &'a Renderer;

    fn handle(&mut self, event: &Event, renderer: Self::Context<'_>) {
        match event {
            Event::UserEvent(ServerEvent::TimeUpdated(timestamp)) => {
                self.updated_time = Some(*timestamp);
            }
            Event::MainEventsCleared => {
                if let Some(time) = self.updated_time.take() {
                    self.uniform.set(
                        renderer,
                        &AtmosphereUniformData::new(time.sun_dir(), self.settings.turbidity),
                    );
                }
            }
            _ => {}
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy, Zeroable, Pod)]
struct AtmosphereUniformData {
    sun_dir: Float3,
    a: Float3,
    b: Float3,
    c: Float3,
    d: Float3,
    e: Float3,
    z: Float3,
}

impl AtmosphereUniformData {
    fn new(sun_dir: Vector3<f32>, turbidity: f32) -> Self {
        let a = vector![-0.0193, -0.0167, 0.1787] * turbidity + vector![-0.2592, -0.2608, -1.4630];
        let b = vector![-0.0665, -0.0950, -0.3554] * turbidity + vector![0.0008, 0.0092, 0.4275];
        let c = vector![-0.0004, -0.0079, -0.0227] * turbidity + vector![0.2125, 0.2102, 5.3251];
        let d = vector![-0.0641, -0.0441, 0.1206] * turbidity + vector![-0.8989, -1.6537, -2.5771];
        let e = vector![-0.0033, -0.0109, -0.0670] * turbidity + vector![0.0452, 0.0529, 0.3703];
        let sun_theta = sun_dir.y.clamp(0.0, 1.0).acos();
        let z = vector![
            Self::zenith_chromacity(
                sun_theta,
                turbidity,
                matrix![
                    0.00166, -0.00375, 0.00209, 0.00000;
                    -0.02903, 0.06377, -0.03202, 0.00394;
                    0.11693, -0.21196, 0.06052, 0.25886
                ],
            ),
            Self::zenith_chromacity(
                sun_theta,
                turbidity,
                matrix![
                    0.00275, -0.00610, 0.00317, 0.00000;
                    -0.04214, 0.08970, -0.04153, 0.00516;
                    0.15346, -0.26756, 0.06670, 0.26688
                ],
            ),
            Self::zenith_luminance(sun_theta, turbidity)
        ]
        .component_div(&Self::perez(0.0, sun_theta, a, b, c, d, e));
        Self {
            sun_dir: sun_dir.into(),
            a: a.into(),
            b: b.into(),
            c: c.into(),
            d: d.into(),
            e: e.into(),
            z: z.into(),
        }
    }

    fn zenith_chromacity(sun_theta: f32, turbidity: f32, c: Matrix3x4<f32>) -> f32 {
        let turbidity_v = vector![turbidity.powi(2), turbidity, 1.0];
        let theta_v = vector![sun_theta.powi(3), sun_theta.powi(2), sun_theta, 1.0];
        turbidity_v.dot(&(c * theta_v))
    }

    fn zenith_luminance(sun_theta: f32, turbidity: f32) -> f32 {
        let chi = (4.0 / 9.0 - turbidity / 120.0) * (PI - 2.0 * sun_theta);
        (4.0453 * turbidity - 4.9710) * chi.tan() - 0.2155 * turbidity + 2.4192
    }

    fn perez(
        theta: f32,
        gamma: f32,
        a: Vector3<f32>,
        b: Vector3<f32>,
        c: Vector3<f32>,
        d: Vector3<f32>,
        e: Vector3<f32>,
    ) -> Vector3<f32> {
        Vector3::from_fn(|i, _| {
            (1.0 + a[i] * (b[i] / theta.cos()).exp())
                * (1.0 + c[i] * (d[i] * gamma).exp() + e[i] * gamma.cos().powi(2))
        })
    }
}

#[derive(Deserialize)]
struct AtmosphereSettings {
    turbidity: f32,
}

impl AtmosphereSettings {
    fn new() -> Self {
        toml::from_str(&fs::read_to_string("assets/atmosphere.toml").expect("file should exist"))
            .expect("file should be valid")
    }
}
