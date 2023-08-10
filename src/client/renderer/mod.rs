pub mod effect;
pub mod mesh;
pub mod program;
pub mod texture;
pub mod uniform;

use super::event_loop::{Event, EventHandler};
use std::mem;
use winit::{
    dpi::PhysicalSize,
    event::{StartCause, WindowEvent},
    window::Window as RawWindow,
};

pub struct Renderer {
    pub surface: wgpu::Surface,
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    pub config: wgpu::SurfaceConfiguration,
    is_resized: bool,
}

impl Renderer {
    pub async fn new(window: &RawWindow) -> Self {
        let PhysicalSize { width, height } = window.inner_size();
        let instance = wgpu::Instance::default();
        let surface = unsafe {
            instance
                .create_surface(window)
                .expect("surface should be creatable")
        };
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: Some(&surface),
                ..Default::default()
            })
            .await
            .expect("adapter should be available");
        let (device, queue) = adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    features: wgpu::Features::PUSH_CONSTANTS
                        | wgpu::Features::TEXTURE_BINDING_ARRAY
                        | wgpu::Features::SAMPLED_TEXTURE_AND_STORAGE_BUFFER_ARRAY_NON_UNIFORM_INDEXING,
                    limits: wgpu::Limits {
                        max_push_constant_size: 128,
                        ..Default::default()
                    },
                    ..Default::default()
                },
                None,
            )
            .await
            .expect("device should be available");
        let config = surface
            .get_default_config(&adapter, width, height)
            .expect("surface should be supported by adapter");
        Self {
            surface,
            device,
            queue,
            config,
            is_resized: false,
        }
    }

    pub fn recreate_surface(&self) {
        self.surface.configure(&self.device, &self.config);
    }
}

impl EventHandler for Renderer {
    type Context<'a> = ();

    fn handle(&mut self, event: &Event, _: Self::Context<'_>) {
        match event {
            Event::NewEvents(StartCause::Init) => {
                self.recreate_surface();
            }
            Event::WindowEvent {
                event:
                    WindowEvent::Resized(PhysicalSize { width, height })
                    | WindowEvent::ScaleFactorChanged {
                        new_inner_size: PhysicalSize { width, height },
                        ..
                    },
                ..
            } if *width != 0 && *height != 0 => {
                self.config.width = *width;
                self.config.height = *height;
                self.is_resized = true;
            }
            Event::MainEventsCleared => {
                if mem::take(&mut self.is_resized) {
                    self.recreate_surface();
                }
            }
            _ => {}
        }
    }
}
