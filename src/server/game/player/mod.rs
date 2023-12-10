pub mod ray;

use self::ray::Ray;
use super::world::World;
use crate::{
    client::ClientEvent,
    server::event_loop::{Event, EventHandler},
    shared::utils,
};
use nalgebra::{point, vector, Point3, Vector3};
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

    fn handle(&mut self, event: &Event, (): Self::Context<'_>) {
        self.prev = self.curr;

        if let Event::ClientEvent(event) = event {
            match *event {
                ClientEvent::InitialRenderRequested {
                    origin,
                    dir,
                    render_distance,
                } => {
                    self.curr = WorldArea {
                        center: utils::chunk_coords(origin),
                        radius: render_distance,
                    };
                    self.ray = Ray { origin, dir };
                }
                ClientEvent::PlayerOrientationChanged { dir } => {
                    self.ray.dir = dir;
                }
                ClientEvent::PlayerPositionChanged { origin } => {
                    self.curr.center = utils::chunk_coords(origin);
                    self.ray.origin = origin;
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
    pub fn points(self) -> impl Iterator<Item = Point3<i32>> {
        self.cuboid_points()
            .filter(move |&coords| self.contains(coords))
    }

    pub fn par_points(self) -> impl ParallelIterator<Item = Point3<i32>> {
        self.par_cuboid_points()
            .filter(move |&coords| self.contains(coords))
    }

    pub fn exclusive_points(self, other: WorldArea) -> impl Iterator<Item = Point3<i32>> {
        self.points()
            .filter(move |&coords| self.is_exclusive(coords, other))
    }

    pub fn par_exclusive_points(
        self,
        other: WorldArea,
    ) -> impl ParallelIterator<Item = Point3<i32>> {
        self.par_points()
            .filter(move |&coords| self.is_exclusive(coords, other))
    }

    fn cuboid_points(self) -> impl Iterator<Item = Point3<i32>> {
        let radius = self.radius as i32;
        (-radius..=radius).flat_map(move |dx| {
            self.y_range(radius).flat_map(move |dy| {
                (-radius..=radius).map(move |dz| self.coords(vector![dx, dy, dz]))
            })
        })
    }

    fn par_cuboid_points(self) -> impl ParallelIterator<Item = Point3<i32>> {
        let radius = self.radius as i32;
        (-radius..=radius).into_par_iter().flat_map(move |dx| {
            self.y_range(radius).into_par_iter().flat_map(move |dy| {
                (-radius..=radius)
                    .into_par_iter()
                    .map(move |dz| self.coords(vector![dx, dy, dz]))
            })
        })
    }

    fn contains(self, coords: Point3<i32>) -> bool {
        utils::magnitude_squared(coords.xz() - self.center.xz()) <= self.radius.pow(2)
    }

    fn is_exclusive(self, coords: Point3<i32>, other: WorldArea) -> bool {
        !other.contains(coords) || (coords.y - other.center.y).unsigned_abs() > other.radius
    }

    fn y_range(self, radius: i32) -> Range<i32> {
        World::Y_RANGE.start.max(self.center.y - radius)..World::Y_RANGE.end
    }

    fn coords(self, delta: Vector3<i32>) -> Point3<i32> {
        point![self.center.x, 0, self.center.z] + delta
    }
}

#[derive(Deserialize)]
pub struct PlayerConfig {
    pub reach: Range<f32>,
}
