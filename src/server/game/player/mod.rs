pub mod ray;

use self::ray::Ray;
use super::world::World;
use crate::{
    client::ClientEvent,
    server::event_loop::{Event, EventHandler},
    shared::utils,
};
use nalgebra::{vector, Point3};
use rayon::prelude::*;
use serde::Deserialize;
use std::ops::Range;

#[derive(Default)]
pub struct Player {
    pub prev: WorldArea,
    pub curr: WorldArea,
    pub ray: Ray,
}

impl EventHandler<Event> for Player {
    type Context<'a> = ();

    fn handle(&mut self, event: &Event, _: Self::Context<'_>) {
        self.prev = self.curr;

        if let Event::ClientEvent(event) = event {
            match event {
                ClientEvent::InitialRenderRequested {
                    origin,
                    dir,
                    render_distance,
                } => {
                    self.curr = WorldArea {
                        center: utils::chunk_coords(*origin),
                        radius: *render_distance,
                    };
                    self.ray = Ray {
                        origin: *origin,
                        dir: *dir,
                    };
                }
                ClientEvent::PlayerOrientationChanged { dir } => {
                    self.ray.dir = *dir;
                }
                ClientEvent::PlayerPositionChanged { origin } => {
                    self.curr.center = utils::chunk_coords(*origin);
                    self.ray.origin = *origin;
                }
                _ => {}
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
        self.cube_points().filter(|point| {
            utils::magnitude_squared(point.xz() - self.center.xz()) <= self.radius.pow(2)
        })
    }

    pub fn par_points(&self) -> impl ParallelIterator<Item = Point3<i32>> + '_ {
        self.par_cube_points().filter(|point| {
            utils::magnitude_squared(point.xz() - self.center.xz()) <= self.radius.pow(2)
        })
    }

    pub fn exclusive_points<'a>(
        &'a self,
        other: &'a WorldArea,
    ) -> impl Iterator<Item = Point3<i32>> + 'a {
        self.points().filter(|point| {
            utils::magnitude_squared(point.xz() - other.center.xz()) > other.radius.pow(2)
                || (point.y - other.center.y).unsigned_abs() > other.radius
        })
    }

    pub fn par_exclusive_points<'a>(
        &'a self,
        other: &'a WorldArea,
    ) -> impl ParallelIterator<Item = Point3<i32>> + 'a {
        self.par_points().filter(|point| {
            utils::magnitude_squared(point.xz() - other.center.xz()) > other.radius.pow(2)
                || (point.y - other.center.y).unsigned_abs() > other.radius
        })
    }

    fn cube_points(&self) -> impl Iterator<Item = Point3<i32>> + '_ {
        let radius = self.radius as i32;
        (-radius..=radius).flat_map(move |dx| {
            (-radius..=radius).flat_map(move |dy| {
                (-radius..=radius)
                    .map(move |dz| self.center + vector![dx, dy, dz])
                    .filter(|point| World::Y_RANGE.contains(&point.y))
            })
        })
    }

    fn par_cube_points(&self) -> impl ParallelIterator<Item = Point3<i32>> + '_ {
        let radius = self.radius as i32;
        (-radius..=radius).into_par_iter().flat_map(move |dx| {
            (-radius..=radius).into_par_iter().flat_map(move |dy| {
                (-radius..=radius)
                    .into_par_iter()
                    .map(move |dz| self.center + vector![dx, dy, dz])
                    .filter(|point| World::Y_RANGE.contains(&point.y))
            })
        })
    }
}

#[derive(Deserialize)]
pub struct PlayerConfig {
    pub reach: Range<f32>,
}
