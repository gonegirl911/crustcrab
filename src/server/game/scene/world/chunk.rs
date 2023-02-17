use super::{
    block::{Block, BlockArea},
    loader::ChunkLoader,
};
use crate::{
    client::{game::scene::world::BlockVertex, ClientEvent},
    server::{
        event_loop::{Event, EventHandler},
        game::player::{
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
    hovered_block: Option<BlockIntersection>,
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
        points.into_iter().filter(|coords| {
            self.cells
                .remove_entry(coords)
                .map(|(coords, cell)| {
                    if let Some(cell) = cell.unload() {
                        self.cells.insert(coords, cell);
                    }
                })
                .is_some()
        })
    }

    fn apply(
        &mut self,
        coords: Point3<i32>,
        action: BlockAction,
        server_tx: Sender<ServerEvent>,
        ray: Ray,
    ) {
        let coords = coords.cast();
        let chunk_coords = Player::chunk_coords(coords);
        let block_coords = Player::block_coords(coords);

        let (cell, is_created) = if let Some(cell) = self.cells.remove(&chunk_coords) {
            (cell.apply(block_coords, &action).map_err(Some), false)
        } else {
            (
                ChunkCell::default_with_action(block_coords, &action).map_err(|_| None),
                true,
            )
        };

        match cell {
            Ok(cell) => {
                let (is_loaded, is_unloaded) = if let Some(cell) = cell {
                    self.cells.insert(chunk_coords, cell);
                    (is_created, false)
                } else {
                    (false, !is_created)
                };

                self.handle(
                    &ChunkMapEvent::BlockSelectionRequested,
                    (server_tx.clone(), ray),
                );

                self.actions
                    .entry(chunk_coords)
                    .or_default()
                    .insert(block_coords, action);

                self.send_updates(
                    Self::area_deltas()
                        .filter(|delta| !is_loaded || *delta != Vector3::zeros())
                        .map(|delta| Player::chunk_coords(coords + delta.cast()))
                        .collect::<FxHashSet<_>>(),
                    server_tx.clone(),
                );

                if is_loaded {
                    self.send_loads([chunk_coords], server_tx);
                } else if is_unloaded {
                    self.send_unloads([chunk_coords], server_tx);
                }
            }
            Err(Some(cell)) => {
                self.cells.insert(chunk_coords, cell);
            }
            Err(None) => {}
        }
    }

    fn send_loads<I>(&self, points: I, server_tx: Sender<ServerEvent>)
    where
        I: IntoIterator<Item = Point3<i32>>,
    {
        points
            .into_iter()
            .map(|coords| ServerEvent::ChunkLoaded {
                coords,
                data: Arc::new(ChunkData {
                    chunk: self.cells[&coords].as_ref().clone(),
                    area: self.chunk_area(coords),
                }),
            })
            .for_each(|event| {
                server_tx.send(event).unwrap_or_else(|_| unreachable!());
            });
    }

    fn par_send_loads<I>(&self, points: I, server_tx: Sender<ServerEvent>)
    where
        I: IntoParallelIterator<Item = Point3<i32>>,
    {
        points
            .into_par_iter()
            .map(|coords| ServerEvent::ChunkLoaded {
                coords,
                data: Arc::new(ChunkData {
                    chunk: self.cells[&coords].as_ref().clone(),
                    area: self.chunk_area(coords),
                }),
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
                .send(ServerEvent::ChunkUnloaded { coords })
                .unwrap_or_else(|_| unreachable!());
        }
    }

    fn send_updates<I: IntoIterator<Item = Point3<i32>>>(
        &self,
        points: I,
        server_tx: Sender<ServerEvent>,
    ) {
        points
            .into_iter()
            .filter_map(|coords| {
                Some(ServerEvent::ChunkUpdated {
                    coords,
                    data: Arc::new(ChunkData {
                        chunk: self.cells.get(&coords)?.as_ref().clone(),
                        area: self.chunk_area(coords),
                    }),
                })
            })
            .for_each(|event| {
                server_tx.send(event).unwrap_or_else(|_| unreachable!());
            });
    }

    fn par_send_updates<I: IntoParallelIterator<Item = Point3<i32>>>(
        &self,
        points: I,
        server_tx: Sender<ServerEvent>,
    ) {
        points
            .into_par_iter()
            .filter_map(|coords| {
                Some(ServerEvent::ChunkUpdated {
                    coords,
                    data: Arc::new(ChunkData {
                        chunk: self.cells.get(&coords)?.as_ref().clone(),
                        area: self.chunk_area(coords),
                    }),
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
            .for_each(|(coords, action)| {
                chunk.apply(*coords, action);
            });
        ChunkCell::load_new(chunk)
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
                .map(|cell| cell[block_coords].data().is_opaque())
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

                self.par_send_loads(loaded, server_tx);
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

                self.par_send_updates(
                    Self::outline(loaded.union(&unloaded).copied()),
                    server_tx.clone(),
                );
                self.send_unloads(unloaded, server_tx.clone());
                self.par_send_loads(loaded, server_tx);
            }
            ChunkMapEvent::BlockSelectionRequested => {
                self.hovered_block = ray.cast(Player::BUILDING_REACH).find(
                    |BlockIntersection { coords, .. }| {
                        let coords = coords.cast();
                        let chunk_coords = Player::chunk_coords(coords);
                        let block_coords = Player::block_coords(coords);
                        self.cells
                            .get(&chunk_coords)
                            .map(|cell| cell[block_coords].is_not_air())
                            .unwrap_or_default()
                    },
                );

                server_tx
                    .send(ServerEvent::BlockHovered {
                        coords: self.hovered_block.map(|data| data.coords),
                    })
                    .unwrap_or_else(|_| unreachable!());
            }
            ChunkMapEvent::BlockDestroyed => {
                if let Some(BlockIntersection { coords, .. }) = self.hovered_block {
                    self.apply(coords, BlockAction::Destroy, server_tx, ray);
                }
            }
            ChunkMapEvent::BlockPlaced { block } => {
                if let Some(BlockIntersection { coords, normal }) = self.hovered_block {
                    self.apply(coords + normal, BlockAction::Place(*block), server_tx, ray);
                }
            }
        }
    }
}

impl Index<Point3<i32>> for ChunkMap {
    type Output = Chunk;

    fn index(&self, coords: Point3<i32>) -> &Self::Output {
        self.cells[&coords].as_ref()
    }
}

struct ChunkCell {
    chunk: Box<Chunk>,
    players_count: usize,
}

impl ChunkCell {
    fn load_new(chunk: Chunk) -> Option<Self> {
        chunk.is_not_empty().then(|| Self {
            chunk: Box::new(chunk),
            players_count: 1,
        })
    }

    fn default_with_action(coords: Point3<u8>, action: &BlockAction) -> Result<Option<Self>, ()> {
        let mut chunk = Chunk::default();
        chunk
            .apply(coords, action)
            .then(|| Self::load_new(chunk))
            .ok_or(())
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

    fn apply(mut self, coords: Point3<u8>, action: &BlockAction) -> Result<Option<Self>, Self> {
        if self.chunk.apply(coords, action) {
            Ok(self.chunk.is_not_empty().then_some(self))
        } else {
            Err(self)
        }
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

    pub fn from_fn<F: FnMut(Point3<u8>) -> Block>(mut f: F) -> Self {
        Self(array::from_fn(|x| {
            array::from_fn(|y| array::from_fn(|z| f(point![x as u8, y as u8, z as u8])))
        }))
    }

    fn vertices<'a>(&'a self, area: &'a ChunkArea) -> impl Iterator<Item = BlockVertex> + 'a {
        self.blocks().flat_map(|(coords, block)| {
            block
                .vertices(coords, unsafe { area.block_area_unchecked(coords) })
                .into_iter()
                .flatten()
        })
    }

    fn apply(&mut self, coords: Point3<u8>, action: &BlockAction) -> bool {
        let prev = &mut self[coords];
        match action {
            BlockAction::Destroy => prev.is_not_air().then(|| *prev = Block::Air).is_some(),
            BlockAction::Place(block) => prev.is_air().then(|| *prev = *block).is_some(),
        }
    }

    fn blocks(&self) -> impl Iterator<Item = (Point3<u8>, &Block)> + '_ {
        self.0.iter().zip(0..).flat_map(move |(blocks, x)| {
            blocks.iter().zip(0..).flat_map(move |(blocks, y)| {
                blocks
                    .iter()
                    .zip(0..)
                    .map(move |(block, z)| (point![x, y, z], block))
            })
        })
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

enum BlockAction {
    Destroy,
    Place(Block),
}

pub enum ChunkMapEvent {
    InitialRenderRequested { area: WorldArea },
    WorldAreaChanged { prev: WorldArea, curr: WorldArea },
    BlockSelectionRequested,
    BlockDestroyed,
    BlockPlaced { block: Block },
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
                ClientEvent::PlayerPositionChanged { .. } => Some(Self::BlockSelectionRequested),
                ClientEvent::BlockDestroyed => Some(Self::BlockDestroyed),
                ClientEvent::BlockPlaced { block } => Some(Self::BlockPlaced { block: *block }),
            }
        } else {
            None
        }
    }
}
