use crate::{
    client::PlayerEvent,
    server::game::{player::ChunkScope, world::chunk::Chunk},
};

pub fn sanitize(mut event: PlayerEvent) -> Option<PlayerEvent> {
    match &mut event {
        PlayerEvent::JoinRequested { render_distance } => {
            *render_distance = (*render_distance).clamp(1, MAX_RENDER_DISTANCE);
        }
        PlayerEvent::Position { origin } => {
            if origin.iter().any(|c| c.abs() > WORLD_BORDER) {
                return None;
            }
        }
        #[expect(clippy::collapsible_match)]
        PlayerEvent::Orientation { dir } => {
            if dir.iter().any(|c| !c.is_finite()) {
                return None;
            }
        }
        _ => {}
    }
    Some(event)
}

const MAX_RENDER_DISTANCE: u32 = 64;
const WORLD_BORDER: f64 = {
    let max_render_distance = MAX_RENDER_DISTANCE as i32;
    let server_context = ChunkScope::SERVER_CONTEXT;
    let buffer = 1;
    (i32::MAX - max_render_distance - server_context - buffer) as f64 * Chunk::DIM as f64
};
