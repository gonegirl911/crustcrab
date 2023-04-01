use crate::{
    client::{
        event_loop::{Event, EventHandler},
        renderer::{PostProcessor, Program, Renderer, Uniform},
    },
    color::Rgb,
    server::ServerEvent,
};
use bytemuck::{Pod, Zeroable};
use nalgebra::{vector, Vector3};
use serde::Deserialize;
use std::{f32::consts::TAU, fs};

pub struct Atmosphere {
    uniform: Uniform<AtmosphereUniformData>,
    program: Program,
    settings: AtmosphereSettings,
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
        let settings = AtmosphereSettings::new();
        Self {
            uniform,
            program,
            settings,
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
                    self.uniform.write(
                        renderer,
                        &AtmosphereUniformData::new(Self::sun_dir(time), &self.settings),
                    );
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
    light_dir: Vector3<f32>,
    g: f32,
    light_intensity: Rgb<f32>,
    h_ray: f32,
    b_ray: Rgb<f32>,
    h_mie: f32,
    b_mie: Rgb<f32>,
    h_ab: f32,
    b_ab: Rgb<f32>,
    ab_falloff: f32,
    r_planet: f32,
    r_atmosphere: f32,
    primary_steps: u32,
    light_steps: u32,
}

impl AtmosphereUniformData {
    fn new(sun_dir: Vector3<f32>, settings: &AtmosphereSettings) -> Self {
        Self {
            light_dir: -sun_dir,
            g: settings.g,
            light_intensity: settings.light_intensity,
            h_ray: settings.h_ray,
            b_ray: settings.b_ray,
            h_mie: settings.h_mie,
            b_mie: settings.b_mie,
            h_ab: settings.h_ab,
            b_ab: settings.b_ab,
            ab_falloff: settings.ab_falloff,
            r_planet: settings.r_planet,
            r_atmosphere: settings.r_atmosphere,
            primary_steps: settings.primary_steps,
            light_steps: settings.light_steps,
        }
    }
}

#[derive(Deserialize)]
struct AtmosphereSettings {
    g: f32,
    light_intensity: Rgb<f32>,
    h_ray: f32,
    b_ray: Rgb<f32>,
    h_mie: f32,
    b_mie: Rgb<f32>,
    h_ab: f32,
    b_ab: Rgb<f32>,
    ab_falloff: f32,
    r_planet: f32,
    r_atmosphere: f32,
    primary_steps: u32,
    light_steps: u32,
}

impl AtmosphereSettings {
    fn new() -> Self {
        toml::from_str(&fs::read_to_string("assets/atmosphere.toml").expect("file should exist"))
            .expect("file should be valid")
    }
}
