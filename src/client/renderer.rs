use super::{
    event_loop::{Event, EventHandler},
    window::Window,
};
use winit::{dpi::PhysicalSize, event::WindowEvent};

pub struct Renderer {
    pub surface: wgpu::Surface,
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    pub config: wgpu::SurfaceConfiguration,
    is_resized: bool,
}

impl Renderer {
    pub async fn new(window: &Window) -> Self {
        let size = window.as_ref().inner_size();
        let instance = wgpu::Instance::new(wgpu::Backends::all());
        let surface = unsafe { instance.create_surface(window.as_ref()) };
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
            })
            .await
            .expect("wgpu adapter should be available");
        let (device, queue) = adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    label: None,
                    features: wgpu::Features::PUSH_CONSTANTS,
                    limits: wgpu::Limits {
                        max_push_constant_size: 128,
                        ..Default::default()
                    },
                },
                None,
            )
            .await
            .expect("wgpu device should be available");
        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: surface.get_supported_formats(&adapter)[0],
            width: size.width,
            height: size.height,
            present_mode: wgpu::PresentMode::Fifo,
            alpha_mode: wgpu::CompositeAlphaMode::Auto,
        };
        Self {
            surface,
            device,
            queue,
            config,
            is_resized: true,
        }
    }

    pub fn recreate_surface(&self) {
        self.surface.configure(&self.device, &self.config)
    }
}

impl EventHandler for Renderer {
    type Context<'a> = ();

    fn handle(&mut self, event: &Event, _: Self::Context<'_>) {
        match event {
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
            Event::RedrawRequested(_) if self.is_resized => {
                self.recreate_surface();
            }
            Event::RedrawEventsCleared => {
                self.is_resized = false;
            }
            _ => {}
        }
    }
}
