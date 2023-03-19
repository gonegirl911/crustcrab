use crate::client::{
    event_loop::{Event, EventHandler},
    renderer::{Blit, Effect, InputOutputTexture, PostProcessor, Program, Renderer},
};

pub struct Atmosphere {
    texture: InputOutputTexture,
    program: Program,
    blit: Blit,
}

impl Atmosphere {
    pub fn new(
        renderer: &Renderer,
        player_bind_group_layout: &wgpu::BindGroupLayout,
        sky_bind_group_layout: &wgpu::BindGroupLayout,
    ) -> Self {
        let texture = InputOutputTexture::new(renderer, PostProcessor::FORMAT);
        let program = Program::new(
            renderer,
            wgpu::include_wgsl!("../../../assets/shaders/atmosphere.wgsl"),
            &[],
            &[player_bind_group_layout, sky_bind_group_layout],
            &[],
            PostProcessor::FORMAT,
            None,
            None,
            None,
        );
        let blit = Blit::new(renderer, texture.bind_group_layout(), PostProcessor::FORMAT);
        Self {
            texture,
            program,
            blit,
        }
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

    #[rustfmt::skip]
    pub fn draw(
        &self,
        view: &wgpu::TextureView,
        encoder: &mut wgpu::CommandEncoder,
        player_bind_group: &wgpu::BindGroup,
        sky_bind_group: &wgpu::BindGroup,
    ) {
        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: None,
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: self.view(),
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: true,
                    },
                })],
                depth_stencil_attachment: None,
            });
            self.program.bind(
                &mut render_pass,
                [player_bind_group, sky_bind_group],
            );
            render_pass.draw(0..6, 0..1);
        }
        self.blit.draw(
            &mut encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: None,
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: true,
                    },
                })],
                depth_stencil_attachment: None,
            }),
            self.bind_group(),
        );
    }
}

impl EventHandler for Atmosphere {
    type Context<'a> = &'a Renderer;

    fn handle(&mut self, event: &Event, renderer: Self::Context<'_>) {
        self.texture.handle(event, renderer);
    }
}
