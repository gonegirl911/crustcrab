use crate::{
    client::PlayerEvent,
    server::game::{chunk::Chunk, player::ChunkScope},
};

pub fn sanitize(mut event: PlayerEvent) -> Option<PlayerEvent> {
    match &mut event {
        PlayerEvent::JoinRequested { render_distance } => {
            *render_distance = (*render_distance).clamp(1, MAX_RENDER_DISTANCE);
        }
        PlayerEvent::Position { origin } => {
            if origin.iter().any(|c| !c.is_finite()) {
                return None;
            }

            origin.apply(|c| *c = c.clamp(-WORLD_BORDER, WORLD_BORDER));
        }
        PlayerEvent::Orientation { dir } => {
            if dir.iter().any(|c| !c.is_finite()) {
                return None;
            }

            #[expect(clippy::question_mark)]
            let Some(normalized) = dir.cast::<f64>().try_normalize(f64::EPSILON) else {
                return None;
            };

            *dir = normalized.cast();
        }
        #[expect(clippy::collapsible_match)]
        PlayerEvent::ChunkBatchAcknowledged { chunks_per_second } => {
            if !chunks_per_second.is_finite() || *chunks_per_second < 0.0 {
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
