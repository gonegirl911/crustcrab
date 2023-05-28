use super::Gui;
use crate::{
    client::{
        event_loop::{Event, EventHandler},
        game::world::BlockVertex,
        renderer::{
            effect::PostProcessor,
            mesh::{Mesh, Vertex},
            program::Program,
            texture::screen::DepthBuffer,
            uniform::Uniform,
            Renderer,
        },
    },
    server::game::world::block::Block,
};
use arrayvec::ArrayVec;
use bytemuck::{Pod, Zeroable};
use nalgebra::{vector, Matrix4, Vector3};
use std::{
    f32::consts::{FRAC_PI_4, FRAC_PI_6},
    mem,
};
use winit::{
    dpi::PhysicalSize,
    event::{ElementState, KeyboardInput, VirtualKeyCode, WindowEvent},
};

pub struct Inventory {
    mesh: Option<Mesh<BlockVertex>>,
    uniform: Uniform<InventoryUniformData>,
    program: Program,
    inventory: ArrayVec<Block, 9>,
    index: usize,
    is_updated: bool,
    is_resized: bool,
}

impl Inventory {
    pub fn new(
        renderer: &Renderer,
        inventory: ArrayVec<Block, 9>,
        textures_bind_group_layout: &wgpu::BindGroupLayout,
    ) -> Self {
        let uniform = Uniform::new(renderer, None, wgpu::ShaderStages::VERTEX);
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
            mesh: None,
            uniform,
            program,
            inventory,
            index: 0,
            is_updated: true,
            is_resized: true,
        }
    }

    pub fn draw<'a>(
        &'a self,
        render_pass: &mut wgpu::RenderPass<'a>,
        textures_bind_group: &'a wgpu::BindGroup,
    ) {
        if let Some(mesh) = &self.mesh {
            self.program.bind(
                render_pass,
                [self.uniform.bind_group(), textures_bind_group],
            );
            mesh.draw(render_pass);
        }
    }

    pub fn selected_block(&self) -> Option<Block> {
        self.inventory.get(self.index).copied()
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

    fn transform(renderer @ Renderer { config, .. }: &Renderer) -> Matrix4<f32> {
        let diagonal = 3.0f32.sqrt();
        let size = Gui::element_size(renderer, 2.3 * diagonal);
        let left = config.width as f32 - size * 1.3;
        let bottom = config.height as f32 - size * 1.3;
        Gui::viewport(renderer)
            * Matrix4::new_translation(&vector![left, bottom, 0.0])
            * Gui::element_scaling(size)
            * Matrix4::new_translation(&Vector3::from_element(0.5))
            * Matrix4::new_rotation(Vector3::x() * -FRAC_PI_6)
            * Matrix4::new_rotation(Vector3::y() * FRAC_PI_4)
            * Matrix4::new_scaling(1.0 / diagonal)
            * Matrix4::new_translation(&Vector3::from_element(-0.5))
    }
}

impl EventHandler for Inventory {
    type Context<'a> = &'a Renderer;

    fn handle(&mut self, event: &Event, renderer: Self::Context<'_>) {
        match event {
            Event::WindowEvent { event, .. } => match event {
                WindowEvent::KeyboardInput {
                    input:
                        KeyboardInput {
                            virtual_keycode: Some(keycode),
                            state: ElementState::Pressed,
                            ..
                        },
                    ..
                } => {
                    if let Some(idx) = self.index(*keycode) {
                        self.is_updated = mem::replace(&mut self.index, idx) != idx;
                    }
                }
                WindowEvent::Resized(PhysicalSize { width, height })
                | WindowEvent::ScaleFactorChanged {
                    new_inner_size: PhysicalSize { width, height },
                    ..
                } if *width != 0 && *height != 0 => {
                    self.is_resized = true;
                }
                _ => {}
            },
            Event::MainEventsCleared => {
                if mem::take(&mut self.is_updated) {
                    self.mesh = self.selected_block().map(|block| {
                        Mesh::new(
                            renderer,
                            &block
                                .vertices(
                                    Default::default(),
                                    Default::default(),
                                    Default::default(),
                                )
                                .into_iter()
                                .flatten()
                                .collect::<Vec<_>>(),
                        )
                    });
                }

                if mem::take(&mut self.is_resized) {
                    self.uniform.write(
                        renderer,
                        &InventoryUniformData::new(Self::transform(renderer)),
                    )
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
