pub mod ray;

use self::ray::Ray;
use super::world::chunk::Chunk;
use crate::{
    client::ClientEvent,
    server::event_loop::{Event, EventHandler},
};
use nalgebra::{vector, Point3};
use std::ops::Range;

#[derive(Default)]
pub struct Player {
    pub prev: WorldArea,
    pub curr: WorldArea,
    pub ray: Ray,
}

impl Player {
    pub const BUILDING_REACH: Range<f32> = 0.0..4.5;

    fn chunk_coords(coords: Point3<f32>) -> Point3<i32> {
        coords.map(|c| (c / Chunk::DIM as f32).floor() as i32)
    }
}

impl EventHandler<Event> for Player {
    type Context<'a> = ();

    fn handle(&mut self, event: &Event, _: Self::Context<'_>) {
        self.prev = self.curr;

        if let Event::ClientEvent(event) = event {
            match event {
                ClientEvent::InitialRenderRequested {
                    player_dir,
                    player_coords,
                    render_distance,
                } => {
                    self.curr = WorldArea {
                        center: Self::chunk_coords(*player_coords),
                        radius: *render_distance,
                    };
                    self.ray = Ray {
                        origin: *player_coords,
                        dir: *player_dir,
                    };
                }
                ClientEvent::PlayerOrientationChanged { dir } => {
                    self.ray.dir = *dir;
                }
                ClientEvent::PlayerPositionChanged { coords } => {
                    self.curr.center = Self::chunk_coords(*coords);
                    self.ray.origin = *coords;
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
        let radius = self.radius as i32;
        let radius2 = radius.pow(2);
        (-radius..=radius).flat_map(move |dx| {
            (-radius..=radius).flat_map(move |dy| {
                (-radius..=radius).filter_map(move |dz| {
                    let dist2 = dx.pow(2) + dy.pow(2) + dz.pow(2);
                    (dist2 <= radius2).then_some(self.center + vector![dx, dy, dz])
                })
            })
        })
    }

    pub fn exclusive_points<'a>(
        &'a self,
        other: &'a WorldArea,
    ) -> impl Iterator<Item = Point3<i32>> + 'a {
        let radius2 = other.radius.pow(2) as i32;
        self.points().filter_map(move |point| {
            let dist2 = (point - other.center).map(|c| c.pow(2)).sum();
            (dist2 > radius2).then_some(point)
        })
    }
}
