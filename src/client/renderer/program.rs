use super::Renderer;
use bytemuck::Pod;
use std::{mem, slice};

pub struct Program(wgpu::RenderPipeline);

impl Program {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        Renderer { device, .. }: &Renderer,
        desc: wgpu::ShaderModuleDescriptor,
        buffers: &[wgpu::VertexBufferLayout],
        bind_group_layouts: &[&wgpu::BindGroupLayout],
        push_constant_ranges: &[wgpu::PushConstantRange],
        format: wgpu::TextureFormat,
        blend: Option<wgpu::BlendState>,
        cull_mode: Option<wgpu::Face>,
        depth_stencil: Option<wgpu::DepthStencilState>,
    ) -> Self {
        let shader = device.create_shader_module(desc);
        let render_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: None,
                bind_group_layouts,
                push_constant_ranges,
            });
        let render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: None,
            layout: Some(&render_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: "vs_main",
                buffers,
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: "fs_main",
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState {
                cull_mode,
                ..Default::default()
            },
            depth_stencil,
            multisample: Default::default(),
            multiview: None,
        });
        Self(render_pipeline)
    }

    pub fn bind<'a, I>(&'a self, render_pass: &mut wgpu::RenderPass<'a>, bind_groups: I)
    where
        I: IntoIterator<Item = &'a wgpu::BindGroup>,
    {
        render_pass.set_pipeline(&self.0);
        for (bind_group, i) in bind_groups.into_iter().zip(0..) {
            render_pass.set_bind_group(i, bind_group, &[]);
        }
    }
}

pub trait PushConstants: Pod {
    const STAGES: wgpu::ShaderStages;

    fn set(&self, render_pass: &mut wgpu::RenderPass) {
        render_pass.set_push_constants(
            Self::STAGES,
            0,
            bytemuck::cast_slice(slice::from_ref(self)),
        );
    }

    fn range() -> wgpu::PushConstantRange {
        wgpu::PushConstantRange {
            stages: Self::STAGES,
            range: 0..mem::size_of::<Self>() as u32,
        }
    }
}
