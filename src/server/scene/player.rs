use super::world::chunk::{Chunk, ChunkMap};
use crate::{
    client::ClientEvent,
    server::event_loop::{Event, EventHandler},
};
use nalgebra::{vector, Point3};
use rayon::prelude::*;
use std::ops::RangeInclusive;

#[derive(Default)]
pub struct Player {
    pub prev: WorldArea,
    pub curr: WorldArea,
}

impl Player {
    pub fn chunk_coords(coords: Point3<f32>) -> Point3<i32> {
        (coords / Chunk::DIM as f32).map(|c| c.floor() as i32)
    }
}

impl EventHandler<Event> for Player {
    type Context<'a> = ();

    fn handle(&mut self, event: &Event, _: Self::Context<'_>) {
        self.prev = self.curr;

        if let Event::ClientEvent(event) = event {
            match event {
                ClientEvent::InitialRenderRequested {
                    player_coords,
                    render_distance,
                } => {
                    self.curr = WorldArea {
                        center: Self::chunk_coords(*player_coords),
                        radius: *render_distance,
                    };
                }
                ClientEvent::PlayerPositionChanged { coords } => {
                    self.curr.center = Self::chunk_coords(*coords);
                }
            }
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Default)]
pub struct WorldArea {
    pub center: Point3<i32>,
    pub radius: u32,
}

impl WorldArea {
    pub fn points(&self) -> impl Iterator<Item = Point3<i32>> + '_ {
        let radius = self.radius as i32;
        (-radius..=radius).flat_map(move |dx| {
            self.y_range().flat_map(move |dy| {
                (-radius..=radius).filter_map(move |dz| {
                    let dist = dx.pow(2) + dy.pow(2) + dz.pow(2);
                    (dist <= radius.pow(2)).then_some(self.center + vector![dx, dy, dz])
                })
            })
        })
    }

    pub fn par_points(&self) -> impl ParallelIterator<Item = Point3<i32>> + '_ {
        let radius = self.radius as i32;
        (-radius..=radius).into_par_iter().flat_map(move |dx| {
            self.y_range().into_par_iter().flat_map(move |dy| {
                (-radius..=radius).into_par_iter().filter_map(move |dz| {
                    let dist = dx.pow(2) + dy.pow(2) + dz.pow(2);
                    (dist <= radius.pow(2)).then_some(self.center + vector![dx, dy, dz])
                })
            })
        })
    }

    pub fn exclusive_points<'a>(
        &'a self,
        other: &'a WorldArea,
    ) -> impl Iterator<Item = Point3<i32>> + 'a {
        let radius = other.radius as i32;
        self.points().filter_map(move |point| {
            let dist = (point - other.center).map(|c| c.pow(2)).sum();
            (dist > radius.pow(2)).then_some(point)
        })
    }

    pub fn par_exclusive_points<'a>(
        &'a self,
        other: &'a WorldArea,
    ) -> impl ParallelIterator<Item = Point3<i32>> + 'a {
        let radius = other.radius as i32;
        self.par_points().filter_map(move |point| {
            let dist = (point - other.center).map(|c| c.pow(2)).sum();
            (dist > radius.pow(2)).then_some(point)
        })
    }

    fn y_range(&self) -> RangeInclusive<i32> {
        let radius = self.radius as i32;
        (-radius).max(ChunkMap::LOWER_LIMIT - self.center.y)
            ..=radius.min(ChunkMap::UPPER_LIMIT - self.center.y)
    }
}
