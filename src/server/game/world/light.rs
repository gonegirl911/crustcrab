use super::{
    block::{data::BlockData, light::BlockLight, Block, SIDE_DELTAS},
    chunk::light::{ChunkAreaLight, ChunkLight},
    {BlockAction, ChunkStore, World},
};
use nalgebra::{vector, Point3};
use rustc_hash::{FxHashMap, FxHashSet};
use std::{
    cmp::Ordering,
    collections::{
        hash_map::{Entry, VacantEntry},
        VecDeque,
    },
};

#[derive(Default)]
pub struct ChunkMapLight(FxHashMap<Point3<i32>, ChunkLight>);

impl ChunkMapLight {
    pub fn chunk_area_light(&self, coords: Point3<i32>) -> ChunkAreaLight {
        let coords = World::coords(coords, Default::default());
        ChunkAreaLight::from_fn(|delta| self.block_light(coords + delta.cast()))
    }

    pub fn apply(
        &mut self,
        chunks: &ChunkStore,
        coords: Point3<i64>,
        action: &BlockAction,
    ) -> FxHashSet<Point3<i64>> {
        match action {
            BlockAction::Place(block) => self.place(chunks, coords, *block),
            BlockAction::Destroy => self.destroy(chunks, coords),
        }
    }

    fn place(
        &mut self,
        chunks: &ChunkStore,
        coords: Point3<i64>,
        block: Block,
    ) -> FxHashSet<Point3<i64>> {
        let mut updates = FxHashSet::default();
        updates.extend(self.block_skylight(chunks, coords));
        updates.extend(self.place_torchlight(chunks, coords, block));
        updates
    }

    fn destroy(&mut self, chunks: &ChunkStore, coords: Point3<i64>) -> FxHashSet<Point3<i64>> {
        let mut updates = FxHashSet::default();
        updates.extend(self.unblock_skylight(chunks, coords));
        updates.extend(self.destroy_torchlight(chunks, coords));
        updates
    }

    fn block_skylight<'a>(
        &'a mut self,
        chunks: &'a ChunkStore,
        coords: Point3<i64>,
    ) -> impl Iterator<Item = Point3<i64>> + 'a {
        BlockLight::SKYLIGHT_RANGE.flat_map(move |i| {
            if Self::is_covered(chunks, coords) {
                let component = self.take_component(coords, i);
                if component != 0 {
                    self.unspread_component(chunks, coords, i, component)
                } else {
                    Default::default()
                }
            } else {
                todo!()
            }
        })
    }

    fn place_torchlight<'a>(
        &'a mut self,
        chunks: &'a ChunkStore,
        coords: Point3<i64>,
        block: Block,
    ) -> impl Iterator<Item = Point3<i64>> + 'a {
        BlockLight::TORCHLIGHT_RANGE
            .zip(block.data().luminance)
            .flat_map(move |(i, v)| {
                let component = self.replace_component(coords, i, v);
                match component.cmp(&v) {
                    Ordering::Less => self.spread_component(chunks, coords, i, v),
                    Ordering::Equal => Default::default(),
                    Ordering::Greater => self.unspread_component(chunks, coords, i, component),
                }
            })
    }

    fn unblock_skylight<'a>(
        &'a mut self,
        chunks: &'a ChunkStore,
        coords: Point3<i64>,
    ) -> impl Iterator<Item = Point3<i64>> + 'a {
        BlockLight::SKYLIGHT_RANGE.flat_map(move |i| {
            if Self::is_covered(chunks, coords) {
                self.fill_component(chunks, coords, i)
            } else {
                self.spread_light_beam(chunks, coords, i).collect()
            }
        })
    }

    fn destroy_torchlight<'a>(
        &'a mut self,
        chunks: &'a ChunkStore,
        coords: Point3<i64>,
    ) -> impl Iterator<Item = Point3<i64>> + 'a {
        BlockLight::TORCHLIGHT_RANGE.flat_map(move |i| {
            let component = self.take_component(coords, i);
            if component != 0 {
                self.unspread_component(chunks, coords, i, component)
            } else {
                self.fill_component(chunks, coords, i)
            }
        })
    }

    fn spread_light_beam<'a>(
        &'a mut self,
        chunks: &'a ChunkStore,
        coords: Point3<i64>,
        index: usize,
    ) -> impl Iterator<Item = Point3<i64>> + 'a {
        let mut value = self.component(coords + vector![0, 1, 0], index);
        Self::floor(chunks, coords)
            .map_while(
                move |coords| match self.set_component(chunks, coords, index, value) {
                    Ok(v) => {
                        value = v;
                        Some(self.spread_component(chunks, coords, index, v))
                    }
                    Err(0) => None,
                    Err(v) => {
                        value = v;
                        Some(Default::default())
                    }
                },
            )
            .flatten()
    }

    fn spread_component(
        &mut self,
        chunks: &ChunkStore,
        coords: Point3<i64>,
        index: usize,
        value: u8,
    ) -> FxHashSet<Point3<i64>> {
        let mut deq = VecDeque::from([(coords, value)]);
        let mut updates = FxHashSet::from_iter([coords]);

        while let Some((coords, value)) = deq.pop_front() {
            for coords in Self::neighbors(coords) {
                if let Ok(value) = self.set_component(chunks, coords, index, value - 1) {
                    deq.push_back((coords, value));
                    updates.insert(coords);
                }
            }
        }

        updates
    }

    fn unspread_component(
        &mut self,
        chunks: &ChunkStore,
        coords: Point3<i64>,
        index: usize,
        value: u8,
    ) -> FxHashSet<Point3<i64>> {
        let mut deq = VecDeque::from([(coords, value)]);
        let mut updates = FxHashSet::from_iter([coords]);
        let mut sources = FxHashSet::default();

        while let Some((coords, value)) = deq.pop_front() {
            for coords in Self::neighbors(coords) {
                match self.unset_component(chunks, coords, index, value - 1) {
                    Ok((value, 0)) => {
                        deq.push_back((coords, value));
                        updates.insert(coords);
                    }
                    Ok((value, luminance)) => {
                        deq.push_back((coords, value));
                        updates.insert(coords);
                        sources.insert((coords, luminance));
                    }
                    Err(0) => {}
                    Err(component) => {
                        sources.insert((coords, component));
                    }
                }
            }
        }

        sources
            .into_iter()
            .flat_map(|(coords, value)| self.spread_component(chunks, coords, index, value))
            .chain(updates)
            .collect()
    }

    fn fill_component(
        &mut self,
        chunks: &ChunkStore,
        coords: Point3<i64>,
        index: usize,
    ) -> FxHashSet<Point3<i64>> {
        let component = self.brightest_neighbor(coords, index);
        if component > 1 {
            let value = component - 1;
            self.overwrite_component(coords, index, value);
            self.spread_component(chunks, coords, index, value)
        } else {
            Default::default()
        }
    }

    fn component(&self, coords: Point3<i64>, index: usize) -> u8 {
        self.block_light(coords).component(index)
    }

    fn set_component(
        &mut self,
        chunks: &ChunkStore,
        coords: Point3<i64>,
        index: usize,
        value: u8,
    ) -> Result<u8, u8> {
        let block_light = self.block_light_mut(coords);
        let component = block_light.get().component(index);
        let data = chunks.block(coords).data();
        let value = Self::apply_filter(value, Self::filter(data, index));
        if component < value {
            block_light.into_mut().set_component(index, value);
            Ok(value)
        } else {
            Err(value)
        }
    }

    fn unset_component(
        &mut self,
        chunks: &ChunkStore,
        coords: Point3<i64>,
        index: usize,
        value: u8,
    ) -> Result<(u8, u8), u8> {
        let block_light = self.block_light_mut(coords);
        let component = block_light.get().component(index);
        let data = chunks.block(coords).data();
        if component != 0 && component == Self::apply_filter(value, Self::filter(data, index)) {
            let luminance = Self::luminance(data, index);
            block_light.into_mut().set_component(index, luminance);
            Ok((component, luminance))
        } else {
            Err(component)
        }
    }

    fn overwrite_component(&mut self, coords: Point3<i64>, index: usize, value: u8) {
        self.block_light_mut(coords)
            .into_mut()
            .set_component(index, value)
    }

    fn replace_component(&mut self, coords: Point3<i64>, index: usize, value: u8) -> u8 {
        let block_light = self.block_light_mut(coords);
        let component = block_light.get().component(index);
        if component != value {
            block_light.into_mut().set_component(index, value);
        }
        component
    }

    fn take_component(&mut self, coords: Point3<i64>, index: usize) -> u8 {
        self.replace_component(coords, index, 0)
    }

    fn brightest_neighbor(&self, coords: Point3<i64>, index: usize) -> u8 {
        Self::neighbors(coords)
            .map(|coords| self.component(coords, index))
            .max()
            .unwrap_or_else(|| unreachable!())
    }

    fn block_light(&self, coords: Point3<i64>) -> BlockLight {
        self.0
            .get(&World::chunk_coords(coords))
            .map_or_else(Default::default, |light| light[World::block_coords(coords)])
    }

    fn block_light_mut(&mut self, coords: Point3<i64>) -> BlockLightRefMut<'_> {
        BlockLightRefMut::new(
            self.0.entry(World::chunk_coords(coords)),
            World::block_coords(coords),
        )
    }

    fn is_covered(chunks: &ChunkStore, coords: Point3<i64>) -> bool {
        Self::ceiling(chunks, coords)
            .map(|coords| chunks.block(coords))
            .any(|block| block.data().is_opaque())
    }

    fn floor(chunks: &ChunkStore, coords: Point3<i64>) -> impl Iterator<Item = Point3<i64>> + '_ {
        let y_range = chunks.y_range(World::chunk_coords(coords).xz());
        (0..)
            .map(move |dy| coords - vector![0, dy, 0])
            .take_while(move |coords| y_range.contains(&World::chunk_coords(*coords).y))
    }

    fn ceiling(chunks: &ChunkStore, coords: Point3<i64>) -> impl Iterator<Item = Point3<i64>> + '_ {
        let y_range = chunks.y_range(World::chunk_coords(coords).xz());
        (1..)
            .map(move |dy| coords + vector![0, dy, 0])
            .take_while(move |coords| y_range.contains(&World::chunk_coords(*coords).y))
    }

    fn neighbors(coords: Point3<i64>) -> impl Iterator<Item = Point3<i64>> {
        SIDE_DELTAS
            .into_values()
            .map(move |delta| coords + delta.cast())
    }

    fn apply_filter(value: u8, filter: f32) -> u8 {
        (value as f32 * filter).round() as u8
    }

    fn luminance(data: &BlockData, index: usize) -> u8 {
        if BlockLight::TORCHLIGHT_RANGE.contains(&index) {
            data.luminance[index - BlockLight::TORCHLIGHT_RANGE.start]
        } else {
            0
        }
    }

    fn filter(data: &BlockData, index: usize) -> f32 {
        data.light_filter[index % 3]
    }
}

enum BlockLightRefMut<'a> {
    Occupied(&'a mut BlockLight),
    Vacant {
        entry: VacantEntry<'a, Point3<i32>, ChunkLight>,
        coords: Point3<u8>,
    },
}

impl<'a> BlockLightRefMut<'a> {
    fn new(entry: Entry<'a, Point3<i32>, ChunkLight>, coords: Point3<u8>) -> Self {
        match entry {
            Entry::Occupied(entry) => Self::Occupied(&mut entry.into_mut()[coords]),
            Entry::Vacant(entry) => Self::Vacant { entry, coords },
        }
    }

    fn get(&self) -> BlockLight {
        if let Self::Occupied(block_light) = self {
            **block_light
        } else {
            Default::default()
        }
    }

    fn into_mut(self) -> &'a mut BlockLight {
        match self {
            Self::Occupied(block_light) => block_light,
            Self::Vacant { entry, coords } => &mut entry.insert(Default::default())[coords],
        }
    }
}
