use super::{Position, Position2, coord::Coord};
use nalgebra::{Const, DimNameSub, U2, U3, point, vector};

macro_rules! impl_swizzle {
    ($(where $BaseDim:ident:
        $($name:ident() -> $Result:ident[$($i:expr),+]),+;
    )*) => {
        $(
            $(
                pub fn $name(&self) -> $Result
                where
                    Const<D>: DimNameSub<$BaseDim>,
                {
                    Position::<{ [$($i),+].len() }> {
                        coords: point![$(self.coords[$i]),+],
                        fract: vector![$(self.fract[$i]),+],
                    }
                    .into()
                }
            )+
        )*
    };
}

impl<const D: usize> Position<D> {
    impl_swizzle! {
        where U2:
            y() -> Coord[1];
        where U3:
            xz() -> Position2[0, 2];
    }
}
