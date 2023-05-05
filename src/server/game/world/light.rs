use super::{
    block::{data::BlockData, light::BlockLight, Block, SIDE_DELTAS},
    chunk::{
        light::{ChunkAreaLight, ChunkLight},
        ChunkArea,
    },
    {BlockAction, ChunkStore, World},
};
use nalgebra::Point3;
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
        let mut value = ChunkAreaLight::default();
        for (chunk_delta, deltas) in ChunkArea::deltas() {
            let light = self.0.get(&(coords + chunk_delta));
            for (block_coords, delta) in deltas {
                value[delta] = light.map_or_else(Default::default, |light| light[block_coords]);
            }
        }
        value
    }

    pub fn insert_many<I>(&mut self, chunks: &ChunkStore, points: I) -> FxHashSet<Point3<i64>>
    where
        I: IntoIterator<Item = Point3<i32>>,
    {
        Default::default()
    }

    pub fn remove_many<I>(&mut self, chunks: &ChunkStore, points: I) -> FxHashSet<Point3<i64>>
    where
        I: IntoIterator<Item = Point3<i32>>,
    {
        Default::default()
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
        // updates.extend(self.block_skylight(chunks, coords));
        updates.extend(self.place_torchlight(chunks, coords, block));
        updates
    }

    fn destroy(&mut self, chunks: &ChunkStore, coords: Point3<i64>) -> FxHashSet<Point3<i64>> {
        let mut updates = FxHashSet::default();
        // updates.extend(self.unblock_skylight(chunks, coords));
        updates.extend(self.destroy_torchlight(chunks, coords));
        updates
    }

    // fn block_skylight<'a>(
    //     &'a mut self,
    //     chunks: &'a ChunkStore,
    //     coords: Point3<i64>,
    // ) -> impl Iterator<Item = Point3<i64>> + 'a {
    //     BlockLight::SKYLIGHT_RANGE.flat_map(move |i| {
    //         todo!();
    //     })
    // }

    fn place_torchlight<'a>(
        &'a mut self,
        chunks: &'a ChunkStore,
        coords: Point3<i64>,
        block: Block,
    ) -> impl Iterator<Item = Point3<i64>> + 'a {
        let data = block.data();
        BlockLight::TORCHLIGHT_RANGE
            .zip(data.luminance)
            .zip(data.light_filter)
            .flat_map(move |((i, l), f)| {
                let block_light = self.block_light_mut(coords);
                let component = block_light.component(i);
                let value = Self::apply_filter(component, f);
                if value >= l {
                    if block_light.set_component(i, value) {
                        Some(coords)
                            .into_iter()
                            .chain(self.unspread_component(chunks, coords, i, component, f))
                    } else {
                        None.into_iter().chain(FxHashSet::default())
                    }
                } else {
                    block_light
                        .set_component(i, l)
                        .then_some(coords)
                        .into_iter()
                        .chain(self.spread_component(chunks, coords, i, l))
                }
            })
    }

    // fn unblock_skylight<'a>(
    //     &'a mut self,
    //     chunks: &'a ChunkStore,
    //     coords: Point3<i64>,
    // ) -> impl Iterator<Item = Point3<i64>> + 'a {
    //     BlockLight::SKYLIGHT_RANGE.flat_map(move |i| {
    //         let mut updates = self.fill_component(chunks, coords, i);
    //         let value = Self::light_beam_value(chunks, coords + Vector3::y(), i);
    //         updates.extend(self.spread_light_beam(chunks, coords + Vector3::y(), i, value));
    //         updates
    //     })
    // }

    fn destroy_torchlight<'a>(
        &'a mut self,
        chunks: &'a ChunkStore,
        coords: Point3<i64>,
    ) -> impl Iterator<Item = Point3<i64>> + 'a {
        BlockLight::TORCHLIGHT_RANGE.flat_map(move |i| {
            let value = self.brightest_neighbor(coords, i).saturating_sub(1);
            let block_light = self.block_light_mut(coords);
            let component = block_light.component(i);
            match component.cmp(&value) {
                Ordering::Less => {
                    block_light.set_component(i, value);
                    Some(coords)
                        .into_iter()
                        .chain(self.spread_component(chunks, coords, i, value))
                }
                Ordering::Equal => None.into_iter().chain(FxHashSet::default()),
                Ordering::Greater => {
                    block_light.set_component(i, 0);
                    Some(coords)
                        .into_iter()
                        .chain(self.unspread_component(chunks, coords, i, component, 0.0))
                }
            }
        })
    }

    // fn spread_light_beam<'a>(
    //     &'a mut self,
    //     chunks: &'a ChunkStore,
    //     coords: Point3<i64>,
    //     index: usize,
    //     mut value: u8,
    // ) -> impl Iterator<Item = Point3<i64>> + 'a {
    //     Self::floor(coords)
    //         .map_while(
    //             move |coords| match self.set_component(chunks, coords, index, value) {
    //                 Ok(v) => {
    //                     value = v;
    //                     Some(self.spread_component(chunks, coords, index, v))
    //                 }
    //                 Err(0) => None,
    //                 Err(v) => {
    //                     value = v;
    //                     Some(Default::default())
    //                 }
    //             },
    //         )
    //         .flatten()
    // }

    // fn unspread_light_beam<'a>(
    //     &'a mut self,
    //     chunks: &'a ChunkStore,
    //     coords: Point3<i64>,
    //     index: usize,
    //     mut value: u8,
    // ) -> impl Iterator<Item = Point3<i64>> + 'a {
    //     Self::floor(coords)
    //         .map_while(
    //             move |coords| match self.unset_component(chunks, coords, index, value) {
    //                 Ok((v, _)) => {
    //                     value = v;
    //                     Some(self.unspread_component(chunks, coords, index, v))
    //                 }
    //                 Err((_, 0)) => None,
    //                 Err((_, v)) => {
    //                     value = v;
    //                     Some(Default::default())
    //                 }
    //             },
    //         )
    //         .flatten()
    // }

    fn spread_component(
        &mut self,
        chunks: &ChunkStore,
        coords: Point3<i64>,
        index: usize,
        value: u8,
    ) -> FxHashSet<Point3<i64>> {
        let mut deq = VecDeque::from([(coords, value)]);
        let mut updates = FxHashSet::default();

        while let Some((coords, value)) = deq.pop_front() {
            for coords in Self::neighbors(coords) {
                if let Some(value) = self.set_component(chunks, coords, index, value - 1) {
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
        expected: u8,
        filter: f32,
    ) -> FxHashSet<Point3<i64>> {
        let mut deq = VecDeque::from([(coords, expected)]);
        let mut updates = FxHashSet::default();
        let mut sources = FxHashSet::default();

        while let Some((coords, expected)) = deq.pop_front() {
            for coords in Self::neighbors(coords) {
                match self.unset_component(chunks, coords, index, expected - 1, filter) {
                    Ok(None) | Err(Ok(0) | Err(Ok(0) | Err(0))) => {}
                    Ok(Some(0)) => unreachable!(),
                    Ok(Some(expected)) => {
                        deq.push_back((coords, expected));
                        updates.insert(coords);
                    }
                    Err(Ok(component)) => {
                        sources.insert((coords, component));
                    }
                    Err(Err(Ok(luminance))) => {
                        updates.insert(coords);
                        sources.insert((coords, luminance));
                    }
                    Err(Err(Err(luminance))) => {
                        sources.insert((coords, luminance));
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

    fn component(&self, coords: Point3<i64>, index: usize) -> u8 {
        self.block_light(coords).component(index)
    }

    fn set_component(
        &mut self,
        chunks: &ChunkStore,
        coords: Point3<i64>,
        index: usize,
        value: u8,
    ) -> Option<u8> {
        let block_light = self.block_light_mut(coords);
        let component = block_light.component(index);
        let data = chunks.block(coords).data();
        let value = Self::apply_filter(value, Self::filter(data, index));
        (component < value).then(|| {
            block_light.set_component(index, value);
            value
        })
    }

    fn unset_component(
        &mut self,
        chunks: &ChunkStore,
        coords: Point3<i64>,
        index: usize,
        expected: u8,
        filter: f32,
    ) -> Result<Option<u8>, Result<u8, Result<u8, u8>>> {
        let block_light = self.block_light_mut(coords);
        let component = block_light.component(index);
        let data = chunks.block(coords).data();
        let expected = Self::apply_filter(expected, Self::filter(data, index));
        if component == expected {
            let value = Self::apply_filter(component, filter);
            let luminance = Self::luminance(data, index);
            if value >= luminance {
                Ok(block_light.set_component(index, value).then_some(expected))
            } else if block_light.set_component(index, luminance) {
                Err(Err(Ok(luminance)))
            } else {
                Err(Err(Err(luminance)))
            }
        } else {
            Err(Ok(component))
        }
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

    fn block_light_mut(&mut self, coords: Point3<i64>) -> BlockLightRefMut {
        BlockLightRefMut::new(
            self.0.entry(World::chunk_coords(coords)),
            World::block_coords(coords),
        )
    }

    // fn light_beam_value(chunks: &ChunkStore, coords: Point3<i64>, index: usize) -> u8 {
    //     let top = World::coords(point![ChunkGenerator::Y_RANGE.end], Default::default()).x;
    //     (coords.y..top)
    //         .rev()
    //         .map(|y| chunks.block(point![coords.x, y, coords.z]))
    //         .skip_while(|block| *block == Block::Air)
    //         .map(|block| Self::filter(block.data(), index))
    //         .try_fold(BlockLight::COMPONENT_MAX, |accum, f| {
    //             Some(NonZeroU8::new(Self::apply_filter(accum, f))?.get())
    //         })
    //         .unwrap_or(0)
    // }

    // fn floor(coords: Point3<i64>) -> impl Iterator<Item = Point3<i64>> {
    //     let bottom = World::coords(point![ChunkGenerator::Y_RANGE.start], Default::default()).x;
    //     (bottom..coords.y)
    //         .rev()
    //         .map(move |y| point![coords.x, y, coords.z])
    // }

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

    fn component(&self, index: usize) -> u8 {
        self.get().component(index)
    }

    fn set_component(self, index: usize, value: u8) -> bool {
        if self.component(index) != value {
            self.into_mut().set_component(index, value);
            true
        } else {
            false
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
