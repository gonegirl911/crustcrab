use crate::{game::world::chunk, protocol::compression::Compressed};
use nalgebra::Point3;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[derive(Serialize, Deserialize)]
pub enum Event {
    Loaded(Compressed<Arc<chunk::Data>>),
    Unloaded(Point3<i32>),
    BatchStarted(BatchTrigger),
    BatchEnded,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum BatchTrigger {
    Scheduler { allowance: usize },
    Action,
}
