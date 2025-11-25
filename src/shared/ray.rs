use nalgebra::{Point3, Vector3};
use std::{cmp::Ordering, iter};

#[derive(Clone, Copy, Default)]
pub struct Ray {
    pub origin: Point3<f32>,
    pub dir: Vector3<f32>,
}

impl Ray {
    pub fn cast(self, reach: f32) -> impl Iterator<Item = BlockIntersection> {
        let calcs = self.origin.coords.zip_map(&self.dir, |o, d| {
            match d.partial_cmp(&0.0).unwrap_or_else(|| unreachable!()) {
                Ordering::Less => (-1, o - o.floor(), 1.0 / -d),
                Ordering::Equal => (0, 1.0, f32::INFINITY),
                Ordering::Greater => (1, if o % 1.0 == 0.0 { 1.0 } else { o.ceil() - o }, 1.0 / d),
            }
        });
        let step = calcs.map(|c| c.0);
        let t_delta = calcs.map(|c| c.2);
        let mut coords = self.origin.map(|c| c.floor() as i64);
        let mut t_max = calcs.map(|c| c.1 * c.2);
        iter::successors(
            Some(BlockIntersection {
                coords,
                normal: Vector3::zeros(),
            }),
            move |_| {
                let i = t_max.imin();
                (t_max[i] <= reach).then(|| {
                    let mut normal = Vector3::zeros();
                    coords[i] += step[i];
                    t_max[i] += t_delta[i];
                    normal[i] -= step[i];
                    BlockIntersection { coords, normal }
                })
            },
        )
    }
}

#[derive(Clone, Copy, PartialEq, Default)]
pub struct BlockIntersection {
    pub coords: Point3<i64>,
    pub normal: Vector3<i64>,
}

pub trait Intersectable {
    fn intersect(&self, ray: Ray) -> Option<f32>;

    fn intersects(&self, ray: Ray) -> bool {
        self.intersect(ray).is_some()
    }
}
