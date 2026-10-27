use base::color::Rgb;
use bytemuck::{Pod, Zeroable};
use nalgebra::{Point3, Vector3};

#[derive(Clone, Copy, Default, Zeroable, Pod)]
#[repr(C, align(16))]
pub(crate) struct Float3 {
    data: [f32; 3],
    padding: f32,
}

impl From<Vector3<f32>> for Float3 {
    fn from(vector: Vector3<f32>) -> Self {
        Self {
            data: vector.into(),
            ..Default::default()
        }
    }
}

impl From<Point3<f32>> for Float3 {
    fn from(point: Point3<f32>) -> Self {
        Self {
            data: point.into(),
            ..Default::default()
        }
    }
}

impl From<Rgb<f32>> for Float3 {
    fn from(rgb: Rgb<f32>) -> Self {
        Self {
            data: rgb.into(),
            ..Default::default()
        }
    }
}
