mod block_action;
mod inventory;
mod movement;
mod rotation;

use super::camera::View;
use crate::event::{self, Event};
use bitflags::bitflags;
use std::time::Duration;

pub(super) struct Controller {
    rotation: rotation::Controller,
    movement: movement::Controller,
    block_action: block_action::Controller,
    pub(super) inventory: inventory::Controller,
    pub(super) external_updates_applied: bool,
}

impl Controller {
    pub(super) fn new(sensitivity: f32) -> Self {
        Self {
            rotation: rotation::Controller::new(sensitivity),
            movement: Default::default(),
            block_action: Default::default(),
            inventory: Default::default(),
            external_updates_applied: false,
        }
    }

    pub(super) fn apply_updates(&mut self, view: &mut View, dt: Duration) -> Changes {
        let mut changes = Changes::default();

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

impl event::Handler for Controller {
    type Context<'a> = ();

    fn handle(&mut self, event: &Event, (): Self::Context<'_>) {
        self.rotation.handle(event, ());
        self.movement.handle(event, ());
        self.block_action.handle(event, ());
        self.inventory.handle(event, ());
    }
}

bitflags! {
    #[derive(Clone, Copy, Default)]
    pub(super) struct Changes: u8 {
        const MOVED = 1 << 0;
        const ROTATED = 1 << 1;
        const BLOCK_PLACED = 1 << 2;
        const BLOCK_DESTROYED = 1 << 3;
        const VIEW = Self::MOVED.bits() | Self::ROTATED.bits();
    }
}
