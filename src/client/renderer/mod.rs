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
use std::{
    mem,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};
use winit::{dpi::PhysicalSize, event::WindowEvent};

pub struct Renderer {
    instance: wgpu::Instance,
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    is_device_lost: Arc<AtomicBool>,
}

impl Renderer {
    pub async fn new(window: Arc<RawWindow>) -> (Self, Surface) {
        let PhysicalSize { width, height } = window.surface_size();
        let instance = wgpu::Instance::default();
        let surface = Surface::create_raw(&instance, window);
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: Some(&surface),
                ..Default::default()
            })
            .await
            .expect("adapter should be available");
        let config = surface
            .get_default_config(&adapter, width, height)
            .unwrap_or_else(|| unreachable!());
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
        let is_device_lost = Arc::new(AtomicBool::new(false));

        device.set_device_lost_callback({
            let is_device_lost = is_device_lost.clone();
            move |reason, _| {
                if reason != wgpu::DeviceLostReason::Destroyed {
                    is_device_lost.store(true, Ordering::Relaxed);
                }
            }
        });

        (
            Self {
                instance,
                device,
                queue,
                is_device_lost,
            },
            Surface {
                surface,
                config,
                should_resize: true,
                is_resized: false,
            },
        )
    }

    pub fn is_device_lost(&self) -> bool {
        self.is_device_lost.load(Ordering::Relaxed)
    }
}

pub struct Surface {
    surface: wgpu::Surface<'static>,
    pub config: wgpu::SurfaceConfiguration,
    should_resize: bool,
    pub is_resized: bool,
}

impl Surface {
    pub fn get_current_texture(&self) -> wgpu::CurrentSurfaceTexture {
        self.surface.get_current_texture()
    }

    pub fn width(&self) -> f32 {
        self.config.width as f32
    }

    pub fn height(&self) -> f32 {
        self.config.height as f32
    }

    pub fn configure(&self, Renderer { device, .. }: &Renderer) {
        self.surface.configure(device, &self.config);
    }

    pub fn recreate(&mut self, window: Arc<RawWindow>, Renderer { instance, .. }: &Renderer) {
        self.surface = Self::create_raw(instance, window);
    }

    fn create_raw(instance: &wgpu::Instance, window: Arc<RawWindow>) -> wgpu::Surface<'static> {
        instance
            .create_surface(window)
            .expect("surface should be creatable")
    }
}

impl EventHandler for Surface {
    type Context<'a> = (&'a RawWindow, &'a Renderer);

    fn handle(&mut self, event: &Event, (window, renderer): Self::Context<'_>) {
        self.is_resized = false;

        if let Event::WindowEvent(event) = event {
            match event {
                WindowEvent::SurfaceResized(_) | WindowEvent::ScaleFactorChanged { .. } => {
                    self.should_resize = true;
                }
                #[allow(clippy::collapsible_match)]
                WindowEvent::RedrawRequested => {
                    if mem::take(&mut self.should_resize) {
                        let PhysicalSize { width, height } = window.surface_size();
                        if width != 0 && height != 0 {
                            self.config.width = width;
                            self.config.height = height;
                            self.configure(renderer);
                            self.is_resized = true;
                        }
                    }
                }
                _ => {}
            }
        }
    }
}
