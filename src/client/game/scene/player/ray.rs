use nalgebra::{Point3, Vector3};
use std::{cmp::Ordering, iter};

#[derive(Clone, Copy)]
pub struct Ray {
    pub origin: Point3<f32>,
    pub dir: Vector3<f32>,
}

impl Ray {
    pub fn points(&self, reach: f32) -> impl Iterator<Item = Point3<i32>> + '_ {
        let values = self.origin.coords.zip_map(&self.dir, |o, d| {
            match d.partial_cmp(&0.0).unwrap_or_else(|| unreachable!()) {
                Ordering::Less => (-1, o - o.floor(), 1.0 / -d),
                Ordering::Equal => (0, 1.0, f32::INFINITY),
                Ordering::Greater => (1, if o == 0.0 { 1.0 } else { o.ceil() - o }, 1.0 / d),
            }
        });
        let curr = self.origin.map(|c| c.floor() as i32);
        let step = values.map(|c| c.0);
        let tmax = values.map(|c| c.1 * c.2);
        let tdelta = values.map(|c| c.2);
        iter::successors(Some((curr, tmax)), move |(curr, tmax)| {
            let i = tmax.imin();
            (tmax[i] <= reach).then(|| {
                let mut curr = *curr;
                let mut tmax = *tmax;
                curr[i] += step[i];
                tmax[i] += tdelta[i];
                (curr, tmax)
            })
        })
        .map(|(curr, _)| curr)
    }
}
