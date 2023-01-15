use super::ray::{Hittable, Ray};
use nalgebra::Point3;
use std::ops::Range;

pub struct BoundingSphere {
    pub center: Point3<f32>,
    pub radius: f32,
}

pub struct Aabb {
    pub min: Point3<f32>,
    pub max: Point3<f32>,
}

impl Hittable for Aabb {
    fn hit(&self, ray: &Ray, Range { start, end }: Range<f32>) -> bool {
        (0..3)
            .try_for_each(|i| {
                let inv_d = 1.0 / ray.dir[i];
                let t0 = (self.min[i] - ray.origin[i]) * inv_d;
                let t1 = (self.max[i] - ray.origin[i]) * inv_d;
                let (t0, t1) = if inv_d < 0.0 { (t1, t0) } else { (t0, t1) };
                (t0.max(start) < t1.min(end)).then_some(())
            })
            .is_some()
    }
}
