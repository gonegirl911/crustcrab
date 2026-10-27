use crossbeam_channel::{Receiver, SendError};
use domain::protocol::control::KickReason;
use std::error::Error;
use thiserror::Error;
use winit::event_loop::EventLoopProxy;

pub fn channel(proxy: EventLoopProxy) -> (Sender, Receiver<Reason>) {
    let (tx, rx) = crossbeam_channel::unbounded();
    (Sender { tx, proxy }, rx)
}

#[derive(Clone)]
pub struct Sender {
    tx: crossbeam_channel::Sender<Reason>,
    proxy: EventLoopProxy,
}

impl Sender {
    pub fn send(&self, reason: Reason) -> Result<(), SendError<Reason>> {
        self.tx.send(reason)?;
        self.proxy.wake_up();
        Ok(())
    }
}

#[derive(Debug, Error)]
pub enum Reason {
    #[error("kicked: {0}")]
    Kicked(KickReason),
    #[error("server stopped responding")]
    ServerUnresponsive,
    #[error("connection closed")]
    Closed,
    #[error("connection error: {0}")]
    Error(Box<dyn Error + Send>),
}
