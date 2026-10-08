pub mod block_action;
pub mod movement;
pub mod rotation;

use super::camera::View;
use crate::client::event_loop::{Event, EventHandler};
use bitflags::bitflags;
use block_action::BlockActionController;
use movement::MovementController;
use rotation::RotationController;
use std::time::Duration;

pub struct Controller {
    rotation: RotationController,
    movement: MovementController,
    block_action: BlockActionController,
    pub external_updates_applied: bool,
}

impl Controller {
    pub fn new(sensitivity: f32) -> Self {
        Self {
            rotation: RotationController::new(sensitivity),
            movement: Default::default(),
            block_action: Default::default(),
            external_updates_applied: false,
        }
    }

    pub fn apply_updates(&mut self, view: &mut View, dt: Duration) -> Changes {
        let mut changes = Changes::empty();

        if self.rotation.apply(view) {
            changes.insert(Changes::ROTATED);
        }

        if self.movement.apply(view, dt) {
            changes.insert(Changes::MOVED);
        }

        if let Some(action) = self.block_action.fire() {
            changes.insert(action);
        }

        changes
    }
}

impl EventHandler for Controller {
    type Context<'a> = ();

    fn handle(&mut self, event: &Event, (): Self::Context<'_>) {
        self.rotation.handle(event, ());
        self.movement.handle(event, ());
        self.block_action.handle(event, ());
    }
}

bitflags! {
    pub struct Changes: u8 {
        const MOVED = 1 << 0;
        const ROTATED = 1 << 1;
        const BLOCK_PLACED = 1 << 2;
        const BLOCK_DESTROYED = 1 << 3;
        const VIEW = Self::MOVED.bits() | Self::ROTATED.bits();
    }
}
