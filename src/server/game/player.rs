use super::world::{World, block::Block};
use crate::{
    client::PlayerEvent,
    server::{
        ControlEvent, SERVER_CONFIG,
        connection::{ConnectionEvent, ConnectionId, ConnectionRegistry},
        event_loop::{Event, EventHandler},
        game::world::block::data::STR_TO_BLOCK,
    },
    shared::{cuboid::Cuboid, ray::Ray, utils},
};
use nalgebra::{Point2, Point3, Vector3, point, vector};
use rayon::iter::ParallelIterator;
use rustc_hash::FxHashMap;
use serde::{
    Deserialize, Deserializer,
    de::{self, Unexpected},
};
use std::{collections::hash_map::Entry, ops::Deref, sync::Arc};

#[derive(Default)]
pub struct PlayerRegistry(pub FxHashMap<ConnectionId, Player>);

impl EventHandler<Event> for PlayerRegistry {
    type Context<'a> = &'a ConnectionRegistry;

    fn handle(&mut self, event: &Event, connections: Self::Context<'_>) {
        match *event {
            Event::Player(id, PlayerEvent::JoinRequested { .. }) => {
                let mut entry = self.0.entry(id).insert_entry(Default::default());
                let player = entry.get_mut();
                player.handle(event, connections);
            }
            Event::Player(id, _) => {
                let player = self.0.get_mut(&id).unwrap();
                player.handle(event, connections);
            }
            Event::Connection(ConnectionEvent::Closed(id)) => {
                if let Entry::Occupied(mut entry) = self.0.entry(id) {
                    let player = entry.get_mut();
                    player.handle(event, connections);
                    entry.remove();
                }
            }
            _ => {}
        }
    }
}

#[derive(Default)]
pub struct Player {
    pub prev: WorldArea,
    pub cur: WorldArea,
    pub aim: Ray,
}

impl EventHandler<Event> for Player {
    type Context<'a> = &'a ConnectionRegistry;

    fn handle(&mut self, event: &Event, connections: Self::Context<'_>) {
        self.prev = self.cur;

        if let Event::Player(id, event) = event {
            match *event {
                PlayerEvent::JoinRequested { render_distance } => {
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
                    self.aim = Ray {
                        origin,
                        dir: dir.cast(),
                    };

                    connections
                        .recipient(*id)
                        .send(ControlEvent::PlayerInitialized {
                            origin,
                            dir,
                            speed,
                            inventory: inventory.clone(),
                        });
                }
                PlayerEvent::PositionChanged { origin } => {
                    self.cur.center = utils::chunk_coords(origin);
                    self.aim.origin = origin;
                }
                PlayerEvent::OrientationChanged { dir } => {
                    self.aim.dir = dir.cast();
                }
                _ => {}
            }
        }
    }
}

#[derive(Clone, Copy, PartialEq, Default)]
pub struct WorldArea {
    pub center: Point3<i32>,
    pub radius: i32,
}

impl WorldArea {
    pub fn par_server_points(&self) -> impl ParallelIterator<Item = Point3<i32>> {
        self.par_cuboid_points()
            .filter(move |&coords| self.server_contains(coords))
    }

    pub fn client_points(&self) -> impl Iterator<Item = Point3<i32>> {
        self.cuboid_points()
            .filter(move |&coords| self.client_contains(coords))
    }

    pub fn par_exclusive_server_points(
        &self,
        other: &Self,
    ) -> impl ParallelIterator<Item = Point3<i32>> {
        self.par_server_points()
            .filter(move |&coords| !other.server_contains(coords))
    }

    pub fn exclusive_client_points(&self, other: &Self) -> impl Iterator<Item = Point3<i32>> {
        self.client_points()
            .filter(move |&coords| !other.client_contains(coords))
    }

    fn server_contains(&self, coords: Point3<i32>) -> bool {
        self.contains_xz(coords.xz())
    }

    pub fn client_contains(&self, coords: Point3<i32>) -> bool {
        self.contains_xz(coords.xz()) && self.client_contains_y(coords.y)
    }

    fn cuboid_points(&self) -> impl Iterator<Item = Point3<i32>> {
        self.cuboid().into_points().map(Point3::cast)
    }

    fn par_cuboid_points(&self) -> impl ParallelIterator<Item = Point3<i32>> {
        self.cuboid().into_par_points().map(Point3::cast)
    }

    fn contains_xz(&self, xz: Point2<i32>) -> bool {
        utils::distance_squared(xz, self.center.xz()) <= (self.radius as u128).pow(2)
    }

    fn client_contains_y(&self, y: i32) -> bool {
        y.abs_diff(self.center.y) <= self.radius as u32
    }

    fn cuboid(&self) -> Cuboid {
        let radius = self.radius as i64;
        let y_start = World::Y_RANGE.start as i64;
        let y_end = World::Y_RANGE.end as i64;
        Cuboid::from_corners(
            point![-radius, y_start, -radius],
            point![radius, y_end - 1, radius],
        )
        .translate(vector![self.center.x as i64, 0, self.center.z as i64])
    }
}

#[derive(Deserialize)]
pub struct PlayerConfig {
    origin: Point3<f64>,
    dir: Vector3<f32>,
    speed: f64,
    pub reach: f64,
    #[serde(deserialize_with = "PlayerConfig::deserialize_inventory")]
    inventory: Arc<[Block]>,
}

impl PlayerConfig {
    fn deserialize_inventory<'de, D>(deserializer: D) -> Result<Arc<[Block]>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let inventory = Box::<[_]>::deserialize(deserializer)?;
        if inventory.len() > 9 {
            return Err(de::Error::custom("inventory has only 9 available slots"));
        }
        inventory
            .into_iter()
            .map(|str| {
                STR_TO_BLOCK.get(str).copied().ok_or_else(|| {
                    de::Error::invalid_value(
                        Unexpected::Str(str),
                        &&*format!(
                            "one of [\"{}\"]",
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
