use super::world::BlockVertex;
use crate::{
    client::{
        event_loop::{Event, EventHandler},
        renderer::{
            buffer::{Instance, InstanceBuffer, MemoryState, Vertex, VertexBuffer},
            effect::{Blender, PostProcessor},
            program::{Program, PushConstants},
            texture::{image::ImageTexture, screen::DepthBuffer},
            Renderer,
        },
        CLIENT_CONFIG,
    },
    server::{
        game::{
            clock::Stage,
            world::{block::Block, chunk::Chunk},
        },
        ServerEvent,
    },
};
use bytemuck::{Pod, Zeroable};
use nalgebra::{vector, Vector2};
use serde::Deserialize;
use std::time::Duration;

pub struct CloudLayer {
    vertex_buffer: VertexBuffer<CloudVertex>,
    instance_buffer: InstanceBuffer<CloudInstance>,
    texture: ImageTexture,
    program: Program,
    blender: Blender,
    pc: CloudPushConstants,
    opacity: f32,
}

impl CloudLayer {
    pub fn new(
        renderer: &Renderer,
        player_bind_group_layout: &wgpu::BindGroupLayout,
        sky_bind_group_layout: &wgpu::BindGroupLayout,
        spare_bind_group_layout: &wgpu::BindGroupLayout,
    ) -> Self {
        let vertex_buffer = VertexBuffer::new(
            renderer,
            MemoryState::Immutable(&Self::vertices().collect::<Vec<_>>()),
        );
        let instance_buffer = InstanceBuffer::new(
            renderer,
            MemoryState::Immutable(&Self::instances().collect::<Vec<_>>()),
        );
        let texture = ImageTexture::new(
            renderer,
            "assets/textures/clouds.png",
            false,
            true,
            1,
            wgpu::AddressMode::Repeat,
        );
        let program = Program::new(
            renderer,
            wgpu::include_wgsl!("../../../assets/shaders/cloud.wgsl"),
            &[CloudVertex::desc(), CloudInstance::desc()],
            &[
                player_bind_group_layout,
                sky_bind_group_layout,
                texture.bind_group_layout(),
            ],
            &[CloudPushConstants::range()],
            PostProcessor::FORMAT,
            None,
            None,
            Some(wgpu::DepthStencilState {
                format: DepthBuffer::FORMAT,
                depth_write_enabled: true,
                depth_compare: wgpu::CompareFunction::Less,
                stencil: Default::default(),
                bias: Default::default(),
            }),
        );
        let blender = Blender::new(renderer, spare_bind_group_layout, PostProcessor::FORMAT);
        Self {
            vertex_buffer,
            instance_buffer,
            texture,
            program,
            blender,
            pc: Default::default(),
            opacity: Self::opacity(Default::default()),
        }
    }

    #[rustfmt::skip]
    #[allow(clippy::too_many_arguments)]
    pub fn draw(
        &self,
        view: &wgpu::TextureView,
        encoder: &mut wgpu::CommandEncoder,
        spare_view: &wgpu::TextureView,
        player_bind_group: &wgpu::BindGroup,
        sky_bind_group: &wgpu::BindGroup,
        depth_view: &wgpu::TextureView,
        spare_bind_group: &wgpu::BindGroup,
    ) {
        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: None,
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: spare_view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(Default::default()),
                        store: true,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: depth_view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: true,
                    }),
                    stencil_ops: None,
                }),
            });
            self.program.bind(
                &mut render_pass,
                [player_bind_group, sky_bind_group, self.texture.bind_group()],
            );
            self.pc.set(&mut render_pass);
            self.vertex_buffer.draw_instanced(&mut render_pass, &self.instance_buffer);
        }
        self.blender.draw(view, encoder, spare_bind_group, self.opacity);
    }

    fn vertices() -> impl Iterator<Item = CloudVertex> {
        Block::Sand
            .data()
            .vertices(Default::default(), Block::Sand.into(), Default::default())
            .map(Into::into)
    }

    fn instances() -> impl Iterator<Item = CloudInstance> {
        let radius = (CLIENT_CONFIG.player.render_distance * Chunk::DIM as u32 / 12) as i32;
        (-radius..=radius).flat_map(move |dx| {
            (-radius..=radius)
                .filter(move |dz| dx.pow(2) + dz.pow(2) <= radius.pow(2))
                .map(move |dz| CloudInstance::new(vector![dx, dz]))
        })
    }

    fn opacity(stage: Stage) -> f32 {
        stage.lerp(
            CLIENT_CONFIG.cloud.day_opacity,
            CLIENT_CONFIG.cloud.night_opacity,
        )
    }
}

impl EventHandler for CloudLayer {
    type Context<'a> = Duration;

    fn handle(&mut self, event: &Event, dt: Self::Context<'_>) {
        match event {
            Event::UserEvent(ServerEvent::TimeUpdated(time)) => {
                self.opacity = Self::opacity(time.stage());
            }
            Event::MainEventsCleared => {
                self.pc.move_forward(dt);
            },
            _ => {}
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy, Zeroable, Pod)]
struct CloudVertex {
    data: u32,
}

impl From<BlockVertex> for CloudVertex {
    fn from(vertex: BlockVertex) -> Self {
        Self { data: vertex.data }
    }
}

impl Vertex for CloudVertex {
    const ATTRIBS: &'static [wgpu::VertexAttribute] = &wgpu::vertex_attr_array![0 => Uint32];
}

#[repr(C)]
#[derive(Clone, Copy, Zeroable, Pod)]
struct CloudInstance {
    offset: Vector2<f32>,
}

impl CloudInstance {
    fn new(offset: Vector2<i32>) -> Self {
        Self {
            offset: (offset * 12).cast(),
        }
    }
}

impl Instance for CloudInstance {
    const ATTRIBS: &'static [wgpu::VertexAttribute] = &wgpu::vertex_attr_array![1 => Float32x2];
}

#[repr(C)]
#[derive(Clone, Copy, Default, Zeroable, Pod)]
struct CloudPushConstants {
    offset: Vector2<f32>,
}

impl CloudPushConstants {
    fn move_forward(&mut self, dt: Duration) {
        self.offset -= Vector2::x() * CLIENT_CONFIG.cloud.speed * dt.as_secs_f32();
        self.offset = self.offset.map(|c| c % (12.0 * 256.0));
    }
}

impl PushConstants for CloudPushConstants {
    const STAGES: wgpu::ShaderStages = wgpu::ShaderStages::VERTEX;
}

#[derive(Deserialize)]
pub struct CloudConfig {
    day_opacity: f32,
    night_opacity: f32,
    speed: f32,
}
