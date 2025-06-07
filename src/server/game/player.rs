use crate::{
    client::ClientEvent,
    server::{
        event_loop::{Event, EventHandler},
        game::world::World,
    },
    shared::{ray::Ray, utils},
};
use nalgebra::{Point2, Point3, point};
use rayon::prelude::*;
use serde::Deserialize;
use std::ops::Range;

#[derive(Default)]
pub struct Player {
    pub prev: WorldArea,
    pub cur: WorldArea,
    pub ray: Ray,
}

impl EventHandler<Event> for Player {
    type Context<'a> = ();

    fn handle(&mut self, event: &Event, (): Self::Context<'_>) {
        self.prev = self.cur;

        if let Event::Client(event) = event {
            match *event {
                ClientEvent::InitialRenderRequested {
                    origin,
                    dir,
                    render_distance,
                } => {
                    self.cur = WorldArea {
                        center: utils::chunk_coords(origin),
                        radius: render_distance as i32,
                    };
                    self.ray = Ray { origin, dir };
                }
                ClientEvent::PlayerOrientationChanged { dir } => {
                    self.ray.dir = dir;
                }
                ClientEvent::PlayerPositionChanged { origin } => {
                    self.cur.center = utils::chunk_coords(origin);
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
    pub fn par_server_points(self) -> impl ParallelIterator<Item = Point3<i32>> {
        self.par_cuboid_points()
            .filter(move |&coords| self.server_contains(coords))
    }

    pub fn client_points(self) -> impl Iterator<Item = Point3<i32>> {
        self.cuboid_points()
            .filter(move |&coords| self.client_contains(coords))
    }

    pub fn par_exclusive_server_points(
        self,
        other: Self,
    ) -> impl ParallelIterator<Item = Point3<i32>> {
        self.par_server_points()
            .filter(move |&coords| !other.server_contains(coords))
    }

    pub fn exclusive_client_points(self, other: Self) -> impl Iterator<Item = Point3<i32>> {
        self.client_points()
            .filter(move |&coords| !other.client_contains(coords))
    }

    fn server_contains(self, coords: Point3<i32>) -> bool {
        self.contains_xz(coords.xz())
    }

    pub fn client_contains(self, coords: Point3<i32>) -> bool {
        self.contains_xz(coords.xz()) && self.client_contains_y(coords.y)
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

    fn contains_xz(self, xz: Point2<i32>) -> bool {
        utils::magnitude_squared(xz, self.center.xz()) <= (self.radius as u128).pow(2)
    }

    fn client_contains_y(self, y: i32) -> bool {
        y.abs_diff(self.center.y) <= self.radius as u32
    }

    fn coords(self, dx: i32, y: i32, dz: i32) -> Point3<i32> {
        point![self.center.x + dx, y, self.center.z + dz]
    }
}

#[derive(Deserialize)]
pub struct PlayerConfig {
    pub reach: Range<f32>,
}
