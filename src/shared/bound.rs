use super::ray::{Intersectable, Ray};
use nalgebra::{Matrix4, Point3, Vector3};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Default, Serialize, Deserialize)]
pub struct Aabb {
    min: Point3<f64>,
    max: Point3<f64>,
}

impl Aabb {
    pub fn new(origin: Point3<f64>, diagonal: Vector3<f64>) -> Self {
        Self::from_corners(origin, origin + diagonal)
    }

    fn from_corners(a: Point3<f64>, b: Point3<f64>) -> Self {
        let (min, max) = a.inf_sup(&b);
        Self { min, max }
    }

    pub fn pad(mut self, padding: f64) -> Self {
        self.min.apply(|c| *c -= padding);
        self.max.apply(|c| *c += padding);
        self
    }

    pub fn translate(mut self, translation: Vector3<f64>) -> Self {
        self.min += translation;
        self.max += translation;
        self
    }

    pub fn to_homogeneous(self) -> Matrix4<f64> {
        Matrix4::new_translation(&self.min.coords).prepend_nonuniform_scaling(&self.diagonal())
    }

    fn circumcenter(&self) -> Point3<f64> {
        self.min + self.diagonal() * 0.5
    }

    fn circumradius(&self) -> f64 {
        self.diagonal().magnitude() * 0.5
    }

    fn diagonal(&self) -> Vector3<f64> {
        self.max - self.min
    }
}

impl Intersectable for Aabb {
    fn intersect(&self, ray: Ray) -> Option<f64> {
        let (t_min, t_max) =
            (0..3).fold((f64::NEG_INFINITY, f64::INFINITY), |(t_min, t_max), i| {
                let t1 = (self.min[i] - ray.origin[i]) / ray.dir[i];
                let t2 = (self.max[i] - ray.origin[i]) / ray.dir[i];
                (t_min.max(t1.min(t2)), t_max.min(t1.max(t2)))
            });
        (t_min <= t_max).then_some(t_min)
    }
}

pub struct BoundingSphere {
    pub center: Point3<f64>,
    pub radius: f32,
}

impl From<Aabb> for BoundingSphere {
    fn from(aabb: Aabb) -> Self {
        Self {
            center: aabb.circumcenter(),
            radius: aabb.circumradius() as f32,
        }
    }
}
