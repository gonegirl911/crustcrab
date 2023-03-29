use super::player::Player;
use crate::{
    client::{
        event_loop::{Event, EventHandler},
        renderer::{PostProcessor, Program, Renderer, Uniform},
    },
    color::{Float3, Rgb},
    server::ServerEvent,
};
use bytemuck::{Pod, Zeroable};
use nalgebra::{vector, Vector3};
use once_cell::sync::Lazy;
use serde::Deserialize;
use std::{f32::consts::TAU, fs};

pub struct Atmosphere {
    uniform: Uniform<AtmosphereUniformData>,
    program: Program,
    updated_time: Option<f32>,
}

impl Atmosphere {
    pub fn new(
        renderer: &Renderer,
        player_bind_group_layout: &wgpu::BindGroupLayout,
        input_bind_group_layout: &wgpu::BindGroupLayout,
        depth_bind_group_layout: &wgpu::BindGroupLayout,
    ) -> Self {
        let uniform = Uniform::new(renderer, wgpu::ShaderStages::FRAGMENT);
        let program = Program::new(
            renderer,
            wgpu::include_wgsl!("../../../assets/shaders/atmosphere.wgsl"),
            &[],
            &[
                player_bind_group_layout,
                uniform.bind_group_layout(),
                input_bind_group_layout,
                depth_bind_group_layout,
            ],
            &[],
            PostProcessor::FORMAT,
            None,
            None,
            None,
        );
        Self {
            uniform,
            program,
            updated_time: Some(0.0),
        }
    }

    pub fn draw(
        &self,
        view: &wgpu::TextureView,
        encoder: &mut wgpu::CommandEncoder,
        player_bind_group: &wgpu::BindGroup,
        input_bind_group: &wgpu::BindGroup,
        depth_bind_group: &wgpu::BindGroup,
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
            [
                player_bind_group,
                self.uniform.bind_group(),
                input_bind_group,
                depth_bind_group,
            ],
        );
        render_pass.draw(0..6, 0..1);
    }

    fn sun_dir(time: f32) -> Vector3<f32> {
        let theta = time * TAU;
        vector![theta.cos(), theta.sin(), 0.0]
    }
}

impl EventHandler for Atmosphere {
    type Context<'a> = &'a Renderer;

    fn handle(&mut self, event: &Event, renderer: Self::Context<'_>) {
        match event {
            Event::UserEvent(ServerEvent::TimeUpdated { time }) => {
                self.updated_time = Some(*time);
            }
            Event::RedrawRequested(_) => {
                if let Some(time) = self.updated_time {
                    self.uniform
                        .write(renderer, &AtmosphereUniformData::new(Self::sun_dir(time)));
                }
            }
            Event::RedrawEventsCleared => {
                self.updated_time = None;
            }
            _ => {}
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy, Zeroable, Pod)]
struct AtmosphereUniformData {
    sun_dir: Float3,
    sun_intensity: Float3,
    sc_air: Float3,
    sc_haze: Float3,
    ex: Float3,
    ex_air: Float3,
    ex_haze: Rgb<f32>,
    s_air: f32,
    s_haze: f32,
    g: f32,
    padding: [f32; 2],
}

impl AtmosphereUniformData {
    fn new(sun_dir: Vector3<f32>) -> Self {
        Self {
            sun_dir: sun_dir.into(),
            sun_intensity: SETTINGS.sun_intensity(sun_dir).into(),
            sc_air: SETTINGS.sc_air.into(),
            sc_haze: SETTINGS.sc_haze.into(),
            ex: SETTINGS.ex().into(),
            ex_air: SETTINGS.ex_air().into(),
            ex_haze: SETTINGS.ex_haze(),
            s_air: SETTINGS.s_air,
            s_haze: SETTINGS.s_haze,
            g: SETTINGS.g,
            padding: [0.0; 2],
        }
    }
}

#[derive(Deserialize)]
struct AtmosphereSettings {
    sun_intensity: Rgb<f32>,
    ab_air: Rgb<f32>,
    ab_haze: Rgb<f32>,
    sc_air: Rgb<f32>,
    sc_haze: Rgb<f32>,
    s_air: f32,
    s_haze: f32,
    g: f32,
}

impl AtmosphereSettings {
    fn sun_intensity(&self, sun_dir: Vector3<f32>) -> Rgb<f32> {
        let cos_theta = sun_dir.dot(&Player::WORLD_UP);
        let theta = cos_theta.acos();
        let diff = 93.885 - theta.to_degrees();
        if diff >= 1.0 {
            let m = 1.0 / (cos_theta + 0.15 * diff.powf(-1.253));
            let s_air = self.s_air * m;
            let s_haze = self.s_haze * m;
            self.sun_intensity * (-self.ex_air() * s_air - self.ex_haze() * s_haze).exp()
        } else {
            Rgb::splat(0.0)
        }
    }

    fn ex(&self) -> Rgb<f32> {
        self.ex_air() + self.ex_haze()
    }

    fn ex_air(&self) -> Rgb<f32> {
        self.ab_air + self.sc_air
    }

    fn ex_haze(&self) -> Rgb<f32> {
        self.ab_haze + self.sc_haze
    }
}

static SETTINGS: Lazy<AtmosphereSettings> = Lazy::new(|| {
    toml::from_str(&fs::read_to_string("assets/atmosphere.toml").expect("file should exist"))
        .expect("file should be valid")
});
