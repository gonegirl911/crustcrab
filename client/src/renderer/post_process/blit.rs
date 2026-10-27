use crate::renderer::{
    Renderer,
    render_pipeline::{RenderPipeline, read_wgsl},
};
use bon::bon;
use enum_map::Enum;
use std::ops::Deref;

pub(crate) struct Blit(RenderPipeline);

#[bon]
impl Blit {
    #[builder]
    pub(crate) fn new(
        renderer: &Renderer,
        input_bind_group_layout: &wgpu::BindGroupLayout,
        #[builder(default)] operation: Operation,
        format: wgpu::TextureFormat,
    ) -> Self {
        Self(
            RenderPipeline::builder()
                .renderer(renderer)
                .shader_desc(read_wgsl("client/assets/shaders/blit.wgsl"))
                .bind_group_layouts(&[Some(input_bind_group_layout)])
                .fragment_entry_point(operation.fragment_entry_point())
                .format(format)
                .build(),
        )
    }

    pub(crate) fn draw(
        &self,
        render_pass: &mut wgpu::RenderPass,
        input_bind_group: &wgpu::BindGroup,
    ) {
        self.0.bind(render_pass, [input_bind_group]);
        render_pass.draw(0..3, 0..1);
    }
}

impl Deref for Blit {
    type Target = RenderPipeline;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[derive(Clone, Copy, Enum, Default)]
pub(crate) enum Operation {
    #[default]
    Copy,
    EncodeSrgb,
    DecodeSrgb,
}

impl Operation {
    fn fragment_entry_point(&self) -> &'static str {
        match self {
            Self::Copy => "fs_copy",
            Self::EncodeSrgb => "fs_encode_srgb",
            Self::DecodeSrgb => "fs_decode_srgb",
        }
    }
}
