pub mod ray;

use self::ray::Ray;
use super::world::World;
use crate::{
    client::ClientEvent,
    server::event_loop::{Event, EventHandler},
    shared::utils,
};
use nalgebra::{point, Point3};
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

        if let Event::Client(event) = event {
            match *event {
                ClientEvent::InitialRenderRequested {
                    origin,
                    dir,
                    render_distance,
                } => {
                    self.curr = WorldArea {
                        center: utils::chunk_coords(origin),
                        radius: render_distance as i32,
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
    center: Point3<i32>,
    radius: i32,
}

impl WorldArea {
    fn points(self) -> impl Iterator<Item = Point3<i32>> {
        self.cuboid_points()
            .filter(move |&coords| self.contains(coords))
    }

    pub fn par_points(self) -> impl ParallelIterator<Item = Point3<i32>> {
        self.par_cuboid_points()
            .filter(move |&coords| self.contains(coords))
    }

    pub fn exclusive_points(self, other: Self) -> impl Iterator<Item = Point3<i32>> {
        self.points().filter(move |&coords| !other.contains(coords))
    }

    pub fn par_exclusive_points(self, other: Self) -> impl ParallelIterator<Item = Point3<i32>> {
        self.par_points()
            .filter(move |&coords| !other.contains(coords))
    }

    fn cuboid_points(self) -> impl Iterator<Item = Point3<i32>> {
        (-self.radius..=self.radius).flat_map(move |dx| {
            World::Y_RANGE.flat_map(move |y| {
                (-self.radius..=self.radius).map(move |dz| self.coords(dx, y, dz))
            })
        })
    }

    fn par_cuboid_points(self) -> impl ParallelIterator<Item = Point3<i32>> {
        (-self.radius..=self.radius)
            .into_par_iter()
            .flat_map(move |dx| {
                World::Y_RANGE.into_par_iter().flat_map(move |y| {
                    (-self.radius..=self.radius)
                        .into_par_iter()
                        .map(move |dz| self.coords(dx, y, dz))
                })
            })
    }

    fn contains(self, coords: Point3<i32>) -> bool {
        utils::magnitude_squared(coords.xz() - self.center.xz()) <= self.radius.pow(2)
    }

    fn coords(self, dx: i32, y: i32, dz: i32) -> Point3<i32> {
        point![self.center.x + dx, y, self.center.z + dz]
    }
}

#[derive(Deserialize)]
pub struct PlayerConfig {
    pub reach: Range<f32>,
}
