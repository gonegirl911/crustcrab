use super::{
    block::{Block, BlockArea},
    generator::ChunkGenerator,
};
use crate::{
    client::{game::scene::world::block::BlockVertex, ClientEvent},
    server::{
        event_loop::{Event, EventHandler},
        scene::player::{Player, WorldArea},
        ServerEvent,
    },
};
use bitvec::prelude::*;
use flume::Sender;
use nalgebra::{point, vector, Point3, Vector3};
use rayon::prelude::*;
use rustc_hash::{FxHashMap, FxHashSet};
use std::{array, collections::LinkedList, sync::Arc};

#[derive(Default)]
pub struct ChunkMap {
    chunks: FxHashMap<Point3<i32>, Arc<Chunk>>,
}

impl ChunkMap {
    pub const LOWER_LIMIT: i32 = 0;
    pub const UPPER_LIMIT: i32 = 15;

    fn insert(
        &mut self,
        coords: Point3<i32>,
        chunk: Arc<Chunk>,
    ) -> impl Iterator<Item = Point3<i32>> {
        assert!(self.chunks.insert(coords, chunk).is_none());
        Self::area_deltas().map(move |delta| coords + delta)
    }

    fn remove(&mut self, coords: Point3<i32>) -> impl Iterator<Item = Point3<i32>> {
        assert!(self.chunks.remove(&coords).is_some());
        Self::neighbor_deltas().map(move |delta| coords + delta)
    }

    fn chunk_area(&self, coords: Point3<i32>) -> ChunkArea {
        ChunkArea::from_fn(|delta| {
            let chunk_coords = coords + Player::chunk_coords(delta.cast()).coords;
            let block_coords = delta.map(|c| (c + Chunk::DIM as i8) as u8 % Chunk::DIM as u8);
            self.chunks
                .get(&chunk_coords)
                .map(|chunk| unsafe { chunk.get_unchecked(block_coords) }.is_opaque())
                .unwrap_or_default()
        })
    }

    fn area_deltas() -> impl Iterator<Item = Vector3<i32>> {
        (-1..=1)
            .flat_map(|dx| (-1..=1).flat_map(move |dy| (-1..=1).map(move |dz| vector![dx, dy, dz])))
    }

    fn neighbor_deltas() -> impl Iterator<Item = Vector3<i32>> {
        Self::area_deltas().filter(|delta| *delta != Vector3::zeros())
    }
}

impl EventHandler<ChunkMapEvent> for ChunkMap {
    type Context<'a> = (Sender<ServerEvent>, &'a ChunkGenerator);

    fn handle(&mut self, event: &ChunkMapEvent, (server_tx, generator): Self::Context<'_>) {
        match event {
            ChunkMapEvent::InitialRenderRequested { area } => {
                self.chunks.par_extend(
                    area.par_points()
                        .map(|coords| (coords, Arc::new(generator.get(coords)))),
                );

                self.chunks.par_iter().for_each(|(coords, chunk)| {
                    server_tx
                        .send(ServerEvent::ChunkUpdated {
                            coords: *coords,
                            data: Some(Arc::new(ChunkData {
                                chunk: chunk.clone(),
                                area: self.chunk_area(*coords),
                            })),
                        })
                        .unwrap_or_else(|_| unreachable!())
                });
            }
            ChunkMapEvent::WorldAreaChanged { prev, curr } => {
                let mut updated = FxHashSet::default();

                updated.extend(
                    prev.exclusive_points(curr)
                        .inspect(|coords| {
                            server_tx
                                .send(ServerEvent::ChunkUpdated {
                                    coords: *coords,
                                    data: None,
                                })
                                .unwrap_or_else(|_| unreachable!())
                        })
                        .flat_map(|coords| self.remove(coords)),
                );

                updated.extend(
                    curr.par_exclusive_points(prev)
                        .map(|coords| (coords, Arc::new(generator.get(coords))))
                        .collect::<LinkedList<_>>()
                        .into_iter()
                        .flat_map(|(coords, chunk)| self.insert(coords, chunk)),
                );

                updated
                    .into_par_iter()
                    .filter_map(|coords| self.chunks.get_key_value(&coords))
                    .for_each(|(coords, chunk)| {
                        server_tx
                            .send(ServerEvent::ChunkUpdated {
                                coords: *coords,
                                data: Some(Arc::new(ChunkData {
                                    chunk: chunk.clone(),
                                    area: self.chunk_area(*coords),
                                })),
                            })
                            .unwrap_or_else(|_| unreachable!());
                    });
            }
        }
    }
}

pub enum ChunkMapEvent {
    InitialRenderRequested { area: WorldArea },
    WorldAreaChanged { prev: WorldArea, curr: WorldArea },
}

impl ChunkMapEvent {
    pub fn new(event: &Event, Player { prev, curr }: &Player) -> Option<Self> {
        match event {
            Event::ClientEvent(ClientEvent::InitialRenderRequested { .. }) => {
                Some(ChunkMapEvent::InitialRenderRequested { area: *curr })
            }
            Event::ClientEvent(ClientEvent::PlayerPositionChanged { .. }) if curr != prev => {
                Some(ChunkMapEvent::WorldAreaChanged {
                    prev: *prev,
                    curr: *curr,
                })
            }
            _ => None,
        }
    }
}

pub struct ChunkData {
    chunk: Arc<Chunk>,
    area: ChunkArea,
}

impl ChunkData {
    pub fn vertices(&self) -> impl Iterator<Item = BlockVertex> + '_ {
        self.chunk.vertices(&self.area)
    }
}

#[derive(Clone)]
pub struct Chunk {
    blocks: [[[Block; Self::DIM]; Self::DIM]; Self::DIM],
    is_empty: bool,
}

impl Chunk {
    pub const DIM: usize = 16;

    pub fn from_fn<F: FnMut(Point3<u8>) -> Block>(mut f: F) -> Self {
        let mut is_empty = true;
        let blocks = array::from_fn(|x| {
            array::from_fn(|y| {
                array::from_fn(|z| {
                    let block = f(point![x as u8, y as u8, z as u8]);
                    is_empty = !(!is_empty || !matches!(block, Block::Air));
                    block
                })
            })
        });
        Self { blocks, is_empty }
    }

    fn vertices<'a>(&'a self, area: &'a ChunkArea) -> impl Iterator<Item = BlockVertex> + 'a {
        (!self.is_empty)
            .then(|| {
                self.blocks.iter().zip(0..).flat_map(move |(blocks, x)| {
                    blocks.iter().zip(0..).flat_map(move |(blocks, y)| {
                        blocks.iter().zip(0..).flat_map(move |(block, z)| {
                            let coords = point![x, y, z];
                            block.vertices(coords, unsafe { area.block_area_unchecked(coords) })
                        })
                    })
                })
            })
            .into_iter()
            .flatten()
    }

    unsafe fn get_unchecked(&self, coords: Point3<u8>) -> Block {
        unsafe {
            *self
                .blocks
                .get_unchecked(coords.x as usize)
                .get_unchecked(coords.y as usize)
                .get_unchecked(coords.z as usize)
        }
    }
}

struct ChunkArea(BitArr!(for Self::DIM * Self::DIM * Self::DIM, in usize));

impl ChunkArea {
    const DIM: usize = (Self::UPPER_BOUND - Self::LOWER_BOUND + 1) as usize;
    const LOWER_BOUND: i8 = -1;
    const UPPER_BOUND: i8 = Chunk::DIM as i8;

    fn from_fn<F: FnMut(Point3<i8>) -> bool>(mut f: F) -> Self {
        let mut data = BitArray::ZERO;
        for x in Self::LOWER_BOUND..=Self::UPPER_BOUND {
            for y in Self::LOWER_BOUND..=Self::UPPER_BOUND {
                for z in Self::LOWER_BOUND..=Self::UPPER_BOUND {
                    let coords = point![x, y, z];
                    unsafe {
                        data.set_unchecked(Self::index_unchecked(coords), f(coords));
                    }
                }
            }
        }
        Self(data)
    }

    unsafe fn block_area_unchecked(&self, coords: Point3<u8>) -> BlockArea {
        let coords = coords.cast();
        BlockArea::from_fn(|delta| unsafe { self.get_unchecked(coords + delta.coords) })
    }

    unsafe fn get_unchecked(&self, coords: Point3<i8>) -> bool {
        unsafe { *self.0.get_unchecked(Self::index_unchecked(coords)) }
    }

    unsafe fn index_unchecked(coords: Point3<i8>) -> usize {
        let coords = coords.map(|c| (c - Self::LOWER_BOUND) as usize);
        coords.x * Self::DIM.pow(2) + coords.y * Self::DIM + coords.z
    }
}
