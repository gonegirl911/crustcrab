pub mod event_loop;
pub mod scene;
pub mod ticker;

use self::{
    event_loop::EventLoop,
    scene::{player::ray::BlockIntersection, world::chunk::ChunkData, Scene},
};
use crate::{
    client::ClientEvent,
    server::event_loop::{Event, EventHandler},
};
use flume::{Receiver, Sender};
use nalgebra::Point3;
use std::sync::Arc;

pub struct Server {
    event_loop: EventLoop,
    scene: Scene,
}

impl Server {
    pub fn new(server_tx: Sender<ServerEvent>, client_rx: Receiver<ClientEvent>) -> Self {
        Self {
            event_loop: EventLoop::new(server_tx, client_rx),
            scene: Scene::default(),
        }
    }

    pub fn run(self) -> ! {
        struct MiniServer {
            scene: Scene,
        }

        impl EventHandler<Event> for MiniServer {
            type Context<'a> = Sender<ServerEvent>;

            fn handle(&mut self, event: &Event, server_tx: Self::Context<'_>) {
                self.scene.handle(event, server_tx);
            }
        }

        self.event_loop.run(MiniServer { scene: self.scene })
    }
}

pub enum ServerEvent {
    TimeUpdated {
        time: f32,
    },
    ChunkLoaded {
        coords: Point3<i32>,
        data: Arc<ChunkData>,
    },
    ChunkUnloaded {
        coords: Point3<i32>,
    },
    ChunkUpdated {
        coords: Point3<i32>,
        data: Arc<ChunkData>,
    },
    BlockSelected {
        data: Option<BlockIntersection>,
    },
}
