use crate::{
    client::{
        event_loop::{Event, EventHandler},
        renderer::{PostProcessor, Program, Renderer, Uniform},
    },
    color::{Float3, Rgb},
    server::ServerEvent,
};
use bytemuck::{Pod, Zeroable};
use nalgebra::{point, Point3};
use std::f32::consts::TAU;

pub struct Sky {
    atmosphere: Atmosphere,
    uniform: Uniform<SkyUniformData>,
    updated_time: Option<f32>,
}

impl Sky {
    pub fn new(renderer: &Renderer, player_bind_group_layout: &wgpu::BindGroupLayout) -> Self {
        let uniform = Uniform::new(renderer, wgpu::ShaderStages::VERTEX_FRAGMENT);
        let atmosphere = Atmosphere::new(
            renderer,
            player_bind_group_layout,
            uniform.bind_group_layout(),
        );
        Self {
            atmosphere,
            uniform,
            updated_time: Some(0.0),
        }
    }

    pub fn bind_group_layout(&self) -> &wgpu::BindGroupLayout {
        self.uniform.bind_group_layout()
    }

    pub fn bind_group(&self) -> &wgpu::BindGroup {
        self.uniform.bind_group()
    }

    pub fn draw(
        &self,
        view: &wgpu::TextureView,
        encoder: &mut wgpu::CommandEncoder,
        player_bind_group: &wgpu::BindGroup,
    ) {
        self.atmosphere.draw(
            &mut encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
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
            }),
            player_bind_group,
            self.uniform.bind_group(),
        );
    }

    fn sun_coords(time: f32) -> Point3<f32> {
        let theta = time * TAU;
        point![theta.cos(), theta.sin(), 0.0]
    }
}

impl EventHandler for Sky {
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
                        &SkyUniformData::new(Self::sun_coords(time), Rgb::new(0.15, 0.15, 0.3)),
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
struct SkyUniformData {
    sun_coords: Float3,
    light_intensity: Float3,
}

impl SkyUniformData {
    fn new(sun_coords: Point3<f32>, light_intensity: Rgb<f32>) -> Self {
        Self {
            sun_coords: sun_coords.into(),
            light_intensity: light_intensity.into(),
        }
    }
}

struct Atmosphere(Program);

impl Atmosphere {
    fn new(
        renderer: &Renderer,
        player_bind_group_layout: &wgpu::BindGroupLayout,
        sky_bind_group_layout: &wgpu::BindGroupLayout,
    ) -> Self {
        Self(Program::new(
            renderer,
            wgpu::include_wgsl!("../../../assets/shaders/atmosphere.wgsl"),
            &[],
            &[player_bind_group_layout, sky_bind_group_layout],
            &[],
            PostProcessor::FORMAT,
            None,
            None,
            None,
        ))
    }

    #[rustfmt::skip]
    fn draw<'a>(
        &'a self,
        render_pass: &mut wgpu::RenderPass<'a>,
        player_bind_group: &'a wgpu::BindGroup,
        sky_bind_group: &'a wgpu::BindGroup,
    ) {
        self.0.bind(render_pass, [player_bind_group, sky_bind_group]);
        render_pass.draw(0..6, 0..1);
    }
}
