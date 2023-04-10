use super::{
    block::{Block, BlockArea},
    light::{ChunkAreaLight, ChunkMapLight},
    loader::ChunkLoader,
};
use crate::{
    client::{game::world::BlockVertex, ClientEvent},
    server::{
        event_loop::{Event, EventHandler},
        game::player::{
            ray::{BlockIntersection, Ray},
            Player, WorldArea,
        },
        ServerEvent, ServerSettings,
    },
    utils,
};
use bitvec::prelude::*;
use flume::Sender;
use nalgebra::{point, vector, Point2, Point3, Vector3};
use rayon::prelude::*;
use rustc_hash::{FxHashMap, FxHashSet};
use std::{
    array,
    collections::{hash_map::Entry, LinkedList},
    mem,
    ops::{Deref, Index, IndexMut, Range},
    sync::Arc,
};

#[derive(Default)]
pub struct ChunkMap {
    cells: CellStore,
    loader: ChunkLoader,
    actions: ActionStore,
    light: ChunkMapLight,
    hovered_block: Option<BlockIntersection>,
    reach: Range<f32>,
}

impl ChunkMap {
    pub fn new(settings: &ServerSettings) -> Self {
        Self {
            reach: settings.reach.clone(),
            ..Default::default()
        }
    }

    fn load_many<I>(&mut self, points: I) -> Vec<Point3<i32>>
    where
        I: IntoIterator<Item = Point3<i32>>,
    {
        points
            .into_iter()
            .map(|coords| {
                if let Some(cell) = self.cells.get_mut(coords) {
                    cell.load();
                    Ok(coords)
                } else {
                    Err(coords)
                }
            })
            .collect::<Vec<_>>()
            .into_par_iter()
            .map(|coords| coords.map_err(|coords| self.load_new(coords).map(|cell| (coords, cell))))
            .collect::<LinkedList<_>>()
            .into_iter()
            .filter_map(|entry| match entry {
                Ok(coords) => Some(coords),
                Err(Some((coords, cell))) => {
                    self.cells.insert(coords, cell);
                    Some(coords)
                }
                Err(None) => None,
            })
            .collect()
    }

    fn unload_many<I>(&mut self, points: I) -> Vec<Point3<i32>>
    where
        I: IntoIterator<Item = Point3<i32>>,
    {
        points
            .into_iter()
            .filter(|coords| self.unload(*coords))
            .collect()
    }

    fn apply(
        &mut self,
        coords: Point3<i64>,
        action: BlockAction,
        server_tx: Sender<ServerEvent>,
        ray: Ray,
    ) {
        let chunk_coords = Self::chunk_coords(coords);
        let block_coords = Self::block_coords(coords);

        let (cell, is_created) = if let Some(cell) = self.cells.remove(chunk_coords) {
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
                    &ChunkMapEvent::BlockHoverRequested,
                    (server_tx.clone(), ray),
                );

                let block_updates = {
                    let mut updates = self.light.apply(&self.cells, coords, &action);
                    if !is_loaded && !is_unloaded {
                        updates.insert(coords);
                    } else {
                        updates.remove(&coords);
                    }
                    updates
                };

                self.actions
                    .entry(chunk_coords)
                    .or_default()
                    .insert(block_coords, action);

                let updates = {
                    let mut updates = Self::chunk_updates(block_updates);
                    if is_loaded || is_unloaded {
                        updates.remove(&chunk_coords);
                    }
                    updates
                };

                self.send_unloads(is_unloaded.then_some(chunk_coords), server_tx.clone());
                self.send_loads(is_loaded.then_some(chunk_coords), server_tx.clone(), true);
                self.send_updates(updates, server_tx, true);
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
        self.send_events(
            points
                .into_iter()
                .map(|coords| self.load_event(coords, is_important)),
            server_tx,
        );
    }

    fn par_send_loads<I>(&self, points: I, server_tx: Sender<ServerEvent>, is_important: bool)
    where
        I: IntoParallelIterator<Item = Point3<i32>>,
    {
        self.send_events(
            points
                .into_par_iter()
                .map(|coords| self.load_event(coords, is_important))
                .collect::<LinkedList<_>>(),
            server_tx,
        );
    }

    fn send_unloads<I>(&self, points: I, server_tx: Sender<ServerEvent>)
    where
        I: IntoIterator<Item = Point3<i32>>,
    {
        self.send_events(
            points.into_iter().map(|coords| self.unload_event(coords)),
            server_tx,
        );
    }

    fn send_updates<I: IntoIterator<Item = Point3<i32>>>(
        &self,
        points: I,
        server_tx: Sender<ServerEvent>,
        is_important: bool,
    ) {
        self.send_events(
            points
                .into_iter()
                .filter_map(|coords| self.update_event(coords, is_important)),
            server_tx,
        );
    }

    fn par_send_updates<I: IntoParallelIterator<Item = Point3<i32>>>(
        &self,
        points: I,
        server_tx: Sender<ServerEvent>,
        is_important: bool,
    ) {
        self.send_events(
            points
                .into_par_iter()
                .filter_map(|coords| self.update_event(coords, is_important))
                .collect::<LinkedList<_>>(),
            server_tx,
        );
    }

    fn load_new(&self, coords: Point3<i32>) -> Option<ChunkCell> {
        let mut chunk = self.loader.get(coords);
        self.actions
            .get(coords)
            .into_iter()
            .flatten()
            .for_each(|(coords, action)| {
                chunk.apply(*coords, action);
            });
        ChunkCell::new(chunk)
    }

    fn unload(&mut self, coords: Point3<i32>) -> bool {
        if let Some(cell) = self.cells.remove(coords) {
            if let Some(cell) = cell.unload() {
                self.cells.insert(coords, cell);
                false
            } else {
                true
            }
        } else {
            false
        }
    }

    fn load_event(&self, coords: Point3<i32>, is_important: bool) -> ServerEvent {
        ServerEvent::ChunkLoaded {
            coords,
            data: Arc::new(ChunkData {
                chunk: (*self.cells[coords]).clone(),
                area: self.cells.chunk_area(coords),
                area_light: self.light.chunk_area_light(coords),
            }),
            is_important,
        }
    }

    fn unload_event(&self, coords: Point3<i32>) -> ServerEvent {
        ServerEvent::ChunkUnloaded { coords }
    }

    fn update_event(&self, coords: Point3<i32>, is_important: bool) -> Option<ServerEvent> {
        Some(ServerEvent::ChunkUpdated {
            coords,
            data: Arc::new(ChunkData {
                chunk: (*self.cells.get(coords)?).clone(),
                area: self.cells.chunk_area(coords),
                area_light: self.light.chunk_area_light(coords),
            }),
            is_important,
        })
    }

    fn send_events<I>(&self, events: I, server_tx: Sender<ServerEvent>)
    where
        I: IntoIterator<Item = ServerEvent>,
    {
        for event in events {
            server_tx.send(event).unwrap_or_else(|_| unreachable!());
        }
    }

    fn outline(points: &FxHashSet<Point3<i32>>) -> FxHashSet<Point3<i32>> {
        points
            .iter()
            .flat_map(|coords| ChunkArea::chunk_deltas().map(move |delta| coords + delta.cast()))
            .filter(|coords| !points.contains(coords))
            .collect()
    }

    fn chunk_updates<I>(block_updates: I) -> FxHashSet<Point3<i32>>
    where
        I: IntoIterator<Item = Point3<i64>>,
    {
        block_updates
            .into_iter()
            .flat_map(|coords| BlockArea::deltas().map(move |delta| coords + delta.cast()))
            .map(Self::chunk_coords)
            .collect()
    }

    pub fn chunk_coords(coords: Point3<i64>) -> Point3<i32> {
        coords.map(|c| utils::div_floor(c, Chunk::DIM as i64) as i32)
    }

    pub fn block_coords(coords: Point3<i64>) -> Point3<u8> {
        coords.map(|c| c.rem_euclid(Chunk::DIM as i64) as u8)
    }
}

impl EventHandler<ChunkMapEvent> for ChunkMap {
    type Context<'a> = (Sender<ServerEvent>, Ray);

    fn handle(&mut self, event: &ChunkMapEvent, (server_tx, ray): Self::Context<'_>) {
        match event {
            ChunkMapEvent::InitialRenderRequested { area } => {
                let mut loads = self.load_many(area.points());

                self.handle(
                    &ChunkMapEvent::BlockHoverRequested,
                    (server_tx.clone(), ray),
                );

                loads.par_sort_unstable_by_key(|coords| {
                    utils::magnitude_squared(coords - area.center)
                });

                self.par_send_loads(loads, server_tx, false);
            }
            ChunkMapEvent::WorldAreaChanged { prev, curr } => {
                let unloads = self.unload_many(prev.exclusive_points(curr));
                let loads = self.load_many(curr.exclusive_points(prev));
                let outline = Self::outline(&loads.iter().chain(&unloads).copied().collect());

                self.handle(
                    &ChunkMapEvent::BlockHoverRequested,
                    (server_tx.clone(), ray),
                );

                self.send_unloads(unloads, server_tx.clone());
                self.par_send_loads(loads, server_tx.clone(), false);
                self.par_send_updates(outline, server_tx, false);
            }
            ChunkMapEvent::BlockHoverRequested => {
                let hovered_block =
                    ray.cast(self.reach.clone())
                        .find(|BlockIntersection { coords, .. }| {
                            let chunk_coords = Self::chunk_coords(*coords);
                            let block_coords = Self::block_coords(*coords);
                            self.cells
                                .get(chunk_coords)
                                .map(|cell| cell[block_coords].is_not_air())
                                .unwrap_or_default()
                        });

                if self.hovered_block != hovered_block {
                    self.hovered_block = hovered_block;
                    server_tx
                        .send(ServerEvent::BlockHovered {
                            coords: self.hovered_block.map(|data| data.coords),
                        })
                        .unwrap_or_else(|_| unreachable!());
                }
            }
            ChunkMapEvent::BlockPlaced { block } => {
                if let Some(BlockIntersection { coords, normal }) = self.hovered_block {
                    self.apply(coords + normal, BlockAction::Place(*block), server_tx, ray);
                }
            }
            ChunkMapEvent::BlockDestroyed => {
                if let Some(BlockIntersection { coords, .. }) = self.hovered_block {
                    self.apply(coords, BlockAction::Destroy, server_tx, ray);
                }
            }
        }
    }
}

#[derive(Default)]
pub struct CellStore {
    cells: FxHashMap<Point3<i32>, ChunkCell>,
    y_ranges: FxHashMap<Point2<i32>, Range<i32>>,
}

impl CellStore {
    fn chunk_area(&self, coords: Point3<i32>) -> ChunkArea {
        ChunkArea::from_fn(|delta| {
            let delta = delta.cast().into();
            let chunk_coords = coords + ChunkMap::chunk_coords(delta).coords;
            let block_coords = ChunkMap::block_coords(delta);
            self.get(chunk_coords)
                .map(|cell| cell[block_coords].data().is_opaque())
                .unwrap_or_default()
        })
    }

    fn insert(&mut self, coords: Point3<i32>, cell: ChunkCell) -> Option<ChunkCell> {
        self.y_ranges
            .entry(coords.xz())
            .and_modify(|range| *range = range.start.min(coords.y)..range.end.max(coords.y + 1))
            .or_insert(coords.y..coords.y + 1);
        self.cells.insert(coords, cell)
    }

    fn remove(&mut self, coords: Point3<i32>) -> Option<ChunkCell> {
        let range = self.y_ranges.get_mut(&coords.xz())?;
        if range.contains(&coords.y) {
            if range.len() == 1 {
                self.y_ranges.remove(&coords.xz());
            } else if coords.y == range.start {
                range.start += 1;
            } else if coords.y == range.end - 1 {
                range.end -= 1;
            }
            self.cells.remove(&coords)
        } else {
            None
        }
    }

    pub fn get(&self, coords: Point3<i32>) -> Option<&ChunkCell> {
        self.cells.get(&coords)
    }

    fn get_mut(&mut self, coords: Point3<i32>) -> Option<&mut ChunkCell> {
        self.cells.get_mut(&coords)
    }
}

impl Index<Point3<i32>> for CellStore {
    type Output = ChunkCell;

    fn index(&self, coords: Point3<i32>) -> &Self::Output {
        &self.cells[&coords]
    }
}

#[derive(Default)]
struct ActionStore(FxHashMap<Point3<i32>, FxHashMap<Point3<u8>, BlockAction>>);

impl ActionStore {
    fn entry(
        &mut self,
        coords: Point3<i32>,
    ) -> Entry<Point3<i32>, FxHashMap<Point3<u8>, BlockAction>> {
        self.0.entry(coords)
    }

    fn get(&self, coords: Point3<i32>) -> Option<&FxHashMap<Point3<u8>, BlockAction>> {
        self.0.get(&coords)
    }
}

pub struct ChunkCell {
    chunk: Box<Chunk>,
}

impl ChunkCell {
    fn new(chunk: Chunk) -> Option<Self> {
        chunk.is_not_empty().then(|| Self {
            chunk: Box::new(chunk),
        })
    }

    fn default_with_action(coords: Point3<u8>, action: &BlockAction) -> Result<Option<Self>, ()> {
        let mut chunk = Chunk::default();
        chunk
            .apply(coords, action)
            .then(|| Self::new(chunk))
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
            BlockAction::Place(block) => prev.is_air().then(|| *prev = *block).is_some(),
            BlockAction::Destroy => prev.is_not_air().then(|| *prev = Block::Air).is_some(),
        }
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

#[derive(Default)]
pub struct ChunkArea(BitArr!(for Self::DIM * Self::DIM * Self::DIM, in usize));

impl ChunkArea {
    pub const DIM: usize = Chunk::DIM + Self::PADDING * 2;
    pub const PADDING: usize = BlockArea::PADDING;
    const AXIS_RANGE: Range<i8> = -(Self::PADDING as i8)..(Chunk::DIM + Self::PADDING) as i8;

    fn from_fn<F: FnMut(Vector3<i8>) -> bool>(mut f: F) -> Self {
        let mut value = Self::default();
        for delta in Self::deltas() {
            value.set(delta, f(delta));
        }
        value
    }

    fn block_area(&self, coords: Point3<u8>) -> BlockArea {
        let coords = coords.coords.cast();
        BlockArea::from_fn(|delta| self.is_opaque(coords + delta))
    }

    fn is_opaque(&self, delta: Vector3<i8>) -> bool {
        unsafe { *self.0.get_unchecked(Self::index(delta)) }
    }

    fn set(&mut self, delta: Vector3<i8>, is_opaque: bool) {
        unsafe {
            self.0.set_unchecked(Self::index(delta), is_opaque);
        }
    }

    fn chunk_deltas() -> impl Iterator<Item = Vector3<i32>> {
        let chunk_padding = utils::div_ceil(Self::PADDING, Chunk::DIM) as i32;
        (-chunk_padding..1 + chunk_padding).flat_map(move |dx| {
            (-chunk_padding..1 + chunk_padding).flat_map(move |dy| {
                (-chunk_padding..1 + chunk_padding).map(move |dz| vector![dx, dy, dz])
            })
        })
    }

    fn deltas() -> impl Iterator<Item = Vector3<i8>> {
        Self::AXIS_RANGE.flat_map(|dx| {
            Self::AXIS_RANGE.flat_map(move |dy| Self::AXIS_RANGE.map(move |dz| vector![dx, dy, dz]))
        })
    }

    fn index(delta: Vector3<i8>) -> usize {
        assert!(
            Self::AXIS_RANGE.contains(&delta.x)
                && Self::AXIS_RANGE.contains(&delta.y)
                && Self::AXIS_RANGE.contains(&delta.z)
        );
        unsafe { Self::index_unchecked(delta) }
    }

    unsafe fn index_unchecked(delta: Vector3<i8>) -> usize {
        let delta = delta.map(|c| (c + Self::PADDING as i8) as usize);
        delta.x * Self::DIM.pow(2) + delta.y * Self::DIM + delta.z
    }
}

pub enum BlockAction {
    Place(Block),
    Destroy,
}

pub enum ChunkMapEvent {
    InitialRenderRequested { area: WorldArea },
    WorldAreaChanged { prev: WorldArea, curr: WorldArea },
    BlockHoverRequested,
    BlockPlaced { block: Block },
    BlockDestroyed,
}

impl ChunkMapEvent {
    pub fn new(event: &Event, Player { prev, curr, .. }: &Player) -> Option<Self> {
        if let Event::ClientEvent(event) = event {
            match event {
                ClientEvent::InitialRenderRequested { .. } => {
                    Some(Self::InitialRenderRequested { area: *curr })
                }
                ClientEvent::PlayerPositionChanged { .. } if curr != prev => {
                    Some(Self::WorldAreaChanged {
                        prev: *prev,
                        curr: *curr,
                    })
                }
                ClientEvent::PlayerPositionChanged { .. } => Some(Self::BlockHoverRequested),
                ClientEvent::PlayerOrientationChanged { .. } => Some(Self::BlockHoverRequested),
                ClientEvent::BlockPlaced { block } => Some(Self::BlockPlaced { block: *block }),
                ClientEvent::BlockDestroyed => Some(Self::BlockDestroyed),
            }
        } else {
            None
        }
    }
}
