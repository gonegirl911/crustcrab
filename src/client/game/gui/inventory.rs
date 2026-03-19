use super::Gui;
use crate::{
    client::{
        CLIENT_CONFIG,
        event_loop::{Event, EventHandler},
        game::world::BlockVertex,
        renderer::{
            Renderer, Surface,
            buffer::{MemoryState, VertexBuffer},
            effect::PostProcessor,
            program::Program,
            texture::screen::DepthBuffer,
            uniform::Uniform,
            utils::{Vertex, read_wgsl},
        },
    },
    server::{
        ServerEvent,
        game::world::block::{Block, area::BlockArea},
    },
};
use bytemuck::{Pod, Zeroable};
use nalgebra::{Matrix4, Vector3, vector};
use serde::Deserialize;
use std::{
    f32::consts::{FRAC_PI_4, FRAC_PI_6},
    mem,
    sync::Arc,
};
use winit::{
    event::{ElementState, KeyEvent, WindowEvent},
    keyboard::{KeyCode, PhysicalKey},
};

pub struct Inventory {
    vertex_buffer: Option<VertexBuffer<BlockVertex>>,
    uniform: Uniform<InventoryUniformData>,
    program: Program,
    contents: Arc<[Block]>,
    index: usize,
    is_flat: bool,
    is_updated: bool,
}

impl Inventory {
    pub fn new(renderer: &Renderer, textures_bind_group_layout: &wgpu::BindGroupLayout) -> Self {
        let uniform = Uniform::new(renderer, MemoryState::UNINIT, wgpu::ShaderStages::VERTEX);
        let program = Program::builder()
            .renderer(renderer)
            .shader_desc(read_wgsl("assets/shaders/inventory.wgsl"))
            .bind_group_layouts(&[uniform.bind_group_layout(), textures_bind_group_layout])
            .buffers(&[BlockVertex::desc()])
            .cull_mode(wgpu::Face::Back)
            .depth_stencil(wgpu::DepthStencilState {
                format: DepthBuffer::FORMAT,
                depth_write_enabled: Some(true),
                depth_compare: Some(wgpu::CompareFunction::Less),
                stencil: Default::default(),
                bias: Default::default(),
            })
            .format(PostProcessor::FORMAT)
            .blend(wgpu::BlendState::ALPHA_BLENDING)
            .build();
        Self {
            vertex_buffer: None,
            uniform,
            program,
            contents: Default::default(),
            index: 0,
            is_flat: false,
            is_updated: true,
        }
    }

    pub fn selected_block(&self) -> Option<Block> {
        self.contents.get(self.index).copied()
    }

    pub fn draw(&self, render_pass: &mut wgpu::RenderPass, textures_bind_group: &wgpu::BindGroup) {
        if let Some(buffer) = &self.vertex_buffer {
            self.program.bind(
                render_pass,
                [self.uniform.bind_group(), textures_bind_group],
            );
            buffer.draw(render_pass);
        }
    }

    fn index(keycode: KeyCode) -> Option<usize> {
        match keycode {
            KeyCode::Digit1 => Some(0),
            KeyCode::Digit2 => Some(1),
            KeyCode::Digit3 => Some(2),
            KeyCode::Digit4 => Some(3),
            KeyCode::Digit5 => Some(4),
            KeyCode::Digit6 => Some(5),
            KeyCode::Digit7 => Some(6),
            KeyCode::Digit8 => Some(7),
            KeyCode::Digit9 => Some(8),
            _ => None,
        }
    }
}

impl EventHandler for Inventory {
    type Context<'a> = (&'a Renderer, &'a Surface);

    fn handle(&mut self, event: &Event, (renderer, surface): Self::Context<'_>) {
        match event {
            Event::ServerEvent(ServerEvent::PlayerInitialized { inventory, .. }) => {
                self.contents = inventory.clone();
            }
            Event::WindowEvent(event) => match *event {
                WindowEvent::KeyboardInput {
                    event:
                        KeyEvent {
                            physical_key: PhysicalKey::Code(keycode),
                            state: ElementState::Pressed,
                            ..
                        },
                    ..
                } => {
                    if let Some(idx) = Self::index(keycode) {
                        self.is_updated = mem::replace(&mut self.index, idx) != idx;
                    }
                }
                WindowEvent::RedrawRequested => {
                    let mut is_transform_outdated = surface.is_resized;

                    if mem::take(&mut self.is_updated) {
                        let mut is_flat = false;

                        self.vertex_buffer = self.selected_block().and_then(|block| {
                            let data = block.data();
                            let vertices = if let Some(vertices) = data.flat_icon() {
                                is_flat = true;
                                vertices.collect::<Vec<_>>()
                            } else {
                                data.mesh(
                                    Default::default(),
                                    &BlockArea::default().with_kernel(block),
                                    &Default::default(),
                                )
                                .collect()
                            };
                            VertexBuffer::try_new(renderer, MemoryState::Immutable(&vertices))
                        });

                        if mem::replace(&mut self.is_flat, is_flat) != is_flat {
                            is_transform_outdated = true;
                        }
                    }

                    if is_transform_outdated {
                        self.uniform
                            .set(renderer, &InventoryUniformData::new(surface, self.is_flat));
                    }
                }
                _ => {}
            },
            _ => {}
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy, Zeroable, Pod)]
struct InventoryUniformData {
    transform: Matrix4<f32>,
}

impl InventoryUniformData {
    fn new(surface: &Surface, is_flat: bool) -> Self {
        let scaling = Gui::scaling(
            surface.width(),
            surface.height(),
            CLIENT_CONFIG.gui.inventory.size,
        );
        let transform = Gui::transform(scaling, scaling.map(|c| 1.0 - c * 1.44));
        Self {
            transform: if is_flat {
                transform
            } else {
                let diagonal = 3.0f32.sqrt();
                let rot_x = -FRAC_PI_6;
                let theta = (1.0 / diagonal).acos() + rot_x;
                transform
                    * Matrix4::new_rotation(Vector3::x() * rot_x)
                        .append_scaling(1.0 / diagonal / theta.cos())
                        .append_translation(&vector![0.5, 0.5, 0.545])
                    * Matrix4::new_rotation(Vector3::y() * FRAC_PI_4)
                        .prepend_translation(&Vector3::repeat(-0.5))
            },
        }
    }
}

#[derive(Deserialize)]
pub struct InventoryConfig {
    size: f32,
}
