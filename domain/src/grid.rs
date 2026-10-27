use nalgebra::{Point, SVector};

pub(super) struct Layout<const D: usize> {
    origin: Point<i64, D>,
    extents: SVector<usize, D>,
}

impl<const D: usize> Layout<D> {
    pub(super) fn unit() -> Self {
        Self {
            origin: Default::default(),
            extents: SVector::repeat(1),
        }
    }

    pub(super) fn scale(mut self, scaling: usize) -> Self {
        self.extents.apply(|extent| *extent *= scaling);
        self
    }

    pub(super) fn into_points(self) -> impl Iterator<Item = Point<i64, D>> {
        (0..self.len()).map(move |i| self.unflatten(i))
    }

    pub(super) fn flatten(&self, coords: Point<i64, D>) -> usize {
        let mut index = 0;
        for axis in 0..D {
            let offset = (coords[axis] - self.origin[axis]) as usize;
            index = index * self.extents[axis] + offset;
        }
        index
    }

    fn unflatten(&self, mut index: usize) -> Point<i64, D> {
        let mut coords = self.origin;
        for axis in (0..D).rev() {
            let offset = index % self.extents[axis];
            index /= self.extents[axis];
            coords[axis] += offset as i64;
        }
        coords
    }

    fn len(&self) -> usize {
        self.extents.product()
    }
}

pub(super) type Layout3 = Layout<3>;
