use nalgebra::{Point3, Vector3, point};
use rayon::iter::{IntoParallelIterator, ParallelIterator};

pub struct Cuboid {
    origin: Point3<i64>,
    diagonal: Vector3<i64>,
}

impl Cuboid {
    pub fn from_corners(a: Point3<i64>, b: Point3<i64>) -> Self {
        let min = a.inf(&b);
        let max = a.sup(&b);
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

    pub fn into_points(self) -> impl Iterator<Item = Point3<i64>> {
        (0..self.volume()).map(move |i| self.decompose(i))
    }

    pub fn into_par_points(self) -> impl ParallelIterator<Item = Point3<i64>> {
        (0..self.volume())
            .into_par_iter()
            .map(move |i| self.decompose(i))
    }

    fn decompose(&self, i: i64) -> Point3<i64> {
        point![
            self.origin.x + i / (self.diagonal.y * self.diagonal.z),
            self.origin.y + i % (self.diagonal.y * self.diagonal.z) / self.diagonal.z,
            self.origin.z + i % self.diagonal.z,
        ]
    }

    fn volume(&self) -> i64 {
        self.diagonal.x * self.diagonal.y * self.diagonal.z
    }
}
