use nalgebra::{Point3, Vector3};

#[derive(Clone, Copy)]
pub struct Ray {
    pub origin: Point3<f32>,
    pub dir: Vector3<f32>,
}
