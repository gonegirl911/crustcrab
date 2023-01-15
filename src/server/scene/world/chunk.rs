use super::{
    block::{Block, BlockArea},
    generator::ChunkGenerator,
};
use crate::{
    client::{game::scene::world::block::BlockVertex, ClientEvent},
    math::{
        bounds::{Aabb, BoundingSphere},
        ray::Ray,
    },
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
    chunks: FxHashMap<Point3<i32>, Box<Chunk>>,
    generator: ChunkGenerator,
}

impl ChunkMap {
    pub const LOWER_LIMIT: i32 = 0;
    pub const UPPER_LIMIT: i32 = 15;

    fn insert(
        &mut self,
        coords: Point3<i32>,
        chunk: Box<Chunk>,
    ) -> impl Iterator<Item = Point3<i32>> {
        self.chunks.insert(coords, chunk);
        Self::area_deltas().map(move |delta| coords + delta)
    }

    fn remove(&mut self, coords: Point3<i32>) -> Option<impl Iterator<Item = Point3<i32>>> {
        self.chunks
            .remove(&coords)
            .map(|_| Self::neighbor_deltas().map(move |delta| coords + delta))
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
    type Context<'a> = Sender<ServerEvent>;

    fn handle(&mut self, event: &ChunkMapEvent, server_tx: Self::Context<'_>) {
        let prev = std::time::Instant::now();

        match event {
            ChunkMapEvent::InitialRenderRequested { area } => {
                self.chunks
                    .par_extend(area.par_points().filter_map(|coords| {
                        Some((coords, Box::new(self.generator.get(coords)?)))
                    }));

                self.chunks.par_iter().for_each(|(coords, chunk)| {
                    server_tx
                        .send(ServerEvent::ChunkUpdated {
                            coords: *coords,
                            data: Some(Arc::new(ChunkData {
                                chunk: (**chunk).clone(),
                                area: self.chunk_area(*coords),
                            })),
                        })
                        .unwrap_or_else(|_| unreachable!());
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
                                .unwrap_or_else(|_| unreachable!());
                        })
                        .filter_map(|coords| self.remove(coords))
                        .flatten(),
                );

                updated.extend(
                    curr.par_exclusive_points(prev)
                        .filter_map(|coords| Some((coords, Box::new(self.generator.get(coords)?))))
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
                                    chunk: (**chunk).clone(),
                                    area: self.chunk_area(*coords),
                                })),
                            })
                            .unwrap_or_else(|_| unreachable!());
                    });
            }
            ChunkMapEvent::BlockDestroyed { ray } => {
                todo!()
            }
            ChunkMapEvent::BlockPlaced { ray } => {
                todo!();
            }
        }

        dbg!(prev.elapsed());
    }
}

pub struct ChunkData {
    chunk: Chunk,
    area: ChunkArea,
}

impl ChunkData {
    pub fn vertices(&self) -> impl Iterator<Item = BlockVertex> + '_ {
        self.chunk.vertices(&self.area)
    }
}

#[derive(Clone, Default)]
pub struct Chunk([[[Block; Self::DIM]; Self::DIM]; Self::DIM]);

impl Chunk {
    pub const DIM: usize = 16;

    pub fn from_fn<F: FnMut(Point3<u8>) -> Block>(mut f: F) -> Option<Self> {
        let blocks = array::from_fn(|x| {
            array::from_fn(|y| array::from_fn(|z| f(point![x as u8, y as u8, z as u8])))
        });
        blocks
            .iter()
            .flatten()
            .flatten()
            .copied()
            .any(Block::is_not_air)
            .then_some(Self(blocks))
    }

    fn vertices<'a>(&'a self, area: &'a ChunkArea) -> impl Iterator<Item = BlockVertex> + 'a {
        self.blocks().flat_map(|(coords, block)| {
            block.vertices(coords, unsafe { area.block_area_unchecked(coords) })
        })
    }

    fn bounding_box(coords: Point3<i32>) -> Aabb {
        let dim = Chunk::DIM as i32;
        Aabb {
            min: (coords * dim).cast(),
            max: (coords.map(|c| c + 1) * dim).cast(),
        }
    }

    pub fn bounding_sphere(coords: Point3<i32>) -> BoundingSphere {
        let dim = Chunk::DIM as f32;
        BoundingSphere {
            center: coords.map(|c| (c as f32 + 0.5)) * dim,
            radius: dim * 3.0f32.sqrt() / 2.0,
        }
    }

    unsafe fn get_unchecked(&self, coords: Point3<u8>) -> Block {
        unsafe {
            *self
                .0
                .get_unchecked(coords.x as usize)
                .get_unchecked(coords.y as usize)
                .get_unchecked(coords.z as usize)
        }
    }

    fn blocks(&self) -> impl Iterator<Item = (Point3<u8>, &Block)> {
        self.0.iter().zip(0..).flat_map(|(blocks, x)| {
            blocks.iter().zip(0..).flat_map(move |(blocks, y)| {
                blocks
                    .iter()
                    .zip(0..)
                    .map(move |(block, z)| (point![x, y, z], block))
            })
        })
    }

    fn blocks_mut(&mut self) -> impl Iterator<Item = (Point3<u8>, &mut Block)> {
        self.0.iter_mut().zip(0..).flat_map(|(blocks, x)| {
            blocks.iter_mut().zip(0..).flat_map(move |(blocks, y)| {
                blocks
                    .iter_mut()
                    .zip(0..)
                    .map(move |(block, z)| (point![x, y, z], block))
            })
        })
    }

    fn par_blocks_mut(&mut self) -> impl ParallelIterator<Item = (Point3<u8>, &mut Block)> {
        self.0.par_iter_mut().enumerate().flat_map(|(x, blocks)| {
            blocks
                .par_iter_mut()
                .enumerate()
                .flat_map(move |(y, blocks)| {
                    blocks
                        .par_iter_mut()
                        .enumerate()
                        .map(move |(z, block)| (point![x, y, z].cast(), block))
                })
        })
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

pub enum ChunkMapEvent {
    InitialRenderRequested { area: WorldArea },
    WorldAreaChanged { prev: WorldArea, curr: WorldArea },
    BlockDestroyed { ray: Ray },
    BlockPlaced { ray: Ray },
}

impl ChunkMapEvent {
    pub fn new(event: &Event, Player { prev, curr }: &Player) -> Option<Self> {
        if let Event::ClientEvent(event) = event {
            match event {
                ClientEvent::InitialRenderRequested { .. } => {
                    Some(ChunkMapEvent::InitialRenderRequested { area: *curr })
                }
                ClientEvent::PlayerPositionChanged { .. } if curr != prev => {
                    Some(ChunkMapEvent::WorldAreaChanged {
                        prev: *prev,
                        curr: *curr,
                    })
                }
                ClientEvent::BlockDestroyed { ray } => {
                    Some(ChunkMapEvent::BlockDestroyed { ray: *ray })
                }
                ClientEvent::BlockPlaced { ray } => Some(ChunkMapEvent::BlockPlaced { ray: *ray }),
                _ => None,
            }
        } else {
            None
        }
    }
}
