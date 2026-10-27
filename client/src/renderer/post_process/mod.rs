pub(crate) mod blit;
mod dither;
mod tonemap;

use super::{Renderer, Surface, render_pass::color_pass, texture::screen::ScreenTextureArray};
use crate::event::{self, Event};
use blit::Blit;
use dither::Dither;
use enum_map::EnumMap;
use tonemap::Tonemap;

pub(crate) struct PostProcessor {
    textures: ScreenTextureArray<2>,
    blits: EnumMap<blit::Operation, Blit>,
    tonemap: Tonemap,
    dither: Dither,
}

impl PostProcessor {
    pub(crate) const FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba16Float;
    const CURRENT_INDEX: usize = 0;
    const SPARE_INDEX: usize = 1;

    pub(crate) fn new(renderer: &Renderer, surface: &Surface) -> Self {
        let textures = ScreenTextureArray::new(renderer, surface, Self::FORMAT);
        let blits = EnumMap::from_fn(|operation| {
            Blit::builder()
                .renderer(renderer)
                .input_bind_group_layout(&textures.bind_group_layout)
                .operation(operation)
                .format(Self::FORMAT)
                .build()
        });
        let tonemap = Tonemap::new(renderer, &textures.bind_group_layout, Self::FORMAT);
        let dither = Dither::new(renderer, &textures.bind_group_layout, Self::FORMAT);
        Self {
            textures,
            blits,
            tonemap,
            dither,
        }
    }

    pub(crate) fn current_view(&self) -> &wgpu::TextureView {
        &self.textures.views[Self::CURRENT_INDEX]
    }

    pub(crate) fn spare_view(&self) -> &wgpu::TextureView {
        &self.textures.views[Self::SPARE_INDEX]
    }

    pub(crate) fn bind_group_layout(&self) -> &wgpu::BindGroupLayout {
        &self.textures.bind_group_layout
    }

    pub(crate) fn current_bind_group(&self) -> &wgpu::BindGroup {
        &self.textures.bind_groups[Self::CURRENT_INDEX]
    }

    pub(crate) fn spare_bind_group(&self) -> &wgpu::BindGroup {
        &self.textures.bind_groups[Self::SPARE_INDEX]
    }

    pub(crate) fn encode_srgb(&mut self, encoder: &mut wgpu::CommandEncoder) {
        Self::step(
            &mut self.textures,
            encoder,
            |render_pass, current_bind_group| {
                self.blits[blit::Operation::EncodeSrgb].draw(render_pass, current_bind_group);
            },
        );
    }

    pub(crate) fn decode_srgb(&mut self, encoder: &mut wgpu::CommandEncoder) {
        Self::step(
            &mut self.textures,
            encoder,
            |render_pass, current_bind_group| {
                self.blits[blit::Operation::DecodeSrgb].draw(render_pass, current_bind_group);
            },
        );
    }

    pub(crate) fn tonemap(&mut self, encoder: &mut wgpu::CommandEncoder) {
        Self::step(
            &mut self.textures,
            encoder,
            |render_pass, current_bind_group| {
                self.tonemap.draw(render_pass, current_bind_group);
            },
        );
    }

    pub(crate) fn dither(&mut self, encoder: &mut wgpu::CommandEncoder) {
        Self::step(
            &mut self.textures,
            encoder,
            |render_pass, current_bind_group| {
                self.dither.draw(render_pass, current_bind_group);
            },
        );
    }

    fn step<D>(textures: &mut ScreenTextureArray<2>, encoder: &mut wgpu::CommandEncoder, draw: D)
    where
        D: FnOnce(&mut wgpu::RenderPass<'_>, &wgpu::BindGroup),
    {
        draw(
            &mut color_pass(
                &textures.views[Self::SPARE_INDEX],
                encoder,
                wgpu::LoadOp::Clear(Default::default()),
            ),
            &textures.bind_groups[Self::CURRENT_INDEX],
        );
        textures.swap(0, 1);
    }
}

impl event::Handler for PostProcessor {
    type Context<'a> = (&'a Renderer, &'a Surface);

    fn handle(&mut self, event: &Event, (renderer, surface): Self::Context<'_>) {
        self.textures.handle(event, (renderer, surface));
    }
}
