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
    f32::consts::{FRAC_PI_4, FRAC_PI_6, SQRT_2},
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
    is_icon_flat: bool,
    is_updated: bool,
}

impl Inventory {
    pub fn new(
        renderer: &Renderer,
        lighting_bind_group_layout: &wgpu::BindGroupLayout,
        textures_bind_group_layout: &wgpu::BindGroupLayout,
    ) -> Self {
        let uniform = Uniform::new(renderer, MemoryState::UNINIT, wgpu::ShaderStages::VERTEX);
        let program = Program::builder()
            .renderer(renderer)
            .shader_desc(read_wgsl("assets/shaders/inventory.wgsl"))
            .bind_group_layouts(&[
                uniform.bind_group_layout(),
                lighting_bind_group_layout,
                textures_bind_group_layout,
            ])
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
            is_icon_flat: false,
            is_updated: true,
        }
    }

    pub fn selected_block(&self) -> Option<Block> {
        self.contents.get(self.index).copied()
    }

    pub fn draw(
        &self,
        render_pass: &mut wgpu::RenderPass,
        lighting_bind_group: &wgpu::BindGroup,
        textures_bind_group: &wgpu::BindGroup,
    ) {
        if let Some(buffer) = &self.vertex_buffer {
            self.program.bind(
                render_pass,
                [
                    self.uniform.bind_group(),
                    lighting_bind_group,
                    textures_bind_group,
                ],
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
                self.is_updated = true;
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
                        let mut is_icon_flat = false;

                        self.vertex_buffer = self.selected_block().and_then(|block| {
                            let data = block.data();
                            let vertices = if let Some(vertices) = data.flat_icon() {
                                is_icon_flat = true;
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

                        if mem::replace(&mut self.is_icon_flat, is_icon_flat) != is_icon_flat {
                            is_transform_outdated = true;
                        }
                    }

                    if is_transform_outdated {
                        self.uniform.set(
                            renderer,
                            &InventoryUniformData::new(surface, self.is_icon_flat),
                        );
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
    fn new(surface: &Surface, is_icon_flat: bool) -> Self {
        let icon_projection = Self::icon_projection(is_icon_flat);
        let scaling = Gui::scaling(surface, CLIENT_CONFIG.gui.inventory.size);
        let gui_transform = Gui::transform(scaling, scaling.map(|c| 1.0 - c * 1.44));
        Self {
            transform: gui_transform * icon_projection,
        }
    }

    fn icon_projection(is_icon_flat: bool) -> Matrix4<f32> {
        if is_icon_flat {
            Matrix4::identity()
        } else {
            let sqrt3 = 3.0f32.sqrt();
            Matrix4::new_translation(&vector![0.5, 0.5, SQRT_2 - sqrt3 / 2.0])
                * Matrix4::new_scaling(2.0 / (SQRT_2 + sqrt3))
                * Matrix4::new_rotation(Vector3::x() * -FRAC_PI_6)
                * Matrix4::new_rotation(Vector3::y() * FRAC_PI_4)
                * Matrix4::new_translation(&Vector3::repeat(-0.5))
        }
    }
}

#[derive(Deserialize)]
pub struct InventoryConfig {
    size: f32,
}
