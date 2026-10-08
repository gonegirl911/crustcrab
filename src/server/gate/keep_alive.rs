use crate::{
    server::{
        ControlEvent,
        connection::{ConnectionEvent, ConnectionId, ConnectionRegistry},
        event_loop::EventHandler,
    },
    shared::net::{KEEP_ALIVE_INTERVAL, KEEP_ALIVE_TIMEOUT},
};
use rustc_hash::FxHashMap;
use std::time::Instant;

#[derive(Default)]
pub struct KeepAliveRegistry(FxHashMap<ConnectionId, KeepAlive>);

impl KeepAliveRegistry {
    pub fn acknowledge(&mut self, id: ConnectionId, tag: u64) -> bool {
        let keep_alive = self.0.get_mut(&id).unwrap();
        if let KeepAlive::Pending {
            tag: expected,
            sent_at,
        } = *keep_alive
            && tag == expected
        {
            *keep_alive = KeepAlive::Idle {
                next: sent_at + KEEP_ALIVE_INTERVAL,
                tag,
            };
            true
        } else {
            false
        }
    }

    pub fn sweep(&mut self, connections: &ConnectionRegistry, now: Instant) -> Vec<ConnectionId> {
        let mut timed_out = vec![];
        for (&id, keep_alive) in &mut self.0 {
            match *keep_alive {
                KeepAlive::Pending { sent_at, .. }
                    if now.duration_since(sent_at) >= KEEP_ALIVE_TIMEOUT =>
                {
                    timed_out.push(id);
                }
                KeepAlive::Idle { next, tag } if now >= next => {
                    let tag = tag + 1;
                    connections.one(id).send(ControlEvent::KeepAlive { tag });
                    *keep_alive = KeepAlive::Pending { tag, sent_at: now };
                }
                _ => {}
            }
        }
        timed_out
    }
}

impl EventHandler<ConnectionEvent> for KeepAliveRegistry {
    type Context<'a> = ();

    fn handle(&mut self, event: &ConnectionEvent, (): Self::Context<'_>) {
        match *event {
            ConnectionEvent::Opened(id, _) => {
                self.0.insert(
                    id,
                    KeepAlive::Idle {
                        next: Instant::now() + KEEP_ALIVE_INTERVAL,
                        tag: 0,
                    },
                );
            }
            ConnectionEvent::Closed(id) => {
                self.0.remove(&id);
            }
        }
    }
}

#[derive(Clone, Copy)]
enum KeepAlive {
    Idle { next: Instant, tag: u64 },
    Pending { tag: u64, sent_at: Instant },
}
