use nalgebra::{Point3, Vector3};

pub struct Aabb {
    min: Point3<f32>,
    max: Point3<f32>,
}

impl Aabb {
    pub fn new(origin: Point3<f32>, diagonal: Vector3<f32>) -> Self {
        Self::from_corners(origin, origin + diagonal)
    }

    fn from_corners(a: Point3<f32>, b: Point3<f32>) -> Self {
        Self {
            min: a.inf(&b),
            max: a.sup(&b),
        }
    }

    fn circumcenter(&self) -> Point3<f32> {
        self.min + (self.max - self.min) * 0.5
    }

    fn circumradius(&self) -> f32 {
        (self.max - self.min).magnitude() * 0.5
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
