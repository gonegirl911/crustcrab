use super::chunk::Chunk;
use bitfield::bitfield;
use nalgebra::Point3;
use rustc_hash::FxHashMap;
use std::ops::{Index, IndexMut};

#[derive(Default)]
pub struct ChunkMapLight(FxHashMap<Point3<i32>, ChunkLight>);

#[derive(Clone, Default)]
struct ChunkLight([[[BlockLight; Chunk::DIM]; Chunk::DIM]; Chunk::DIM]);

bitfield! {
    #[derive(Clone, Copy, Default)]
    struct BlockLight(u16);
    u8;
    skylight, set_skylight: 3, 0;
    torchlight, set_torchlight: 7, 4, 3;
}
