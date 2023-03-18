use crate::{
    client::event_loop::{Event, EventHandler},
    server::ServerEvent,
};

#[derive(Default)]
pub struct Clock {
    time: f32,
}

impl Clock {
    pub fn time(&self) -> f32 {
        self.time
    }
}

impl EventHandler for Clock {
    type Context<'a> = ();

    fn handle(&mut self, event: &Event, _: Self::Context<'_>) {
        if let Event::UserEvent(ServerEvent::TimeUpdated { time }) = event {
            self.time = *time;
        }
    }
}
