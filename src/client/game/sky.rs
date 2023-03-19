use crate::{
    client::{
        event_loop::{Event, EventHandler},
        renderer::{Renderer, Uniform},
    },
    color::{Float3, Rgb},
    server::ServerEvent,
};
use bytemuck::{Pod, Zeroable};
use nalgebra::{point, Point3};
use std::f32::consts::TAU;

pub struct Sky {
    uniform: Uniform<SkyUniformData>,
    updated_time: Option<f32>,
}

impl Sky {
    pub fn new(renderer: &Renderer) -> Self {
        Self {
            uniform: Uniform::new(renderer, wgpu::ShaderStages::VERTEX_FRAGMENT),
            updated_time: Some(0.0),
        }
    }

    pub fn bind_group_layout(&self) -> &wgpu::BindGroupLayout {
        self.uniform.bind_group_layout()
    }

    pub fn bind_group(&self) -> &wgpu::BindGroup {
        self.uniform.bind_group()
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
