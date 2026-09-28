use super::{
    Renderer, Surface,
    render_pipeline::RenderPipeline,
    texture::screen::ScreenTextureArray,
    utils::{Immediates, read_wgsl},
};
use crate::client::event_loop::{Event, EventHandler};
use bytemuck::{Pod, Zeroable};

pub struct PostProcessor {
    textures: ScreenTextureArray<2>,
    blit: Blit,
}

impl PostProcessor {
    pub const FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba16Float;

    pub fn new(renderer: &Renderer, surface: &Surface) -> Self {
        let textures = ScreenTextureArray::new(renderer, surface, Self::FORMAT);
        let blit = Blit::new(renderer, &textures.bind_group_layout, surface.config.format);
        Self { textures, blit }
    }

    pub fn view(&self) -> &wgpu::TextureView {
        &self.textures.views[0]
    }

    pub fn spare_view(&self) -> &wgpu::TextureView {
        &self.textures.views[1]
    }

    pub fn bind_group_layout(&self) -> &wgpu::BindGroupLayout {
        &self.textures.bind_group_layout
    }

    fn bind_group(&self) -> &wgpu::BindGroup {
        &self.textures.bind_groups[0]
    }

    pub fn spare_bind_group(&self) -> &wgpu::BindGroup {
        &self.textures.bind_groups[1]
    }

    pub fn step<E>(&mut self, effect: E)
    where
        E: FnOnce(&wgpu::TextureView, &wgpu::BindGroup),
    {
        effect(self.spare_view(), self.bind_group());
        self.textures.swap();
    }

    pub fn draw(&self, render_pass: &mut wgpu::RenderPass) {
        self.blit.draw(render_pass, self.bind_group());
    }
}

impl EventHandler for PostProcessor {
    type Context<'a> = (&'a Renderer, &'a Surface);

    fn handle(&mut self, event: &Event, (renderer, surface): Self::Context<'_>) {
        self.textures.handle(event, (renderer, surface));
    }
}

pub struct Blit(RenderPipeline);

impl Blit {
    pub fn new(
        renderer: &Renderer,
        input_bind_group_layout: &wgpu::BindGroupLayout,
        format: wgpu::TextureFormat,
    ) -> Self {
        Self(
            RenderPipeline::builder()
                .renderer(renderer)
                .shader_desc(read_wgsl("assets/shaders/blit.wgsl"))
                .bind_group_layouts(&[input_bind_group_layout])
                .format(format)
                .build(),
        )
    }

    pub fn draw(&self, render_pass: &mut wgpu::RenderPass, input_bind_group: &wgpu::BindGroup) {
        self.0.bind(render_pass, [input_bind_group]);
        render_pass.draw(0..3, 0..1);
    }
}

pub struct Blender(RenderPipeline);

impl Blender {
    pub fn new(
        renderer: &Renderer,
        input_bind_group_layout: &wgpu::BindGroupLayout,
        format: wgpu::TextureFormat,
    ) -> Self {
        Self(
            RenderPipeline::builder()
                .renderer(renderer)
                .shader_desc(read_wgsl("assets/shaders/blender.wgsl"))
                .bind_group_layouts(&[input_bind_group_layout])
                .immediate_size(BlenderImmediates::SIZE)
                .format(format)
                .blend(wgpu::BlendState::ALPHA_BLENDING)
                .build(),
        )
    }

    pub fn draw(
        &self,
        render_pass: &mut wgpu::RenderPass,
        input_bind_group: &wgpu::BindGroup,
        opacity: f32,
    ) {
        self.0.bind(render_pass, [input_bind_group]);
        BlenderImmediates::new(opacity).set(render_pass);
        render_pass.draw(0..3, 0..1);
    }
}

#[derive(Clone, Copy, Zeroable, Pod)]
#[repr(C)]
struct BlenderImmediates {
    opacity: f32,
}

impl BlenderImmediates {
    fn new(opacity: f32) -> Self {
        Self { opacity }
    }
}

impl Immediates for BlenderImmediates {}

pub struct Aces(RenderPipeline);

impl Aces {
    pub fn new(
        renderer: &Renderer,
        input_bind_group_layout: &wgpu::BindGroupLayout,
        format: wgpu::TextureFormat,
    ) -> Self {
        Self(
            RenderPipeline::builder()
                .renderer(renderer)
                .shader_desc(read_wgsl("assets/shaders/aces.wgsl"))
                .bind_group_layouts(&[input_bind_group_layout])
                .format(format)
                .build(),
        )
    }

    pub fn draw(&self, render_pass: &mut wgpu::RenderPass, input_bind_group: &wgpu::BindGroup) {
        self.0.bind(render_pass, [input_bind_group]);
        render_pass.draw(0..3, 0..1);
    }
}
