use super::{
    block::{Block, BlockLight, area::BlockArea, data::STR_TO_BLOCK},
    chunk::Chunk,
    coords,
    world::World,
};
use crate::{
    client::PlayerEvent,
    server::{
        ControlEvent, SERVER_CONFIG,
        connection::{ConnectionEvent, ConnectionId, ConnectionRegistry},
        event_loop::{Event, EventHandler},
    },
    shared::{cuboid::Cuboid, ray::Ray, utils},
};
use nalgebra::{Point2, Point3, Vector3, point, vector};
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
    pub prev: ChunkScope,
    pub cur: ChunkScope,
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

                    self.cur = ChunkScope {
                        center: coords::chunk(origin),
                        radius: render_distance as i32,
                    };
                    self.aim = Ray {
                        origin,
                        dir: dir.cast(),
                    };

                    connections.one(*id).send(ControlEvent::PlayerInitialized {
                        origin,
                        dir,
                        speed,
                        inventory: inventory.clone(),
                    });
                }
                PlayerEvent::Position { origin } => {
                    self.cur.center = coords::chunk(origin);
                    self.aim.origin = origin;
                }
                PlayerEvent::Orientation { dir } => {
                    self.aim.dir = dir.cast();
                }
                _ => {}
            }
        }
    }
}

#[derive(Clone, Copy, PartialEq, Default)]
pub struct ChunkScope {
    pub center: Point3<i32>,
    pub radius: i32,
}

impl ChunkScope {
    pub const SERVER_CONTEXT: i32 =
        (BlockLight::COMPONENT_MAX as usize - 1 + BlockArea::PADDING).div_ceil(Chunk::DIM) as i32;

    pub fn server_points(&self) -> impl Iterator<Item = Point3<i32>> {
        self.server_cuboid()
            .into_points()
            .map(Point3::cast)
            .filter(move |&coords| self.server_contains(coords))
    }

    pub fn client_points(&self) -> impl Iterator<Item = Point3<i32>> {
        self.client_cuboid()
            .into_points()
            .map(Point3::cast)
            .filter(move |&coords| self.client_contains(coords))
    }

    pub fn exclusive_server_points(&self, other: &Self) -> impl Iterator<Item = Point3<i32>> {
        self.server_points()
            .filter(move |&coords| !other.server_contains(coords))
    }

    pub fn exclusive_client_points(&self, other: &Self) -> impl Iterator<Item = Point3<i32>> {
        self.client_points()
            .filter(move |&coords| !other.client_contains(coords))
    }

    pub fn server_contains(&self, coords: Point3<i32>) -> bool {
        self.contains_xz(coords.xz(), Self::SERVER_CONTEXT) && self.server_contains_y(coords.y)
    }

    pub fn client_contains(&self, coords: Point3<i32>) -> bool {
        self.contains_xz(coords.xz(), 0) && self.client_contains_y(coords.y)
    }

    fn contains_xz(&self, xz: Point2<i32>, context: i32) -> bool {
        let center_xz = self.center.xz();
        let nearest = point![
            center_xz.x.clamp(xz.x - context, xz.x + context),
            center_xz.y.clamp(xz.y - context, xz.y + context),
        ];
        utils::distance_squared(nearest, center_xz) <= (self.radius as u128).pow(2)
    }

    fn server_contains_y(&self, y: i32) -> bool {
        y >= self.y_start(Self::SERVER_CONTEXT)
    }

    fn client_contains_y(&self, y: i32) -> bool {
        let y_start = self.y_start(0);
        let y_end = self.client_y_end();
        y >= y_start && y <= y_end
    }

    fn server_cuboid(&self) -> Cuboid {
        let y_end = World::Y_RANGE.end - 1;
        self.cuboid(Self::SERVER_CONTEXT, y_end)
    }

    fn client_cuboid(&self) -> Cuboid {
        let y_end = self.client_y_end();
        self.cuboid(0, y_end)
    }

    fn cuboid(&self, context: i32, y_end: i32) -> Cuboid {
        let radius = self.radius as i64 + context as i64;
        let y_start = self.y_start(context).min(World::Y_RANGE.end - 1);
        let y_end = y_end.max(World::Y_RANGE.start);
        Cuboid::from_corners(
            point![-radius, y_start as i64, -radius],
            point![radius, y_end as i64, radius],
        )
        .translate(vector![self.center.x as i64, 0, self.center.z as i64])
    }

    fn y_start(&self, context: i32) -> i32 {
        (self.center.y - self.radius - context).max(World::Y_RANGE.start)
    }

    fn client_y_end(&self) -> i32 {
        (self.center.y + self.radius).min(World::Y_RANGE.end - 1)
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
        let inventory = Vec::deserialize(deserializer)?;

        if inventory.len() > 9 {
            return Err(de::Error::invalid_value(
                Unexpected::Seq,
                &"an inventory with at most 9 entries",
            ));
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
