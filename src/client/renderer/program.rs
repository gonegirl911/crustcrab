use super::Renderer;
use bon::bon;
use bytemuck::Pod;
use std::slice;

pub struct Program(wgpu::RenderPipeline);

#[bon]
impl Program {
    #[builder]
    pub fn new<'a>(
        #[expect(unused)] renderer @ Renderer { device, .. }: &'a Renderer,
        shader_desc: wgpu::ShaderModuleDescriptor<'a>,
        #[builder(default)] bind_group_layouts: &'a [&'a wgpu::BindGroupLayout],
        #[builder(default)] immediate_size: u32,
        #[builder(default)] buffers: &'a [wgpu::VertexBufferLayout<'a>],
        cull_mode: Option<wgpu::Face>,
        depth_stencil: Option<wgpu::DepthStencilState>,
        format: wgpu::TextureFormat,
        blend: Option<wgpu::BlendState>,
    ) -> Self {
        let shader = device.create_shader_module(shader_desc);
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: None,
            bind_group_layouts,
            immediate_size,
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
            multiview_mask: None,
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

pub trait Immediates: Pod {
    const SIZE: u32 = {
        let size = size_of::<Self>();
        assert!(usize::BITS <= u32::BITS || size <= u32::MAX as usize);
        size as u32
    };

    fn set(&self, render_pass: &mut wgpu::RenderPass) {
        render_pass.set_immediates(0, bytemuck::cast_slice(slice::from_ref(self)));
    }
}
