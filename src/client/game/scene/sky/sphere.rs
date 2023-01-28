use crate::client::renderer::Vertex;
use bytemuck::{Pod, Zeroable};
use nalgebra::{point, Point3};
use std::f32::consts::{PI, TAU};

pub struct Sphere {
    sectors: u16,
    stacks: u16,
}

impl Sphere {
    pub fn new(sectors: u16, stacks: u16) -> Self {
        Self { sectors, stacks }
    }

    pub fn vertices(&self) -> impl Iterator<Item = SphereVertex> + '_ {
        (0..=self.stacks).flat_map(move |y| {
            let lat = PI * (0.5 - y as f32 / self.stacks as f32);
            (0..=self.sectors).map(move |x| {
                let long = TAU * x as f32 / self.sectors as f32;
                SphereVertex::new(
                    point![long.cos() * lat.cos(), lat.sin(), long.sin() * lat.cos()],
                    y as f32 / self.stacks as f32,
                )
            })
        })
    }

    pub fn indices(&self) -> impl Iterator<Item = u16> + '_ {
        (0..self.stacks).flat_map(move |y| {
            (0..self.sectors).flat_map(move |x| {
                let k1 = y * (self.sectors + 1) + x;
                let k2 = (y + 1) * (self.sectors + 1) + x;
                (y != 0)
                    .then_some([k1, k2, k1 + 1])
                    .into_iter()
                    .chain((y != self.stacks - 1).then_some([k1 + 1, k2, k2 + 1]))
                    .flatten()
            })
        })
    }
}

#[repr(C)]
#[derive(Clone, Copy, Zeroable, Pod)]
pub struct SphereVertex {
    coords: Point3<f32>,
    tex_v: f32,
}

impl SphereVertex {
    fn new(coords: Point3<f32>, tex_v: f32) -> Self {
        Self { coords, tex_v }
    }
}

impl Vertex for SphereVertex {
    const ATTRIBS: &'static [wgpu::VertexAttribute] =
        &wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32];
}
