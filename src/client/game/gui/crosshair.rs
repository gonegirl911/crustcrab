use crate::client::renderer::Renderer;

pub struct Crosshair {}

impl Crosshair {
    pub fn new(renderer: &Renderer) -> Self {
        Self {}
    }

    pub fn draw(&self, render_pass: &mut wgpu::RenderPass) {}
}
