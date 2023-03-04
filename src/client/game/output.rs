use crate::client::{
    event_loop::{Event, EventHandler},
    renderer::{InputOutputTexture, Program, Renderer},
};

pub struct Output {
    texture: InputOutputTexture,
    program: Program,
}

impl Output {
    pub fn new(renderer: &Renderer) -> Self {
        let texture = InputOutputTexture::new(renderer);
        let program = Program::new(
            renderer,
            wgpu::include_wgsl!("../../../assets/shaders/output.wgsl"),
            &[],
            &[texture.bind_group_layout()],
            &[],
            None,
            None,
            None,
            None,
        );
        Self { texture, program }
    }

    pub fn view(&self) -> &wgpu::TextureView {
        self.texture.view()
    }

    pub fn bind_group_layout(&self) -> &wgpu::BindGroupLayout {
        self.texture.bind_group_layout()
    }

    pub fn bind_group(&self) -> &wgpu::BindGroup {
        self.texture.bind_group()
    }

    pub fn draw(&self, view: &wgpu::TextureView, encoder: &mut wgpu::CommandEncoder) {
        let render_pass = &mut encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: None,
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                    store: true,
                },
            })],
            depth_stencil_attachment: None,
        });
        self.program.draw(render_pass, [self.texture.bind_group()]);
        render_pass.draw(0..3, 0..1);
    }
}

impl EventHandler for Output {
    type Context<'a> = &'a Renderer;

    fn handle(&mut self, event: &Event, renderer: Self::Context<'_>) {
        self.texture.handle(event, renderer);
    }
}
