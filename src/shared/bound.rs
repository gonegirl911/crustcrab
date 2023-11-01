use crate::server::game::player::ray::{Hittable, Ray};
use nalgebra::{Matrix4, Point3, Vector3};

#[derive(Clone, Copy, Default)]
pub struct Aabb {
    min: Point3<f32>,
    max: Point3<f32>,
}

impl Aabb {
    pub fn new(origin: Point3<f32>, diagonal: Vector3<f32>) -> Self {
        Self::from_corners(origin, origin + diagonal)
    }

    fn from_corners(a: Point3<f32>, b: Point3<f32>) -> Self {
        let (min, max) = a.inf_sup(&b);
        Self { min, max }
    }

    fn circumcenter(self) -> Point3<f32> {
        self.min + self.diagonal() * 0.5
    }

    fn circumradius(self) -> f32 {
        self.diagonal().magnitude() * 0.5
    }

    pub fn to_homogeneous(self) -> Matrix4<f32> {
        Matrix4::new_translation(&self.min.coords).prepend_nonuniform_scaling(&self.diagonal())
    }

    fn diagonal(self) -> Vector3<f32> {
        self.max - self.min
    }
}

impl Hittable for Aabb {
    fn hit(&self, ray: Ray) -> bool {
        let (t_min, t_max) = (0..3).fold((f32::MIN, f32::MAX), |(t_min, t_max), i| {
            let t1 = (self.min[i] - ray.origin[i]) / ray.dir[i];
            let t2 = (self.max[i] - ray.origin[i]) / ray.dir[i];
            (t_min.max(t1.min(t2)), t_max.min(t1.max(t2)))
        });
        t_min <= t_max
    }
}

pub struct BoundingSphere {
    pub center: Point3<f32>,
    pub radius: f32,
}

impl From<Aabb> for BoundingSphere {
    fn from(aabb: Aabb) -> Self {
        Self {
            center: aabb.circumcenter(),
            radius: aabb.circumradius(),
        }
    }
}
