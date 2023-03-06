use super::{
    block::{Block, BlockArea, NEIGHBOR_DELTAS},
    light::{ChunkAreaLight, ChunkMapLight},
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
use nalgebra::{point, Point, Point3, Scalar};
use rayon::prelude::*;
use rustc_hash::{FxHashMap, FxHashSet};
use std::{
    array,
    collections::LinkedList,
    mem,
    ops::{Deref, Index, IndexMut, Mul, Range},
    sync::Arc,
};

#[derive(Default)]
pub struct ChunkMap {
    cells: FxHashMap<Point3<i32>, ChunkCell>,
    actions: FxHashMap<Point3<i32>, FxHashMap<Point3<u8>, BlockAction>>,
    hovered_block: Option<BlockIntersection>,
    loader: ChunkLoader,
    light: ChunkMapLight,
}

impl ChunkMap {
    fn load_many(&mut self, points: &[Point3<i32>]) -> Vec<Point3<i32>> {
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
            .iter()
            .copied()
            .filter(|coords| self.cells.contains_key(coords))
            .collect()
    }

    fn unload_many<'a, I>(&'a mut self, points: I) -> Vec<Point3<i32>>
    where
        I: IntoIterator<Item = Point3<i32>> + 'a,
    {
        points
            .into_iter()
            .filter(|coords| {
                self.cells
                    .remove_entry(coords)
                    .map(|(coords, cell)| {
                        if let Some(cell) = cell.unload() {
                            self.cells.insert(coords, cell);
                        }
                    })
                    .is_some()
            })
            .collect()
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

                let light_updates = self.light.apply(&self.cells, coords, &action);

                self.actions
                    .entry(chunk_coords)
                    .or_default()
                    .insert(block_coords, action);

                self.send_updates(
                    NEIGHBOR_DELTAS
                        .into_iter()
                        .map(|delta| Player::chunk_coords(coords + delta.coords.cast()))
                        .chain((!is_loaded && !is_unloaded).then_some(chunk_coords))
                        .chain(light_updates)
                        .collect::<FxHashSet<_>>(),
                    server_tx.clone(),
                    true,
                );

                if is_loaded {
                    self.send_loads([chunk_coords], server_tx, true);
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

    fn send_loads<I>(&self, points: I, server_tx: Sender<ServerEvent>, is_important: bool)
    where
        I: IntoIterator<Item = Point3<i32>>,
    {
        points
            .into_iter()
            .map(|coords| ServerEvent::ChunkLoaded {
                coords,
                data: Arc::new(ChunkData {
                    chunk: (*self.cells[&coords]).clone(),
                    area: self.chunk_area(coords),
                    area_light: self.light.chunk_area_light(coords),
                }),
                is_important,
            })
            .for_each(|event| {
                server_tx.send(event).unwrap_or_else(|_| unreachable!());
            });
    }

    fn par_send_loads<I>(&self, points: I, server_tx: Sender<ServerEvent>, is_important: bool)
    where
        I: IntoParallelIterator<Item = Point3<i32>>,
    {
        points
            .into_par_iter()
            .map(|coords| ServerEvent::ChunkLoaded {
                coords,
                data: Arc::new(ChunkData {
                    chunk: (*self.cells[&coords]).clone(),
                    area: self.chunk_area(coords),
                    area_light: self.light.chunk_area_light(coords),
                }),
                is_important,
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
        is_important: bool,
    ) {
        points
            .into_iter()
            .filter_map(|coords| {
                Some(ServerEvent::ChunkUpdated {
                    coords,
                    data: Arc::new(ChunkData {
                        chunk: (*self.cells.get(&coords)?).clone(),
                        area: self.chunk_area(coords),
                        area_light: self.light.chunk_area_light(coords),
                    }),
                    is_important,
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
        is_important: bool,
    ) {
        points
            .into_par_iter()
            .filter_map(|coords| {
                Some(ServerEvent::ChunkUpdated {
                    coords,
                    data: Arc::new(ChunkData {
                        chunk: (*self.cells.get(&coords)?).clone(),
                        area: self.chunk_area(coords),
                        area_light: self.light.chunk_area_light(coords),
                    }),
                    is_important,
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

    fn chunk_area(&self, coords: Point3<i32>) -> ChunkArea {
        ChunkArea::new(&self.cells, coords)
    }

    fn outline(points: &FxHashSet<Point3<i32>>) -> FxHashSet<Point3<i32>> {
        points
            .iter()
            .flat_map(|coords| {
                NEIGHBOR_DELTAS
                    .into_iter()
                    .map(move |delta| coords + delta.coords.cast())
            })
            .filter(|coords| !points.contains(coords))
            .collect()
    }
}

impl EventHandler<ChunkMapEvent> for ChunkMap {
    type Context<'a> = (Sender<ServerEvent>, Ray);

    fn handle(&mut self, event: &ChunkMapEvent, (server_tx, ray): Self::Context<'_>) {
        match event {
            ChunkMapEvent::InitialRenderRequested { area } => {
                let mut loads = self.load_many(&area.points().collect::<Vec<_>>());

                self.handle(
                    &ChunkMapEvent::BlockSelectionRequested,
                    (server_tx.clone(), ray),
                );

                loads.par_sort_unstable_by_key(|coords| {
                    (coords - area.center).map(|c| c.pow(2)).sum()
                });

                self.par_send_loads(loads, server_tx, false);
            }
            ChunkMapEvent::WorldAreaChanged { prev, curr } => {
                let unloads = self.unload_many(prev.exclusive_points(curr));
                let loads = self.load_many(&curr.exclusive_points(prev).collect::<Vec<_>>());
                let updates = Self::outline(&loads.iter().chain(&unloads).copied().collect());

                self.handle(
                    &ChunkMapEvent::BlockSelectionRequested,
                    (server_tx.clone(), ray),
                );

                self.par_send_updates(updates, server_tx.clone(), false);
                self.send_unloads(unloads, server_tx.clone());
                self.par_send_loads(loads, server_tx, false);
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

pub struct ChunkCell {
    chunk: Box<Chunk>,
}

impl ChunkCell {
    fn load_new(chunk: Chunk) -> Option<Self> {
        chunk.is_not_empty().then(|| Self {
            chunk: Box::new(chunk),
        })
    }

    fn default_with_action(coords: Point3<u8>, action: &BlockAction) -> Result<Option<Self>, ()> {
        let mut chunk = Chunk::default();
        chunk
            .apply(coords, action)
            .then(|| Self::load_new(chunk))
            .ok_or(())
    }

    fn load(&mut self) {}

    fn unload(self) -> Option<Self> {
        None
    }

    fn apply(mut self, coords: Point3<u8>, action: &BlockAction) -> Result<Option<Self>, Self> {
        if self.chunk.apply(coords, action) {
            Ok(self.chunk.is_not_empty().then_some(self))
        } else {
            Err(self)
        }
    }
}

impl Deref for ChunkCell {
    type Target = Chunk;

    fn deref(&self) -> &Self::Target {
        &self.chunk
    }
}

pub struct ChunkData {
    chunk: Chunk,
    area: ChunkArea,
    area_light: ChunkAreaLight,
}

impl ChunkData {
    pub fn vertices(&self) -> impl Iterator<Item = BlockVertex> + '_ {
        self.chunk.vertices(&self.area, &self.area_light)
    }
}

#[repr(align(16))]
#[derive(Clone, Default)]
pub struct Chunk([[[Block; Self::DIM]; Self::DIM]; Self::DIM]);

impl Chunk {
    pub const DIM: usize = 16;

    pub fn from_fn<F: FnMut(Point3<u8>) -> Block>(mut f: F) -> Self {
        Self(array::from_fn(|x| {
            array::from_fn(|y| array::from_fn(|z| f(point![x as u8, y as u8, z as u8])))
        }))
    }

    fn vertices<'a>(
        &'a self,
        area: &'a ChunkArea,
        area_light: &'a ChunkAreaLight,
    ) -> impl Iterator<Item = BlockVertex> + 'a {
        self.blocks().flat_map(|(coords, block)| {
            block
                .vertices(
                    coords,
                    area.block_area(coords),
                    area_light.block_area_light(coords),
                )
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
            .all(|blocks| *unsafe { mem::transmute::<_, &u128>(blocks) } == 0)
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

pub struct ChunkArea(BitArr!(for Self::DIM * Self::DIM * Self::DIM, in usize));

impl ChunkArea {
    pub const DIM: usize = Chunk::DIM + Self::PADDING * 2;
    pub const PADDING: usize = BlockArea::PADDING;
    pub const NEG_PADDING_RANGE: Range<i8> = -(Self::PADDING as i8)..0;
    pub const POS_PADDING_RANGE: Range<i8> = Chunk::DIM as i8..(Chunk::DIM + Self::PADDING) as i8;
    const AXIS_RANGE: Range<i8> = -(Self::PADDING as i8)..(Chunk::DIM + Self::PADDING) as i8;

    fn new(cells: &FxHashMap<Point3<i32>, ChunkCell>, coords: Point3<i32>) -> Self {
        let mut value = Self(Default::default());

        for (delta, block) in cells[&coords].blocks() {
            value.set(delta.cast(), block.data().is_opaque());
        }

        for perm in [
            Permutation([0, 1, 2]),
            Permutation([1, 0, 2]),
            Permutation([1, 2, 0]),
        ] {
            for x in Self::NEG_PADDING_RANGE.chain(Self::POS_PADDING_RANGE) {
                let delta = perm * point![x, 0, 0];
                let chunk_coords = coords + Player::chunk_coords(delta.cast()).coords;
                let block_coords = Player::block_coords(delta.cast());
                let Some(cell) = cells.get(&chunk_coords) else { continue };
                for y in 0..Chunk::DIM as u8 {
                    for z in 0..Chunk::DIM as u8 {
                        let rest = perm * point![0, y, z];
                        let delta = delta + rest.coords.cast();
                        let block_coords = block_coords + rest.coords;
                        value.set(delta, cell[block_coords].data().is_opaque());
                    }
                }
            }
        }

        for perm in [
            Permutation([0, 1, 2]),
            Permutation([0, 2, 1]),
            Permutation([2, 0, 1]),
        ] {
            for x in Self::NEG_PADDING_RANGE.chain(Self::POS_PADDING_RANGE) {
                for y in Self::NEG_PADDING_RANGE.chain(Self::POS_PADDING_RANGE) {
                    let delta = perm * point![x, y, 0];
                    let chunk_coords = coords + Player::chunk_coords(delta.cast()).coords;
                    let block_coords = Player::block_coords(delta.cast());
                    let Some(cell) = cells.get(&chunk_coords) else { continue };
                    for z in 0..Chunk::DIM as u8 {
                        let rest = perm * point![0, 0, z];
                        let delta = delta + rest.coords.cast();
                        let block_coords = block_coords + rest.coords;
                        value.set(delta, cell[block_coords].data().is_opaque());
                    }
                }
            }
        }

        for x in Self::NEG_PADDING_RANGE.chain(Self::POS_PADDING_RANGE) {
            for y in Self::NEG_PADDING_RANGE.chain(Self::POS_PADDING_RANGE) {
                for z in Self::NEG_PADDING_RANGE.chain(Self::POS_PADDING_RANGE) {
                    let delta = point![x, y, z];
                    let chunk_coords = coords + Player::chunk_coords(delta.cast()).coords;
                    let block_coords = Player::block_coords(delta.cast());
                    let Some(cell) = cells.get(&chunk_coords) else { continue };
                    value.set(delta, cell[block_coords].data().is_opaque());
                }
            }
        }

        value
    }

    fn block_area(&self, coords: Point3<u8>) -> BlockArea {
        let coords = coords.cast();
        BlockArea::from_fn(|delta| self.is_opaque(coords + delta.coords))
    }

    fn is_opaque(&self, coords: Point3<i8>) -> bool {
        unsafe { *self.0.get_unchecked(Self::index(coords)) }
    }

    fn set(&mut self, coords: Point3<i8>, is_opaque: bool) {
        unsafe {
            self.0.set_unchecked(Self::index(coords), is_opaque);
        }
    }

    fn index(coords: Point3<i8>) -> usize {
        assert!(
            Self::AXIS_RANGE.contains(&coords.x)
                && Self::AXIS_RANGE.contains(&coords.y)
                && Self::AXIS_RANGE.contains(&coords.z)
        );
        unsafe { Self::index_unchecked(coords) }
    }

    unsafe fn index_unchecked(coords: Point3<i8>) -> usize {
        let coords = coords.map(|c| (c + Self::PADDING as i8) as usize);
        coords.x * Self::DIM.pow(2) + coords.y * Self::DIM + coords.z
    }
}

#[derive(Clone, Copy)]
pub struct Permutation<const D: usize>(pub [usize; D]);

impl<T: Scalar, const D: usize> Mul<Point<T, D>> for Permutation<D> {
    type Output = Point<T, D>;

    fn mul(self, rhs: Point<T, D>) -> Self::Output {
        self.0.map(|i| rhs[i].clone()).into()
    }
}

pub enum BlockAction {
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
