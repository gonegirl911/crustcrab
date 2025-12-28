pub mod buffer;
pub mod effect;
pub mod program;
pub mod texture;
pub mod uniform;
pub mod utils;

use super::{
    event_loop::{Event, EventHandler},
    window::RawWindow,
};
use std::{mem, sync::Arc};
use winit::{dpi::PhysicalSize, event::WindowEvent};

pub struct Renderer {
    pub surface: wgpu::Surface<'static>,
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    pub config: wgpu::SurfaceConfiguration,
    should_resize_surface: bool,
    pub is_surface_resized: bool,
}

impl Renderer {
    pub async fn new(window: Arc<RawWindow>) -> Self {
        let PhysicalSize { width, height } = window.surface_size();
        let instance = wgpu::Instance::default();
        let surface = instance
            .create_surface(window)
            .expect("surface should be creatable");
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: Some(&surface),
                ..Default::default()
            })
            .await
            .expect("adapter should be available");
        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                required_features: wgpu::Features::IMMEDIATES
                    | wgpu::Features::SAMPLED_TEXTURE_AND_STORAGE_BUFFER_ARRAY_NON_UNIFORM_INDEXING
                    | wgpu::Features::TEXTURE_BINDING_ARRAY,
                required_limits: wgpu::Limits {
                    max_binding_array_elements_per_shader_stage: 6,
                    max_immediate_size: 68,
                    ..Default::default()
                },
                ..Default::default()
            })
            .await
            .expect("device should be available");
        let config = surface
            .get_default_config(&adapter, width, height)
            .unwrap_or_else(|| unreachable!());
        Self {
            surface,
            device,
            queue,
            config,
            should_resize_surface: true,
            is_surface_resized: false,
        }
    }

    pub fn aspect(&self) -> f32 {
        self.config.width as f32 / self.config.height as f32
    }

    pub fn recreate_surface(&self) {
        self.surface.configure(&self.device, &self.config);
    }

    fn resize_surface(&mut self, PhysicalSize { width, height }: PhysicalSize<u32>) -> bool {
        if width == 0 || height == 0 {
            false
        } else {
            self.config.width = width;
            self.config.height = height;
            self.recreate_surface();
            true
        }
    }
}

impl EventHandler for Renderer {
    type Context<'a> = &'a RawWindow;

    fn handle(&mut self, event: &Event, window: Self::Context<'_>) {
        self.is_surface_resized = false;

        if let Event::WindowEvent(event) = event {
            match event {
                WindowEvent::SurfaceResized(_) | WindowEvent::ScaleFactorChanged { .. } => {
                    self.should_resize_surface = true;
                }
                WindowEvent::RedrawRequested => {
                    self.is_surface_resized = mem::take(&mut self.should_resize_surface)
                        && self.resize_surface(window.surface_size());
                }
                _ => {}
            }
        }
    }
}
