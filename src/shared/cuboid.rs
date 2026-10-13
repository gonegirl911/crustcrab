use nalgebra::{Point3, Vector3, vector};

pub struct Cuboid {
    origin: Point3<i64>,
    diagonal: Vector3<i64>,
}

impl Cuboid {
    pub fn from_corners(a: Point3<i64>, b: Point3<i64>) -> Self {
        let (min, max) = a.inf_sup(&b);
        Self {
            origin: min,
            diagonal: max - min + Vector3::repeat(1),
        }
    }

    pub fn unit() -> Self {
        Self {
            origin: Point3::origin(),
            diagonal: Vector3::repeat(1),
        }
    }

    pub fn scale(mut self, scaling: i64) -> Self {
        self.diagonal.apply(|c| *c *= scaling);
        self
    }

    pub fn pad(mut self, padding: i64) -> Self {
        self.origin.apply(|c| *c -= padding);
        self.diagonal.apply(|c| *c += padding * 2);
        self
    }

    pub fn translate(mut self, translation: Vector3<i64>) -> Self {
        self.origin += translation;
        self
    }

    pub fn into_points(self) -> impl Iterator<Item = Point3<i64>> {
        (0..self.volume()).map(move |i| self.unflatten(i))
    }

    pub fn flatten(&self, coords: Point3<i64>) -> i64 {
        let delta = coords - self.origin;
        delta.x * self.diagonal.y * self.diagonal.z + delta.y * self.diagonal.z + delta.z
    }

    pub fn unflatten(&self, i: i64) -> Point3<i64> {
        let delta = vector![
            i / (self.diagonal.y * self.diagonal.z),
            i % (self.diagonal.y * self.diagonal.z) / self.diagonal.z,
            i % self.diagonal.z,
        ];
        self.origin + delta
    }

    fn volume(&self) -> i64 {
        self.diagonal.x * self.diagonal.y * self.diagonal.z
    }
}
