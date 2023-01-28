mod crosshair;

use super::output::Output;
use crate::client::renderer::Renderer;
use crosshair::Crosshair;

pub struct Gui {
    crosshair: Crosshair,
}

impl Gui {
    pub fn new(renderer: &Renderer) -> Self {
        Self {
            crosshair: Crosshair::new(renderer),
        }
    }

    pub fn draw(&self, output: &Output, encoder: &mut wgpu::CommandEncoder) {
        self.crosshair
            .draw(&mut encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: None,
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: output.view(),
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: true,
                    },
                })],
                depth_stencil_attachment: None,
            }));
    }
}
