use super::Gui;
use crate::{
    client::{
        CLIENT_CONFIG,
        event_loop::{Event, EventHandler},
        game::world::BlockVertex,
        renderer::{
            Renderer,
            buffer::{MemoryState, VertexBuffer},
            effect::PostProcessor,
            program::Program,
            texture::screen::DepthBuffer,
            uniform::Uniform,
            utils::{Vertex, read_wgsl},
        },
    },
    server::game::world::block::{Block, area::BlockArea, data::STR_TO_BLOCK},
};
use bytemuck::{Pod, Zeroable};
use nalgebra::{Matrix4, Vector3, vector};
use serde::{Deserialize, Deserializer};
use std::{
    f32::consts::{FRAC_PI_4, FRAC_PI_6},
    mem,
    ops::Deref,
};
use winit::{
    event::{ElementState, KeyEvent, WindowEvent},
    keyboard::{KeyCode, PhysicalKey},
};

pub struct Inventory {
    vertex_buffer: Option<VertexBuffer<BlockVertex>>,
    uniform: Uniform<InventoryUniformData>,
    program: Program,
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
                depth_write_enabled: true,
                depth_compare: wgpu::CompareFunction::Less,
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
            index: 0,
            is_flat: false,
            is_updated: true,
        }
    }

    #[rustfmt::skip]
    pub fn selected_block(&self) -> Option<Block> {
        CLIENT_CONFIG.gui.inventory.contents.get(self.index).copied()
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
    type Context<'a> = &'a Renderer;

    fn handle(&mut self, event: &Event, renderer: Self::Context<'_>) {
        if let Event::WindowEvent(event) = event {
            match *event {
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
                    let mut is_transform_outdated = renderer.is_surface_resized;

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
                            .set(renderer, &InventoryUniformData::new(renderer, self.is_flat));
                    }
                }
                _ => {}
            }
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy, Zeroable, Pod)]
struct InventoryUniformData {
    transform: Matrix4<f32>,
}

impl InventoryUniformData {
    fn new(renderer: &Renderer, is_flat: bool) -> Self {
        let scaling = Gui::scaling(renderer, CLIENT_CONFIG.gui.inventory.size);
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
    #[serde(deserialize_with = "InventoryConfig::deserialize_contents")]
    contents: Vec<Block>,
    size: f32,
}

impl InventoryConfig {
    fn deserialize_contents<'de, D>(deserializer: D) -> Result<Vec<Block>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let contents = Vec::<String>::deserialize(deserializer)?;
        assert!(contents.len() <= 9, "inventory has only 9 available slots");
        contents
            .into_iter()
            .map(|str| {
                STR_TO_BLOCK.get(&*str).copied().ok_or_else(|| {
                    serde::de::Error::invalid_value(
                        serde::de::Unexpected::Str(&str),
                        &&*format!(
                            "one of \"{}\"",
                            STR_TO_BLOCK
                                .keys()
                                .map(Deref::deref)
                                .collect::<Vec<_>>()
                                .join("\", \"")
                        ),
                    )
                })
            })
            .collect()
    }
}
