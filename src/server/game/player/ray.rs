use nalgebra::{Point3, Vector3};
use std::{cmp::Ordering, iter, ops::RangeBounds};

#[derive(Clone, Copy, Default)]
pub struct Ray {
    pub origin: Point3<f32>,
    pub dir: Vector3<f32>,
}

impl Ray {
    pub fn cast<'a, R>(&'a self, reach: R) -> impl Iterator<Item = BlockIntersection> + 'a
    where
        R: RangeBounds<f32> + 'a,
    {
        let values = self.origin.coords.zip_map(&self.dir, |o, d| {
            match d.partial_cmp(&0.0).unwrap_or_else(|| unreachable!()) {
                Ordering::Less => (-1, o - o.floor(), 1.0 / -d),
                Ordering::Equal => (0, 1.0, f32::INFINITY),
                Ordering::Greater => (1, if o == 0.0 { 1.0 } else { o.ceil() - o }, 1.0 / d),
            }
        });
        let coords = self.origin.map(|c| c.floor() as i64);
        let step = values.map(|c| c.0);
        let tmax = values.map(|c| c.1 * c.2);
        let tdelta = values.map(|c| c.2);
        iter::successors(
            Some((coords, tmax, Vector3::zeros())),
            move |(coords, tmax, _)| {
                let i = tmax.imin();
                reach.contains(&tmax[i]).then(|| {
                    let mut coords = *coords;
                    let mut tmax = *tmax;
                    let mut normal = Vector3::zeros();
                    coords[i] += step[i];
                    tmax[i] += tdelta[i];
                    normal[i] -= step[i];
                    (coords, tmax, normal)
                })
            },
        )
        .map(|(coords, _, normal)| BlockIntersection { coords, normal })
    }
}

#[derive(Clone, Copy)]
pub struct BlockIntersection {
    pub coords: Point3<i64>,
    pub normal: Vector3<i64>,
}
