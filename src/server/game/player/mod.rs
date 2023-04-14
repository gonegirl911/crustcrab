pub mod ray;

use self::ray::Ray;
use super::world::chunk::Chunk;
use crate::{
    client::ClientEvent,
    server::event_loop::{Event, EventHandler},
    shared::utils,
};
use nalgebra::{vector, Point3};

#[derive(Default)]
pub struct Player {
    pub prev: WorldArea,
    pub curr: WorldArea,
    pub ray: Ray,
}

impl Player {
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
                    origin,
                    dir,
                    render_distance,
                } => {
                    self.curr = WorldArea {
                        center: Self::chunk_coords(*origin),
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
                    self.curr.center = Self::chunk_coords(*origin);
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
        self.square_points()
            .filter(|point| utils::magnitude_squared(point - self.center) <= self.radius.pow(2))
    }

    pub fn exclusive_points<'a>(
        &'a self,
        other: &'a WorldArea,
    ) -> impl Iterator<Item = Point3<i32>> + 'a {
        self.points()
            .filter(|point| utils::magnitude_squared(point - other.center) > other.radius.pow(2))
    }

    fn square_points(&self) -> impl Iterator<Item = Point3<i32>> + '_ {
        let radius = self.radius as i32;
        (-radius..=radius).flat_map(move |dx| {
            (-radius..=radius).flat_map(move |dy| {
                (-radius..=radius).map(move |dz| self.center + vector![dx, dy, dz])
            })
        })
    }
}
