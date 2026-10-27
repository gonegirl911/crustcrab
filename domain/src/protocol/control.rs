use crate::game::{
    clock::DayCycle,
    world::block::{self, Block},
};
use base::position::Position3;
use compact_str::CompactString;
use nalgebra::Vector3;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use thiserror::Error;

#[derive(Serialize, Deserialize)]
pub enum Event {
    TimeInitialized {
        ticks_per_second: u32,
        cycle: DayCycle,
        ticks: u16,
    },
    PlayerInitialized {
        origin: Position3,
        dir: Vector3<f32>,
        speed: f32,
        inventory: Arc<[Block]>,
    },
    WorldInitialized {
        block_names: block::Catalog<CompactString>,
    },
    JoinFinished,
    TimeUpdated {
        ticks: u16,
    },
    BlockHovered(Option<Arc<block::hover::Data>>),
    KeepAlive {
        tag: u64,
    },
    Kicked {
        reason: KickReason,
    },
}

#[derive(Debug, Error, Serialize, Deserialize)]
pub enum KickReason {
    #[error("invalid event")]
    InvalidEvent,
    #[error("excessive event rate ({violations} violations)")]
    ExcessiveRate { violations: u32 },
    #[error("connection timed out")]
    TimedOut,
}
