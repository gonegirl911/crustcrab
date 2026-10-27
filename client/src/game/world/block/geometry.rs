use enum_map::{Enum, EnumMap, enum_map};
use nalgebra::Vector3;
use serde::{Deserialize, Serialize};
use std::sync::LazyLock;

#[derive(Clone, Copy, PartialEq, Enum, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(in crate::game) enum Side {
    Front,
    Right,
    Back,
    Left,
    Top,
    Bottom,
}

#[derive(Clone, Copy, Enum, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(in crate::game) enum Corner {
    LowerLeft,
    LowerRight,
    UpperRight,
    UpperLeft,
}

pub(in crate::game) static SIDE_DELTAS: LazyLock<EnumMap<Side, Vector3<i8>>> =
    LazyLock::new(|| {
        enum_map! {
            Side::Front => -Vector3::z(),
            Side::Right => Vector3::x(),
            Side::Back => Vector3::z(),
            Side::Left => -Vector3::x(),
            Side::Top => Vector3::y(),
            Side::Bottom => -Vector3::y(),
        }
    });

static SIDE_CORNER_SIDES: LazyLock<EnumMap<Side, EnumMap<Corner, [Side; 2]>>> =
    LazyLock::new(|| {
        enum_map! {
            Side::Front => enum_map! {
                Corner::LowerLeft => [Side::Bottom, Side::Left],
                Corner::LowerRight => [Side::Bottom, Side::Right],
                Corner::UpperRight => [Side::Top, Side::Right],
                Corner::UpperLeft => [Side::Top, Side::Left],
            },
            Side::Right => enum_map! {
                Corner::LowerLeft => [Side::Bottom, Side::Front],
                Corner::LowerRight => [Side::Bottom, Side::Back],
                Corner::UpperRight => [Side::Top, Side::Back],
                Corner::UpperLeft => [Side::Top, Side::Front],
            },
            Side::Back => enum_map! {
                Corner::LowerLeft => [Side::Bottom, Side::Right],
                Corner::LowerRight => [Side::Bottom, Side::Left],
                Corner::UpperRight => [Side::Top, Side::Left],
                Corner::UpperLeft => [Side::Top, Side::Right],
            },
            Side::Left => enum_map! {
                Corner::LowerLeft => [Side::Bottom, Side::Back],
                Corner::LowerRight => [Side::Bottom, Side::Front],
                Corner::UpperRight => [Side::Top, Side::Front],
                Corner::UpperLeft => [Side::Top, Side::Back],
            },
            Side::Top => enum_map! {
                Corner::LowerLeft => [Side::Front, Side::Left],
                Corner::LowerRight => [Side::Front, Side::Right],
                Corner::UpperRight => [Side::Back, Side::Right],
                Corner::UpperLeft => [Side::Back, Side::Left],
            },
            Side::Bottom => enum_map! {
                Corner::LowerLeft => [Side::Back, Side::Left],
                Corner::LowerRight => [Side::Back, Side::Right],
                Corner::UpperRight => [Side::Front, Side::Right],
                Corner::UpperLeft => [Side::Front, Side::Left],
            },
        }
    });

pub(in crate::game) static SIDE_CORNER_DELTAS: LazyLock<
    EnumMap<Side, EnumMap<Corner, Vector3<u8>>>,
> = LazyLock::new(|| {
    SIDE_CORNER_SIDES.map(|s1, corner_sides| {
        corner_sides.map(|_, [s2, s3]| {
            (SIDE_DELTAS[s1] + SIDE_DELTAS[s2] + SIDE_DELTAS[s3]).map(|c| c.max(0) as u8)
        })
    })
});

pub(in crate::game) const LL_UR_TRIANGULATION: [Corner; 6] = [
    Corner::LowerLeft,
    Corner::LowerRight,
    Corner::UpperRight,
    Corner::LowerLeft,
    Corner::UpperRight,
    Corner::UpperLeft,
];
