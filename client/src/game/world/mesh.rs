use super::block::geometry::Side;
use enum_map::Enum;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Enum, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(in crate::game) enum SideShade {
    X = 0,
    Top = 1,
    Bottom = 2,
    Z = 3,
}

impl From<Side> for SideShade {
    fn from(side: Side) -> Self {
        match side {
            Side::Left | Side::Right => Self::X,
            Side::Top => Self::Top,
            Side::Bottom => Self::Bottom,
            Side::Front | Side::Back => Self::Z,
        }
    }
}
