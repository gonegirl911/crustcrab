pub mod atlas;
pub mod block;
pub mod chunk;
pub mod selected;

use self::{
    atlas::TextureAtlas,
    block::BlockVertex,
    chunk::{BlockPushConstants, ChunkMeshPool},
    selected::SelectedBlock,
};
use super::{depth_buffer::DepthBuffer, player::frustum::Frustum};
use crate::client::{
    event_loop::{Event, EventHandler},
    renderer::Renderer,
};
use std::mem;

pub struct World {
    meshes: ChunkMeshPool,
    atlas: TextureAtlas,
    selected_block: SelectedBlock,
    render_pipeline: wgpu::RenderPipeline,
}

impl World {
    pub fn new(
        renderer @ Renderer { device, config, .. }: &Renderer,
        player_bind_group_layout: &wgpu::BindGroupLayout,
        clock_bind_group_layout: &wgpu::BindGroupLayout,
        sky_bind_group_layout: &wgpu::BindGroupLayout,
    ) -> Self {
        let meshes = ChunkMeshPool::new();
        let atlas = TextureAtlas::new(renderer);
        let selected_block = SelectedBlock::new(renderer, player_bind_group_layout);

        let shader = device.create_shader_module(wgpu::include_wgsl!(
            "../../../../../assets/shaders/block.wgsl"
        ));
        let render_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: None,
                bind_group_layouts: &[
                    player_bind_group_layout,
                    clock_bind_group_layout,
                    atlas.bind_group_layout(),
                    sky_bind_group_layout,
                ],
                push_constant_ranges: &[wgpu::PushConstantRange {
                    stages: wgpu::ShaderStages::VERTEX,
                    range: 0..mem::size_of::<BlockPushConstants>() as u32,
                }],
            });
        let render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: None,
            layout: Some(&render_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: "vs_main",
                buffers: &[BlockVertex::desc()],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: "fs_main",
                targets: &[Some(wgpu::ColorTargetState {
                    format: config.format,
                    blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState {
                cull_mode: Some(wgpu::Face::Back),
                ..Default::default()
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: DepthBuffer::DEPTH_FORMAT,
                depth_write_enabled: true,
                depth_compare: wgpu::CompareFunction::Less,
                stencil: Default::default(),
                bias: Default::default(),
            }),
            multisample: Default::default(),
            multiview: None,
        });

        Self {
            meshes,
            atlas,
            selected_block,
            render_pipeline,
        }
    }

    pub fn draw<'a>(
        &'a self,
        render_pass: &mut wgpu::RenderPass<'a>,
        player_bind_group: &'a wgpu::BindGroup,
        clock_bind_group: &'a wgpu::BindGroup,
        sky_bind_group: &'a wgpu::BindGroup,
        frustum: &Frustum,
    ) {
        render_pass.set_pipeline(&self.render_pipeline);
        render_pass.set_bind_group(0, player_bind_group, &[]);
        render_pass.set_bind_group(1, clock_bind_group, &[]);
        render_pass.set_bind_group(2, self.atlas.bind_group(), &[]);
        render_pass.set_bind_group(3, sky_bind_group, &[]);
        self.meshes.draw(render_pass, frustum);
        self.selected_block.draw(render_pass, player_bind_group);
    }
}

impl EventHandler for World {
    type Context<'a> = &'a Renderer;

    fn handle(&mut self, event: &Event, renderer: Self::Context<'_>) {
        self.meshes.handle(event, renderer);
        self.selected_block.handle(event, ());
    }
}
