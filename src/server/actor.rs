use super::{
    connection::{ConnectionEvent, ConnectionRegistry},
    event_loop::EventHandler,
};
use crossbeam_channel::Sender;
use std::thread;

pub struct Actor<E> {
    tx: Sender<Envelope<E>>,
}

impl<E> Actor<E> {
    pub fn forward(&self, event: &ConnectionEvent) {
        _ = self.tx.send(Envelope::Connection(event.clone()));
    }

    pub fn send(&self, event: E) {
        _ = self.tx.send(Envelope::Event(event));
    }
}

impl<E: Send + 'static> Actor<E> {
    pub fn spawn<A>(mut actor: A) -> Self
    where
        A: for<'a> EventHandler<E, Context<'a> = &'a ConnectionRegistry> + Send + 'static,
    {
        let (tx, rx) = crossbeam_channel::unbounded();
        thread::spawn(move || {
            let mut connections = ConnectionRegistry::default();
            for message in rx {
                match message {
                    Envelope::Connection(event) => {
                        connections.handle(&event, ());
                    }
                    Envelope::Event(event) => {
                        actor.handle(&event, &connections);
                    }
                }
            }
        });
        Self { tx }
    }
}

enum Envelope<E> {
    Connection(ConnectionEvent),
    Event(E),
}
