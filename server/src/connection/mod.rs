use crossbeam_channel::Sender;
use domain::protocol::{chunk, control};
use std::fmt::{self, Display, Formatter};
use uuid::Uuid;

pub struct Connection {
    pub control_tx: Sender<control::Event>,
    pub chunk_tx: Sender<chunk::Event>,
}

#[derive(Clone, Copy)]
pub struct Id(Uuid);

impl Id {
    #[expect(clippy::new_without_default)]
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

impl Display for Id {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

pub enum Event {
    Opened(Id, Connection),
    Closed(Id),
}
