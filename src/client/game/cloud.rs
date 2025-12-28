use super::world::BlockVertex;
use crate::{
    client::{
        CLIENT_CONFIG,
        event_loop::{Event, EventHandler},
        renderer::{
            Renderer,
            buffer::{Instance, InstanceBuffer, MemoryState, Vertex, VertexBuffer},
            effect::{Blender, PostProcessor},
            program::{Immediates, Program},
            shader::read_wgsl,
            texture::{image::ImageTexture, screen::DepthBuffer},
        },
    },
    server::{
        ServerEvent,
        game::{
            clock::Stage,
            world::block::{Block, area::BlockArea},
        },
    },
    shared::color::{Float3, Rgb, Rgba},
};
use bytemuck::{Pod, Zeroable};
use nalgebra::{Point2, Vector2, Vector3, point, vector};
use serde::Deserialize;
use std::time::Duration;
use winit::event::WindowEvent;

pub struct CloudLayer {
    vertex_buffer: VertexBuffer<BlockVertex>,
    instance_buffer: InstanceBuffer<CloudInstance>,
    texture: ImageTexture,
    program: Program,
    blender: Blender,
    imm: CloudImmediates,
    opacity: f32,
}

impl CloudLayer {
    pub fn new(
        renderer: &Renderer,
        player_bind_group_layout: &wgpu::BindGroupLayout,
        spare_bind_group_layout: &wgpu::BindGroupLayout,
    ) -> Self {
        let vertex_buffer = VertexBuffer::new(renderer, MemoryState::Immutable(&Self::vertices()));
        let instance_buffer = InstanceBuffer::new(
            renderer,
            MemoryState::Immutable(&Self::instances().collect::<Vec<_>>()),
        );
        let texture = ImageTexture::builder()
            .renderer(renderer)
            .path(TEX_PATH)
            .is_srgb(false)
            .address_mode(wgpu::AddressMode::Repeat)
            .build();
        let program = Program::builder()
            .renderer(renderer)
            .shader_desc(read_wgsl("assets/shaders/cloud.wgsl"))
            .bind_group_layouts(&[player_bind_group_layout, texture.bind_group_layout()])
            .immediate_size(CloudImmediates::SIZE)
            .buffers(&[BlockVertex::desc(), CloudInstance::desc()])
            .depth_stencil(wgpu::DepthStencilState {
                format: DepthBuffer::FORMAT,
                depth_write_enabled: true,
                depth_compare: wgpu::CompareFunction::LessEqual,
                stencil: Default::default(),
                bias: Default::default(),
            })
            .format(PostProcessor::FORMAT)
            .build();
        let blender = Blender::new(renderer, spare_bind_group_layout, PostProcessor::FORMAT);
        Self {
            vertex_buffer,
            instance_buffer,
            texture,
            program,
            blender,
            imm: Default::default(),
            opacity: Self::opacity(Default::default()),
        }
    }

    #[rustfmt::skip]
    pub fn draw(
        &self,
        view: &wgpu::TextureView,
        encoder: &mut wgpu::CommandEncoder,
        spare_view: &wgpu::TextureView,
        player_bind_group: &wgpu::BindGroup,
        depth_view: &wgpu::TextureView,
        spare_bind_group: &wgpu::BindGroup,
    ) {
        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: spare_view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(Default::default()),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: depth_view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                ..Default::default()
            });
            self.program.bind(
                &mut render_pass,
                [player_bind_group, self.texture.bind_group()],
            );
            self.imm.set(&mut render_pass);
            self.vertex_buffer.draw_instanced(&mut render_pass, &self.instance_buffer);
        }
        self.blender.draw(view, encoder, spare_bind_group, self.opacity, true);
    }

    fn vertices() -> Vec<BlockVertex> {
        Block::SAND
            .data()
            .mesh(
                Default::default(),
                &BlockArea::default().with_kernel(Block::SAND),
                &Default::default(),
            )
            .collect()
    }

    fn instances() -> impl Iterator<Item = CloudInstance> {
        let radius = (CLIENT_CONFIG.player.render_distance() / CLIENT_CONFIG.cloud.size.x) as i32;
        (-radius..=radius).flat_map(move |dx| {
            (-radius..=radius)
                .filter(move |dz| dx.pow(2) + dz.pow(2) <= radius.pow(2))
                .map(move |dz| CloudInstance::new(vector![dx, dz]))
        })
    }

    fn opacity(stage: Stage) -> f32 {
        stage.lerp(
            CLIENT_CONFIG.cloud.day.color.a,
            CLIENT_CONFIG.cloud.night.color.a,
        )
    }
}

impl EventHandler for CloudLayer {
    type Context<'a> = Duration;

    fn handle(&mut self, event: &Event, dt: Self::Context<'_>) {
        match event {
            Event::ServerEvent(ServerEvent::TimeUpdated(time)) => {
                let stage = time.stage();
                self.imm.update_color(stage);
                self.opacity = Self::opacity(stage);
            }
            Event::WindowEvent(WindowEvent::RedrawRequested) => {
                self.imm.update_offset(dt);
            }
            _ => {}
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy, Zeroable, Pod)]
struct CloudInstance {
    offset: Vector2<f32>,
}

impl CloudInstance {
    fn new(offset: Vector2<i32>) -> Self {
        Self {
            offset: (offset.cast() * CLIENT_CONFIG.cloud.size.x as i64).cast(),
        }
    }
}

impl Instance for CloudInstance {
    const ATTRIBS: &[wgpu::VertexAttribute] = &wgpu::vertex_attr_array![1 => Float32x2];
}

#[repr(C)]
#[derive(Clone, Copy, Zeroable, Pod)]
struct CloudImmediates {
    dims: Point2<f32>,
    size: Point2<f32>,
    scale_factor: Float3,
    color: Float3,
    offset: Vector2<f32>,
    padding: [f32; 2],
}

impl CloudImmediates {
    fn update_color(&mut self, stage: Stage) {
        self.color = Self::color(stage).into();
    }

    fn update_offset(&mut self, dt: Duration) {
        self.offset.x -= CLIENT_CONFIG.cloud.speed * dt.as_secs_f32();
        self.offset.x %= self.size.x * self.dims.x;
    }

    fn dims() -> Point2<u32> {
        let (width, height) = image::image_dimensions(TEX_PATH)
            .unwrap_or_else(|e| panic!("failed to read dimensions of {TEX_PATH}: {e}"));
        point![width, height]
    }

    fn scale_factor() -> Vector3<f32> {
        let size = CLIENT_CONFIG.cloud.size.coords.xyx();
        let padding = CLIENT_CONFIG.cloud.padding;
        size.map(|c| 1.0 + padding * 2.0 / c as f32)
    }

    fn color(stage: Stage) -> Rgb<f32> {
        stage.lerp(
            CLIENT_CONFIG.cloud.day.color.rgb,
            CLIENT_CONFIG.cloud.night.color.rgb,
        )
    }
}

impl Default for CloudImmediates {
    fn default() -> Self {
        Self {
            dims: Self::dims().cast(),
            size: CLIENT_CONFIG.cloud.size.cast(),
            scale_factor: Self::scale_factor().into(),
            color: Self::color(Default::default()).into(),
            offset: Default::default(),
            padding: Default::default(),
        }
    }
}

impl Immediates for CloudImmediates {}

#[derive(Deserialize)]
pub struct CloudConfig {
    size: Point2<u64>,
    pub padding: f32,
    speed: f32,
    day: StageConfig,
    night: StageConfig,
}

#[derive(Deserialize)]
struct StageConfig {
    color: Rgba<f32>,
}

const TEX_PATH: &str = "assets/textures/clouds.png";
