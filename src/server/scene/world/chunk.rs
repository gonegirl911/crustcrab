use super::{
    block::{Block, BlockArea},
    loader::ChunkLoader,
};
use crate::{
    client::{game::scene::world::block::BlockVertex, ClientEvent},
    server::{
        event_loop::{Event, EventHandler},
        scene::player::{
            ray::{BlockIntersection, Ray},
            Player, WorldArea,
        },
        ServerEvent,
    },
};
use bitvec::prelude::*;
use flume::Sender;
use nalgebra::{point, vector, Point3, Vector3};
use rayon::prelude::*;
use rustc_hash::{FxHashMap, FxHashSet};
use std::{
    array,
    collections::LinkedList,
    num::NonZeroUsize,
    ops::{Index, IndexMut, Range},
    sync::Arc,
};

#[derive(Default)]
pub struct ChunkMap {
    cells: FxHashMap<Point3<i32>, ChunkCell>,
    actions: FxHashMap<Point3<i32>, FxHashMap<Point3<u8>, BlockAction>>,
    loader: ChunkLoader,
}

impl ChunkMap {
    pub const Y_RANGE: Range<i32> = 0..16;

    fn load_many<I>(&mut self, points: I) -> impl Iterator<Item = Point3<i32>> + '_
    where
        I: IntoIterator<Item = Point3<i32>>,
    {
        let points = points.into_iter().collect::<Vec<_>>();

        let new = points
            .iter()
            .copied()
            .filter(|coords| self.cells.get_mut(coords).map(ChunkCell::load).is_none())
            .collect::<Vec<_>>();

        self.cells.extend(
            new.into_par_iter()
                .filter_map(|coords| self.get_new(coords).map(|cell| (coords, cell)))
                .collect::<LinkedList<_>>(),
        );

        points
            .into_iter()
            .filter(|coords| self.cells.contains_key(coords))
    }

    fn unload_many<'a, I>(&'a mut self, points: I) -> impl Iterator<Item = Point3<i32>> + 'a
    where
        I: IntoIterator<Item = Point3<i32>> + 'a,
    {
        points.into_iter().filter(|coords| self.unload(*coords))
    }

    fn send_loads<I>(&self, points: I, server_tx: Sender<ServerEvent>)
    where
        I: IntoParallelIterator<Item = Point3<i32>>,
    {
        points
            .into_par_iter()
            .map(|coords| ServerEvent::ChunkUpdated {
                coords,
                data: Some(Arc::new(ChunkData {
                    chunk: self.cells[&coords].as_ref().clone(),
                    area: self.chunk_area(coords),
                })),
            })
            .collect::<LinkedList<_>>()
            .into_iter()
            .for_each(|event| {
                server_tx.send(event).unwrap_or_else(|_| unreachable!());
            });
    }

    fn send_unloads<I>(&self, points: I, server_tx: Sender<ServerEvent>)
    where
        I: IntoIterator<Item = Point3<i32>>,
    {
        for coords in points {
            server_tx
                .send(ServerEvent::ChunkUpdated { coords, data: None })
                .unwrap_or_else(|_| unreachable!());
        }
    }

    fn send_updates(
        &self,
        loaded: &FxHashSet<Point3<i32>>,
        unloaded: &FxHashSet<Point3<i32>>,
        server_tx: Sender<ServerEvent>,
    ) {
        Self::outline(loaded.union(&unloaded).copied())
            .into_par_iter()
            .filter_map(|coords| {
                Some(ServerEvent::ChunkUpdated {
                    coords,
                    data: Some(Arc::new(ChunkData {
                        chunk: self.cells.get(&coords)?.as_ref().clone(),
                        area: self.chunk_area(coords),
                    })),
                })
            })
            .collect::<LinkedList<_>>()
            .into_iter()
            .for_each(|event| {
                server_tx.send(event).unwrap_or_else(|_| unreachable!());
            });
    }

    fn get_new(&self, coords: Point3<i32>) -> Option<ChunkCell> {
        let mut chunk = self.loader.get(coords);
        self.actions
            .get(&coords)
            .into_iter()
            .flatten()
            .for_each(|(block_coords, action)| {
                chunk.apply(*block_coords, action);
            });
        ChunkCell::load_new(chunk)
    }

    fn unload(&mut self, coords: Point3<i32>) -> bool {
        self.cells
            .remove_entry(&coords)
            .map(|(coords, cell)| {
                if let Some(cell) = cell.unload() {
                    self.cells.insert(coords, cell);
                }
            })
            .is_some()
    }

    fn outline<I: IntoIterator<Item = Point3<i32>>>(points: I) -> FxHashSet<Point3<i32>> {
        let points = points.into_iter().collect::<FxHashSet<_>>();
        points
            .iter()
            .flat_map(|coords| Self::area_deltas().map(move |delta| coords + delta))
            .collect::<FxHashSet<_>>()
            .difference(&points)
            .copied()
            .collect()
    }

    fn chunk_area(&self, coords: Point3<i32>) -> ChunkArea {
        ChunkArea::from_fn(|delta| {
            let chunk_coords = coords + Player::chunk_coords(delta.cast()).coords;
            let block_coords = delta.map(|c| (c + Chunk::DIM as i8) as u8 % Chunk::DIM as u8);
            self.cells
                .get(&chunk_coords)
                .map(|cell| cell[block_coords].is_opaque())
                .unwrap_or_default()
        })
    }

    fn area_deltas() -> impl Iterator<Item = Vector3<i32>> {
        (-1..=1)
            .flat_map(|dx| (-1..=1).flat_map(move |dy| (-1..=1).map(move |dz| vector![dx, dy, dz])))
    }
}

impl EventHandler<ChunkMapEvent> for ChunkMap {
    type Context<'a> = (Sender<ServerEvent>, Ray);

    fn handle(&mut self, event: &ChunkMapEvent, (server_tx, ray): Self::Context<'_>) {
        match event {
            ChunkMapEvent::InitialRenderRequested { area } => {
                let mut loaded = self.load_many(area.points()).collect::<Vec<_>>();

                self.handle(
                    &ChunkMapEvent::BlockSelectionRequested,
                    (server_tx.clone(), ray),
                );

                loaded.par_sort_unstable_by_key(|coords| {
                    (coords - area.center).map(|c| c.pow(2)).sum()
                });

                self.send_loads(loaded, server_tx);
            }
            ChunkMapEvent::WorldAreaChanged { prev, curr } => {
                let unloaded = self
                    .unload_many(prev.exclusive_points(curr))
                    .collect::<FxHashSet<_>>();

                let loaded = self
                    .load_many(curr.exclusive_points(prev))
                    .collect::<FxHashSet<_>>();

                self.handle(
                    &ChunkMapEvent::BlockSelectionRequested,
                    (server_tx.clone(), ray),
                );

                self.send_updates(&loaded, &unloaded, server_tx.clone());
                self.send_unloads(unloaded, server_tx.clone());
                self.send_loads(loaded, server_tx);
            }
            ChunkMapEvent::BlockSelectionRequested => {
                server_tx
                    .send(ServerEvent::BlockSelected {
                        data: ray.cast(Player::BUILDING_REACH).find(
                            |BlockIntersection { coords, .. }| {
                                let coords = coords.cast();
                                let chunk_coords = Player::chunk_coords(coords);
                                let block_coords = Player::block_coords(coords);
                                self.cells
                                    .get(&chunk_coords)
                                    .map(|cell| cell[block_coords].is_not_air())
                                    .unwrap_or_default()
                            },
                        ),
                    })
                    .unwrap_or_else(|_| unreachable!());
            }
            ChunkMapEvent::BlockDestroyed { data } => {}
            ChunkMapEvent::BlockPlaced { block, data } => {}
        }
    }
}

struct ChunkCell {
    chunk: Box<Chunk>,
    players_count: usize,
}

impl ChunkCell {
    fn load_new(chunk: Chunk) -> Option<Self> {
        chunk.is_not_empty().then_some(Self {
            chunk: Box::new(chunk),
            players_count: 1,
        })
    }

    fn load(&mut self) {
        self.players_count += 1;
    }

    fn unload(self) -> Option<Self> {
        Some(Self {
            players_count: NonZeroUsize::new(self.players_count - 1)?.get(),
            ..self
        })
    }

    fn apply(mut self, coords: Point3<u8>, action: &BlockAction) -> Option<Self> {
        self.chunk.apply(coords, action);
        self.chunk.is_not_empty().then_some(self)
    }
}

impl AsRef<Chunk> for ChunkCell {
    fn as_ref(&self) -> &Chunk {
        &self.chunk
    }
}

impl Index<Point3<u8>> for ChunkCell {
    type Output = Block;

    fn index(&self, coords: Point3<u8>) -> &Self::Output {
        &self.chunk[coords]
    }
}

impl IndexMut<Point3<u8>> for ChunkCell {
    fn index_mut(&mut self, coords: Point3<u8>) -> &mut Self::Output {
        &mut self.chunk[coords]
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

enum BlockAction {
    Destroy,
    Place(Block),
}

#[derive(Clone, Default)]
pub struct Chunk([[[Block; Self::DIM]; Self::DIM]; Self::DIM]);

impl Chunk {
    pub const DIM: usize = 16;

    pub fn from_fn<F: FnMut(Point3<u8>) -> Block>(mut f: F) -> Self {
        Self(array::from_fn(|x| {
            array::from_fn(|y| array::from_fn(|z| f(point![x as u8, y as u8, z as u8])))
        }))
    }

    fn vertices<'a>(&'a self, area: &'a ChunkArea) -> impl Iterator<Item = BlockVertex> + 'a {
        self.blocks().flat_map(|(coords, block)| {
            block.vertices(coords, unsafe { area.block_area_unchecked(coords) })
        })
    }

    fn apply(&mut self, coords: Point3<u8>, action: &BlockAction) {
        match action {
            BlockAction::Destroy => {
                self[coords] = Block::Air;
            }
            BlockAction::Place(block) => {
                self[coords] = *block;
            }
        }
    }

    fn is_empty(&self) -> bool {
        self.0
            .iter()
            .flatten()
            .flatten()
            .copied()
            .all(Block::is_air)
    }

    fn is_not_empty(&self) -> bool {
        !self.is_empty()
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
}

impl Index<Point3<u8>> for Chunk {
    type Output = Block;

    fn index(&self, coords: Point3<u8>) -> &Self::Output {
        &self.0[coords.x as usize][coords.y as usize][coords.z as usize]
    }
}

impl IndexMut<Point3<u8>> for Chunk {
    fn index_mut(&mut self, coords: Point3<u8>) -> &mut Self::Output {
        &mut self.0[coords.x as usize][coords.y as usize][coords.z as usize]
    }
}

struct ChunkArea(BitArr!(for Self::DIM * Self::DIM * Self::DIM, in usize));

impl ChunkArea {
    const DIM: usize = (Self::RANGE.end - Self::RANGE.start) as usize;
    const RANGE: Range<i8> = -1..Chunk::DIM as i8 + 1;

    fn from_fn<F: FnMut(Point3<i8>) -> bool>(mut f: F) -> Self {
        let mut data = BitArray::ZERO;
        for x in Self::RANGE {
            for y in Self::RANGE {
                for z in Self::RANGE {
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
        let coords = coords.map(|c| (c - Self::RANGE.start) as usize);
        coords.x * Self::DIM.pow(2) + coords.y * Self::DIM + coords.z
    }
}

pub enum ChunkMapEvent {
    InitialRenderRequested {
        area: WorldArea,
    },
    WorldAreaChanged {
        prev: WorldArea,
        curr: WorldArea,
    },
    BlockSelectionRequested,
    BlockDestroyed {
        data: BlockIntersection,
    },
    BlockPlaced {
        block: Block,
        data: BlockIntersection,
    },
}

impl ChunkMapEvent {
    pub fn new(event: &Event, Player { prev, curr, .. }: &Player) -> Option<Self> {
        if let Event::ClientEvent(event) = event {
            match event {
                ClientEvent::InitialRenderRequested { .. } => {
                    Some(Self::InitialRenderRequested { area: *curr })
                }
                ClientEvent::PlayerOrientationChanged { .. } => Some(Self::BlockSelectionRequested),
                ClientEvent::PlayerPositionChanged { .. } if curr != prev => {
                    Some(Self::WorldAreaChanged {
                        prev: *prev,
                        curr: *curr,
                    })
                }
                ClientEvent::PlayerPositionChanged { .. } => None,
                ClientEvent::BlockDestroyed { data } => Some(Self::BlockDestroyed { data: *data }),
                ClientEvent::BlockPlaced { block, data } => Some(Self::BlockPlaced {
                    block: *block,
                    data: *data,
                }),
            }
        } else {
            None
        }
    }
}
