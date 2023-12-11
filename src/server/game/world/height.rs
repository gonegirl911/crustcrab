use nalgebra::Point2;
use std::ops::Index;

#[derive(Default)]
pub struct HeightMap;

impl Index<Point2<i32>> for HeightMap {
    type Output = i32;

    fn index(&self, _: Point2<i32>) -> &Self::Output {
        &3
    }
}
