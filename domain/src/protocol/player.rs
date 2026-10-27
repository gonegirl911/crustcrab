use crate::game::world::block::Block;
use base::position::Position3;
use nalgebra::Vector3;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub enum Event {
    JoinRequested { render_distance: u32 },
    JoinAcknowledged,
    Position { origin: Position3 },
    Orientation { dir: Vector3<f32> },
    BlockPlaced(Block),
    BlockDestroyed,
    ChunkBatchAcknowledged { chunks_per_second: Option<f32> },
    KeepAlive { tag: u64 },
}
