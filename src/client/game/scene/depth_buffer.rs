use crate::client::{
    event_loop::{Event, EventHandler},
    renderer::{Renderer, ScreenTexture},
};

pub struct DepthBuffer {
    texture: ScreenTexture,
}

impl DepthBuffer {
    pub const FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;

    pub fn new(renderer: &Renderer) -> Self {
        Self {
            texture: ScreenTexture::new(
                renderer,
                Self::FORMAT,
                wgpu::TextureUsages::RENDER_ATTACHMENT,
            ),
        }
    }

    pub fn view(&self) -> &wgpu::TextureView {
        self.texture.view()
    }
}

impl EventHandler for DepthBuffer {
    type Context<'a> = &'a Renderer;

    fn handle(&mut self, event: &Event, renderer: Self::Context<'_>) {
        self.texture.handle(event, renderer);
    }
}
