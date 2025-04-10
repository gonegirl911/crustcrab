pub mod buffer;
pub mod effect;
pub mod program;
pub mod texture;
pub mod uniform;
pub mod utils;

use super::{
    event_loop::{Event, EventHandler},
    window::{RawWindow, Window},
};
use std::mem;
use winit::{dpi::PhysicalSize, event::WindowEvent};

pub struct Renderer {
    pub surface: wgpu::Surface<'static>,
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    pub config: wgpu::SurfaceConfiguration,
    should_resize: bool,
    pub is_resized: bool,
}

impl Renderer {
    pub async fn new(window: Window) -> Self {
        let PhysicalSize { width, height } = window.inner_size();
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
                required_features: wgpu::Features::PUSH_CONSTANTS
                    | wgpu::Features::TEXTURE_BINDING_ARRAY
                    | wgpu::Features::SAMPLED_TEXTURE_AND_STORAGE_BUFFER_ARRAY_NON_UNIFORM_INDEXING,
                required_limits: wgpu::Limits {
                    max_push_constant_size: 68,
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
            should_resize: true,
            is_resized: false,
        }
    }

    pub fn recreate_surface(&self) {
        self.surface.configure(&self.device, &self.config);
    }

    pub fn aspect(&self) -> f32 {
        self.config.width as f32 / self.config.height as f32
    }

    fn resize(&mut self, PhysicalSize { width, height }: PhysicalSize<u32>) -> bool {
        if width != 0 && height != 0 {
            self.config.width = width;
            self.config.height = height;
            self.recreate_surface();
            true
        } else {
            false
        }
    }
}

impl EventHandler for Renderer {
    type Context<'a> = &'a RawWindow;

    fn handle(&mut self, event: &Event, window: Self::Context<'_>) {
        self.is_resized = false;

        if let Event::WindowEvent { event, .. } = event {
            match event {
                WindowEvent::Resized(_) | WindowEvent::ScaleFactorChanged { .. } => {
                    self.should_resize = true;
                }
                WindowEvent::RedrawRequested => {
                    if mem::take(&mut self.should_resize) {
                        self.is_resized = self.resize(window.inner_size());
                    }
                }
                _ => {}
            }
        }
    }
}
