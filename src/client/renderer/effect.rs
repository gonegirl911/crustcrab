use super::{
    Renderer,
    program::{Program, PushConstants},
    texture::screen::ScreenTextureArray,
    utils::read_wgsl,
};
use crate::client::event_loop::{Event, EventHandler};
use bytemuck::{Pod, Zeroable};

pub struct PostProcessor {
    textures: ScreenTextureArray<2>,
    blit: Blit,
}

impl PostProcessor {
    pub const FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba16Float;

    pub fn new(renderer @ Renderer { config, .. }: &Renderer) -> Self {
        let textures = ScreenTextureArray::new(renderer, Self::FORMAT);
        let blit = Blit::new(renderer, textures.bind_group_layout(), config.format);
        Self { textures, blit }
    }

    pub fn view(&self) -> &wgpu::TextureView {
        self.textures.view(0)
    }

    pub fn spare_view(&self) -> &wgpu::TextureView {
        self.textures.view(1)
    }

    pub fn bind_group_layout(&self) -> &wgpu::BindGroupLayout {
        self.textures.bind_group_layout()
    }

    fn bind_group(&self) -> &wgpu::BindGroup {
        self.textures.bind_group(0)
    }

    pub fn spare_bind_group(&self) -> &wgpu::BindGroup {
        self.textures.bind_group(1)
    }

    pub fn apply<E: Effect>(&mut self, encoder: &mut wgpu::CommandEncoder, effect: &E) {
        self.apply_raw(|view, bind_group| {
            effect.draw(
                &mut encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view,
                        depth_slice: None,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Load,
                            store: wgpu::StoreOp::Store,
                        },
                    })],
                    ..Default::default()
                }),
                bind_group,
            );
        });
    }

    pub fn apply_raw<E>(&mut self, effect: E)
    where
        E: FnOnce(&wgpu::TextureView, &wgpu::BindGroup),
    {
        effect(self.spare_view(), self.bind_group());
        self.textures.swap();
    }

    pub fn draw(&self, view: &wgpu::TextureView, encoder: &mut wgpu::CommandEncoder) {
        self.blit.draw(
            &mut encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    },
                })],
                ..Default::default()
            }),
            self.bind_group(),
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
        Self(
            Program::builder()
                .renderer(renderer)
                .shader_desc(read_wgsl("assets/shaders/blit.wgsl"))
                .bind_group_layouts(&[input_bind_group_layout])
                .format(format)
                .build(),
        )
    }
}

impl Effect for Blit {
    fn draw(&self, render_pass: &mut wgpu::RenderPass, input_bind_group: &wgpu::BindGroup) {
        self.0.bind(render_pass, [input_bind_group]);
        render_pass.draw(0..3, 0..1);
    }
}

pub struct Blender(Program);

impl Blender {
    pub fn new(
        renderer: &Renderer,
        input_bind_group_layout: &wgpu::BindGroupLayout,
        format: wgpu::TextureFormat,
    ) -> Self {
        Self(
            Program::builder()
                .renderer(renderer)
                .shader_desc(read_wgsl("assets/shaders/blender.wgsl"))
                .bind_group_layouts(&[input_bind_group_layout])
                .push_constant_ranges(&[wgpu::PushConstantRange {
                    stages: wgpu::ShaderStages::FRAGMENT,
                    range: 0..size_of::<BlenderPushConstants>() as u32,
                }])
                .format(format)
                .blend(wgpu::BlendState::ALPHA_BLENDING)
                .build(),
        )
    }

    pub fn draw(
        &self,
        view: &wgpu::TextureView,
        encoder: &mut wgpu::CommandEncoder,
        input_bind_group: &wgpu::BindGroup,
        opacity: f32,
        should_clear: bool,
    ) {
        let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: if should_clear {
                        wgpu::LoadOp::Clear(Default::default())
                    } else {
                        wgpu::LoadOp::Load
                    },
                    store: wgpu::StoreOp::Store,
                },
            })],
            ..Default::default()
        });
        self.0.bind(&mut render_pass, [input_bind_group]);
        BlenderPushConstants::new(opacity).set(&mut render_pass);
        render_pass.draw(0..3, 0..1);
    }
}

#[repr(C)]
#[derive(Clone, Copy, Zeroable, Pod)]
struct BlenderPushConstants {
    opacity: f32,
}

impl BlenderPushConstants {
    fn new(opacity: f32) -> Self {
        Self { opacity }
    }
}

impl PushConstants for BlenderPushConstants {
    const STAGES: wgpu::ShaderStages = wgpu::ShaderStages::FRAGMENT;
}

pub struct Aces(Program);

impl Aces {
    pub fn new(
        renderer: &Renderer,
        input_bind_group_layout: &wgpu::BindGroupLayout,
        format: wgpu::TextureFormat,
    ) -> Self {
        Self(
            Program::builder()
                .renderer(renderer)
                .shader_desc(read_wgsl("assets/shaders/aces.wgsl"))
                .bind_group_layouts(&[input_bind_group_layout])
                .format(format)
                .build(),
        )
    }
}

impl Effect for Aces {
    fn draw(&self, render_pass: &mut wgpu::RenderPass, input_bind_group: &wgpu::BindGroup) {
        self.0.bind(render_pass, [input_bind_group]);
        render_pass.draw(0..3, 0..1);
    }
}

pub trait Effect {
    fn draw(&self, render_pass: &mut wgpu::RenderPass, input_bind_group: &wgpu::BindGroup);
}
