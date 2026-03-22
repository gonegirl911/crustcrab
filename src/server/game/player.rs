use super::world::{World, block::Block};
use crate::{
    client::ClientEvent,
    server::{
        SERVER_CONFIG, ServerEvent, ServerSender,
        event_loop::{Event, EventHandler},
        game::world::block::data::STR_TO_BLOCK,
    },
    shared::{ray::Ray, utils},
};
use nalgebra::{Point2, Point3, Vector3, point};
use rayon::iter::{IntoParallelIterator, ParallelIterator};
use serde::{
    Deserialize, Deserializer,
    de::{self, Unexpected},
};
use std::{ops::Deref, sync::Arc};

#[derive(Default)]
pub struct Player {
    pub prev: WorldArea,
    pub cur: WorldArea,
    pub ray: Ray,
}

impl EventHandler<Event> for Player {
    type Context<'a> = &'a ServerSender;

    fn handle(&mut self, event: &Event, server_tx: Self::Context<'_>) {
        self.prev = self.cur;

        if let Event::Client(event) = event {
            match *event {
                ClientEvent::PlayerConnected { render_distance } => {
                    let PlayerConfig {
                        origin,
                        dir,
                        speed,
                        ref inventory,
                        ..
                    } = SERVER_CONFIG.player;

                    self.cur = WorldArea {
                        center: utils::chunk_coords(origin),
                        radius: render_distance as i32,
                    };
                    self.ray = Ray { origin, dir };

                    _ = server_tx.send(ServerEvent::PlayerInitialized {
                        origin,
                        dir,
                        speed,
                        inventory: inventory.clone(),
                    });
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
    pub origin: Point3<f32>,
    pub dir: Vector3<f32>,
    pub speed: f32,
    pub reach: f32,
    #[serde(deserialize_with = "PlayerConfig::deserialize_inventory")]
    pub inventory: Arc<[Block]>,
}

impl PlayerConfig {
    fn deserialize_inventory<'de, D>(deserializer: D) -> Result<Arc<[Block]>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let inventory = Box::<[_]>::deserialize(deserializer)?;
        assert!(inventory.len() <= 9, "inventory has only 9 available slots");
        inventory
            .into_iter()
            .map(|str| {
                STR_TO_BLOCK.get(str).copied().ok_or_else(|| {
                    de::Error::invalid_value(
                        Unexpected::Str(str),
                        &&*format!(
                            "one of \"{}\"",
                            STR_TO_BLOCK
                                .keys()
                                .map(Deref::deref)
                                .collect::<Vec<_>>()
                                .join("\", \"")
                        ),
                    )
                })
            })
            .collect()
    }
}
