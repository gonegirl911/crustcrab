use super::{program::Program, texture::screen::InputOutputTextureArray, Renderer};
use crate::client::event_loop::{Event, EventHandler};

pub struct PostProcessor {
    textures: InputOutputTextureArray<2>,
    blit: Blit,
}

impl PostProcessor {
    pub const FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba16Float;

    pub fn new(renderer @ Renderer { config, .. }: &Renderer) -> Self {
        let textures = InputOutputTextureArray::new(renderer, Self::FORMAT);
        let blit = Blit::new(renderer, textures.bind_group_layout(), config.format);
        Self { textures, blit }
    }

    pub fn view(&self) -> &wgpu::TextureView {
        self.textures.view(0)
    }

    pub fn bind_group_layout(&self) -> &wgpu::BindGroupLayout {
        self.textures.bind_group_layout()
    }

    pub fn apply<E: Effect>(&mut self, encoder: &mut wgpu::CommandEncoder, effect: &E) {
        self.apply_raw(|view, bind_group| {
            effect.draw(
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
                bind_group,
            );
        });
    }

    pub fn apply_raw<E>(&mut self, effect: E)
    where
        E: FnOnce(&wgpu::TextureView, &wgpu::BindGroup),
    {
        effect(self.textures.view(1), self.textures.bind_group(0));
        self.textures.swap();
    }

    pub fn draw(&self, view: &wgpu::TextureView, encoder: &mut wgpu::CommandEncoder) {
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
            self.textures.bind_group(0),
        );
    }
}

impl EventHandler for PostProcessor {
    type Context<'a> = &'a Renderer;

    fn handle(&mut self, event: &Event, renderer: Self::Context<'_>) {
        self.textures.handle(event, renderer);
    }
}

pub struct Blit(Program);

impl Blit {
    pub fn new(
        renderer: &Renderer,
        input_bind_group_layout: &wgpu::BindGroupLayout,
        format: wgpu::TextureFormat,
    ) -> Self {
        Self(Program::new(
            renderer,
            wgpu::include_wgsl!("../../../assets/shaders/blit.wgsl"),
            &[],
            &[input_bind_group_layout],
            &[],
            format,
            None,
            None,
            None,
        ))
    }
}

impl Effect for Blit {
    fn draw<'a>(
        &'a self,
        render_pass: &mut wgpu::RenderPass<'a>,
        input_bind_group: &'a wgpu::BindGroup,
    ) {
        self.0.bind(render_pass, [input_bind_group]);
        render_pass.draw(0..3, 0..1);
    }
}

pub struct Aces(Program);

impl Aces {
    pub fn new(
        renderer: &Renderer,
        input_bind_group_layout: &wgpu::BindGroupLayout,
        format: wgpu::TextureFormat,
    ) -> Self {
        Self(Program::new(
            renderer,
            wgpu::include_wgsl!("../../../assets/shaders/aces.wgsl"),
            &[],
            &[input_bind_group_layout],
            &[],
            format,
            None,
            None,
            None,
        ))
    }
}

impl Effect for Aces {
    fn draw<'a>(
        &'a self,
        render_pass: &mut wgpu::RenderPass<'a>,
        input_bind_group: &'a wgpu::BindGroup,
    ) {
        self.0.bind(render_pass, [input_bind_group]);
        render_pass.draw(0..3, 0..1);
    }
}

pub trait Effect {
    fn draw<'a>(
        &'a self,
        render_pass: &mut wgpu::RenderPass<'a>,
        input_bind_group: &'a wgpu::BindGroup,
    );
}
