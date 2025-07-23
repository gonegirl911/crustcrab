use super::Renderer;
use bytemuck::Pod;
use std::slice;

pub struct Program(wgpu::RenderPipeline);

impl Program {
    #[expect(clippy::too_many_arguments)]
    pub fn new(
        Renderer { device, .. }: &Renderer,
        shader_desc: wgpu::ShaderModuleDescriptor,
        buffers: &[wgpu::VertexBufferLayout],
        bind_group_layouts: &[&wgpu::BindGroupLayout],
        push_constant_ranges: &[wgpu::PushConstantRange],
        cull_mode: Option<wgpu::Face>,
        depth_stencil: Option<wgpu::DepthStencilState>,
        format: wgpu::TextureFormat,
        blend: Option<wgpu::BlendState>,
    ) -> Self {
        let shader = device.create_shader_module(shader_desc);
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: None,
            bind_group_layouts,
            push_constant_ranges,
        });
        let render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: None,
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: None,
                compilation_options: Default::default(),
                buffers,
            },
            primitive: wgpu::PrimitiveState {
                cull_mode,
                ..Default::default()
            },
            depth_stencil,
            multisample: Default::default(),
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: None,
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            multiview: None,
            cache: None,
        });
        Self(render_pipeline)
    }

    pub fn bind<'a, I>(&self, render_pass: &mut wgpu::RenderPass, bind_groups: I)
    where
        I: IntoIterator<Item = &'a wgpu::BindGroup>,
    {
        render_pass.set_pipeline(&self.0);
        for (i, bind_group) in (0..).zip(bind_groups) {
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
            range: 0..size_of::<Self>() as u32,
        }
    }
}
