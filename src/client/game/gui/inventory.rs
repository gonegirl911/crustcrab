use super::Gui;
use crate::{
    client::{
        event_loop::{Event, EventHandler},
        game::world::BlockVertex,
        renderer::{
            buffer::{MemoryState, Vertex, VertexBuffer},
            effect::PostProcessor,
            program::Program,
            texture::screen::DepthBuffer,
            uniform::Uniform,
            Renderer,
        },
        CLIENT_CONFIG,
    },
    server::game::world::block::Block,
};
use arrayvec::ArrayVec;
use bytemuck::{Pod, Zeroable};
use nalgebra::{vector, Matrix4, Vector3};
use serde::Deserialize;
use std::{
    f32::consts::{FRAC_PI_4, FRAC_PI_6},
    mem,
};
use winit::{
    dpi::PhysicalSize,
    event::{ElementState, KeyboardInput, VirtualKeyCode, WindowEvent},
};

pub struct Inventory {
    buffer: Option<VertexBuffer<BlockVertex>>,
    uniform: Uniform<InventoryUniformData>,
    program: Program,
    index: usize,
    is_flat: bool,
    is_updated: bool,
    is_resized: bool,
}

impl Inventory {
    pub fn new(renderer: &Renderer, textures_bind_group_layout: &wgpu::BindGroupLayout) -> Self {
        let uniform = Uniform::new(renderer, MemoryState::UNINIT, wgpu::ShaderStages::VERTEX);
        let program = Program::new(
            renderer,
            wgpu::include_wgsl!("../../../../assets/shaders/inventory.wgsl"),
            &[BlockVertex::desc()],
            &[uniform.bind_group_layout(), textures_bind_group_layout],
            &[],
            PostProcessor::FORMAT,
            Some(wgpu::BlendState::ALPHA_BLENDING),
            Some(wgpu::Face::Back),
            Some(wgpu::DepthStencilState {
                format: DepthBuffer::FORMAT,
                depth_write_enabled: true,
                depth_compare: wgpu::CompareFunction::Less,
                stencil: Default::default(),
                bias: Default::default(),
            }),
        );
        Self {
            buffer: None,
            uniform,
            program,
            index: 0,
            is_flat: false,
            is_updated: true,
            is_resized: true,
        }
    }

    pub fn selected_block(&self) -> Option<Block> {
        CLIENT_CONFIG.gui.inventory.content.get(self.index).copied()
    }

    pub fn draw<'a>(
        &'a self,
        render_pass: &mut wgpu::RenderPass<'a>,
        textures_bind_group: &'a wgpu::BindGroup,
    ) {
        if let Some(buffer) = &self.buffer {
            self.program.bind(
                render_pass,
                [self.uniform.bind_group(), textures_bind_group],
            );
            buffer.draw(render_pass);
        }
    }

    fn index(&self, keycode: VirtualKeyCode) -> Option<usize> {
        match keycode {
            VirtualKeyCode::Key1 => Some(0),
            VirtualKeyCode::Key2 => Some(1),
            VirtualKeyCode::Key3 => Some(2),
            VirtualKeyCode::Key4 => Some(3),
            VirtualKeyCode::Key5 => Some(4),
            VirtualKeyCode::Key6 => Some(5),
            VirtualKeyCode::Key7 => Some(6),
            VirtualKeyCode::Key8 => Some(7),
            VirtualKeyCode::Key9 => Some(8),
            _ => None,
        }
    }

    fn transform(&self, renderer: &Renderer) -> Matrix4<f32> {
        let scaling = Gui::scaling(renderer, CLIENT_CONFIG.gui.inventory.size);
        Gui::transform(scaling, scaling.map(|c| 1.0 - c * 1.44))
            * if !self.is_flat {
                let diagonal = 3.0f32.sqrt();
                let rot_x = -FRAC_PI_6;
                let theta = (1.0 / diagonal).acos() + rot_x;
                Matrix4::new_rotation(Vector3::x() * rot_x)
                    .append_scaling(1.0 / diagonal / theta.cos())
                    .append_translation(&vector![0.5, 0.5, 0.545])
                    * Matrix4::new_rotation(Vector3::y() * FRAC_PI_4)
                        .prepend_translation(&Vector3::repeat(-0.5))
            } else {
                Matrix4::identity()
            }
    }
}

impl EventHandler for Inventory {
    type Context<'a> = &'a Renderer;

    fn handle(&mut self, event: &Event, renderer: Self::Context<'_>) {
        match event {
            Event::WindowEvent { event, .. } => match *event {
                WindowEvent::KeyboardInput {
                    input:
                        KeyboardInput {
                            virtual_keycode: Some(keycode),
                            state: ElementState::Pressed,
                            ..
                        },
                    ..
                } => {
                    if let Some(idx) = self.index(keycode) {
                        self.is_updated = mem::replace(&mut self.index, idx) != idx;
                    }
                }
                WindowEvent::Resized(PhysicalSize { width, height })
                | WindowEvent::ScaleFactorChanged {
                    new_inner_size: &mut PhysicalSize { width, height },
                    ..
                } if width != 0 && height != 0 => {
                    self.is_resized = true;
                }
                _ => {}
            },
            Event::MainEventsCleared => {
                if mem::take(&mut self.is_updated) {
                    let mut is_flat = false;

                    self.buffer = self.selected_block().and_then(|block| {
                        let data = block.data();
                        VertexBuffer::new_non_empty(
                            renderer,
                            MemoryState::Immutable(&if let Some(vertices) = data.flat_icon() {
                                is_flat = true;
                                vertices.collect::<Vec<_>>()
                            } else {
                                data.vertices(Default::default(), block.into(), Default::default())
                                    .collect()
                            }),
                        )
                    });

                    if mem::replace(&mut self.is_flat, is_flat) != is_flat {
                        self.is_resized = true;
                    }
                }

                if mem::take(&mut self.is_resized) {
                    self.uniform.set(
                        renderer,
                        &InventoryUniformData::new(self.transform(renderer)),
                    );
                }
            }
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
    fn new(transform: Matrix4<f32>) -> Self {
        Self { transform }
    }
}

#[derive(Deserialize)]
pub struct InventoryConfig {
    content: ArrayVec<Block, 9>,
    size: f32,
}
