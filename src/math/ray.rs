use nalgebra::{Point3, Vector3};
use std::ops::Range;

#[derive(Clone, Copy)]
pub struct Ray {
    pub origin: Point3<f32>,
    pub dir: Vector3<f32>,
}

pub trait Hittable {
    fn hit(&self, ray: &Ray, t_range: Range<f32>) -> bool;
}
