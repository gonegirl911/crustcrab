use nalgebra::{Point3, Vector3};
use std::{cmp::Ordering, iter, ops::RangeBounds};

#[derive(Clone, Copy, Default)]
pub struct Ray {
    pub origin: Point3<f32>,
    pub dir: Vector3<f32>,
}

impl Ray {
    pub fn cast<R>(self, reach: R) -> impl Iterator<Item = BlockIntersection>
    where
        R: RangeBounds<f32>,
    {
        let values = self.origin.coords.zip_map(&self.dir, |o, d| {
            match d.partial_cmp(&0.0).unwrap_or_else(|| unreachable!()) {
                Ordering::Less => (-1, o - o.floor(), 1.0 / -d),
                Ordering::Equal => (0, 1.0, f32::INFINITY),
                Ordering::Greater => (1, if o == 0.0 { 1.0 } else { o.ceil() - o }, 1.0 / d),
            }
        });
        let mut coords = self.origin.map(|c| c.floor() as i64);
        let step = values.map(|c| c.0);
        let t_delta = values.map(|c| c.2);
        let mut t_max = values.map(|c| c.1 * c.2);
        iter::successors(
            Some(BlockIntersection::new(coords, Vector3::zeros())),
            move |_| {
                let i = t_max.imin();
                reach.contains(&t_max[i]).then(|| {
                    let mut normal = Vector3::zeros();
                    coords[i] += step[i];
                    t_max[i] += t_delta[i];
                    normal[i] -= step[i];
                    BlockIntersection::new(coords, normal)
                })
            },
        )
    }
}

#[derive(Clone, Copy, PartialEq)]
pub struct BlockIntersection {
    pub coords: Point3<i64>,
    pub normal: Vector3<i64>,
}

impl BlockIntersection {
    fn new(coords: Point3<i64>, normal: Vector3<i64>) -> Self {
        Self { coords, normal }
    }
}

pub trait Hittable {
    fn hit(&self, ray: Ray) -> bool;
}
