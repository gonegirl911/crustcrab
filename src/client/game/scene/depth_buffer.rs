use crate::client::{
    event_loop::{Event, EventHandler},
    renderer::Renderer,
};
use winit::{dpi::PhysicalSize, event::WindowEvent};

pub struct DepthBuffer {
    view: wgpu::TextureView,
    is_resized: bool,
}

impl DepthBuffer {
    pub const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;

    pub fn new(Renderer { device, config, .. }: &Renderer) -> Self {
        Self {
            view: device
                .create_texture(&wgpu::TextureDescriptor {
                    label: None,
                    size: wgpu::Extent3d {
                        width: config.width,
                        height: config.height,
                        depth_or_array_layers: 1,
                    },
                    mip_level_count: 1,
                    sample_count: 1,
                    dimension: wgpu::TextureDimension::D2,
                    format: Self::DEPTH_FORMAT,
                    usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                        | wgpu::TextureUsages::TEXTURE_BINDING,
                    view_formats: &[],
                })
                .create_view(&Default::default()),
            is_resized: false,
        }
    }

    pub fn view(&self) -> &wgpu::TextureView {
        &self.view
    }
}

impl EventHandler for DepthBuffer {
    type Context<'a> = &'a Renderer;

    fn handle(&mut self, event: &Event, renderer: Self::Context<'_>) {
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
                self.is_resized = true;
            }
            Event::RedrawRequested(_) if self.is_resized => {
                *self = Self::new(renderer);
            }
            _ => {}
        }
    }
}
