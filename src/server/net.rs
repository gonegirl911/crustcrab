use super::{
    SERVER_CONFIG,
    connection::{Connection, ConnectionEvent, ConnectionId},
};
use crate::{
    client::PlayerEvent,
    shared::{
        enum_map::EnumMap,
        net::{ConnectionSettings, MIN_KEEPALIVE_INTERVAL_MS, attach, codec},
    },
};
use crossbeam_channel::Receiver;
use log::{error, info, warn};
use macros::Enum;
use serde::Deserialize;
use std::{
    io::{self, BufReader, BufWriter},
    net::{Shutdown, SocketAddr},
    sync::Arc,
    thread,
    time::Duration,
};

pub fn listen(
    event_addr: SocketAddr,
    chunk_addr: SocketAddr,
) -> (
    Receiver<ConnectionEvent>,
    Receiver<(ConnectionId, PlayerEvent)>,
) {
    let keepalive = SERVER_CONFIG.keepalive;
    if keepalive.interval_ms < MIN_KEEPALIVE_INTERVAL_MS {
        warn!(
            "keepalive interval ({}ms) below minimum threshold ({MIN_KEEPALIVE_INTERVAL_MS}ms); \
            clients will refuse connection",
            keepalive.interval_ms
        );
    }
    if keepalive.interval_ms * 2 >= keepalive.timeout_ms {
        warn!(
            "keepalive interval ({}ms) not safely below timeout ({}ms); \
            healthy idle clients may be evicted",
            keepalive.interval_ms, keepalive.timeout_ms
        );
    }
    let peer_timeout = Duration::from_millis(keepalive.timeout_ms);

    let attached_rx = attach::serve(
        EnumMap::from_iter([(Role::Event, event_addr), (Role::Chunk, chunk_addr)]),
        ConnectionSettings {
            keepalive_interval_ms: keepalive.interval_ms,
        },
    );

    let (connection_tx, connection_rx) = crossbeam_channel::unbounded();
    let (player_tx, player_rx) = crossbeam_channel::unbounded();

    thread::spawn(move || {
        for attached in attached_rx {
            let mut attached = attached.map(|_, (stream, addr)| Some((stream, addr)));
            let (event_stream, event_addr) = attached[Role::Event].take().unwrap();
            let (chunk_stream, chunk_addr) = attached[Role::Chunk].take().unwrap();
            let event_stream = Arc::new(event_stream);

            if let Err(e) = event_stream.set_read_timeout(Some(peer_timeout)) {
                warn!("[{event_addr}] set read timeout: {e}");
            }
            if let Err(e) = event_stream.set_write_timeout(Some(peer_timeout)) {
                warn!("[{event_addr}] set write timeout: {e}");
            }
            if let Err(e) = chunk_stream.set_write_timeout(Some(peer_timeout)) {
                warn!("[{chunk_addr}] set write timeout: {e}");
            }

            let (control_tx, control_rx) = crossbeam_channel::unbounded();
            let (chunk_tx, chunk_rx) = crossbeam_channel::unbounded();

            let id = ConnectionId::new();
            connection_tx
                .send(ConnectionEvent::Opened(
                    id,
                    Connection {
                        control_tx,
                        chunk_tx,
                    },
                ))
                .unwrap();

            thread::spawn({
                let event_stream = event_stream.clone();
                move || {
                    let mut control_writer = BufWriter::new(&*event_stream);
                    let mut buf = Vec::new();
                    for event in control_rx {
                        if let Err(e) = codec::send(&mut control_writer, &event, &mut buf) {
                            if let codec::Error::Io(e) = &e
                                && matches!(
                                    e.kind(),
                                    io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut
                                )
                            {
                                info!("[{event_addr}] stalled connection evicted");
                                _ = event_stream.shutdown(Shutdown::Both);
                            } else {
                                error!("[{event_addr}] write control event: {e}");
                            }
                            break;
                        }
                    }
                    info!("[{event_addr}] writing closed");
                }
            });

            thread::spawn({
                let event_stream = event_stream.clone();
                move || {
                    let mut chunk_writer = BufWriter::new(chunk_stream);
                    let mut buf = Vec::new();
                    for event in chunk_rx {
                        if let Err(e) = codec::send(&mut chunk_writer, &event, &mut buf) {
                            if let codec::Error::Io(e) = &e
                                && matches!(
                                    e.kind(),
                                    io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut
                                )
                            {
                                info!("[{chunk_addr}] stalled connection evicted");
                                _ = event_stream.shutdown(Shutdown::Both);
                            } else {
                                error!("[{chunk_addr}] write control event: {e}");
                            }
                            break;
                        }
                    }
                    info!("[{chunk_addr}] writing closed");
                }
            });

            thread::spawn({
                let connection_tx = connection_tx.clone();
                let player_tx = player_tx.clone();
                move || {
                    let mut player_reader = BufReader::new(&*event_stream);
                    let mut buf = Vec::new();
                    loop {
                        let event = match codec::recv(&mut player_reader, &mut buf) {
                            Ok(event) => event,
                            Err(codec::Error::Io(e))
                                if matches!(
                                    e.kind(),
                                    io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut
                                ) =>
                            {
                                info!("[{event_addr}] inactive connection evicted");
                                break;
                            }
                            Err(codec::Error::ConnectionClosed) => break,
                            Err(e) => {
                                error!("[{event_addr}] read player event: {e}");
                                break;
                            }
                        };
                        if player_tx.send((id, event)).is_err() {
                            break;
                        }
                    }
                    info!("[{event_addr}] reading closed");
                    _ = connection_tx.send(ConnectionEvent::Closed(id));
                }
            });
        }
    });

    (connection_rx, player_rx)
}

#[derive(Clone, Copy, Enum)]
enum Role {
    Event,
    Chunk,
}

#[derive(Clone, Copy, Deserialize)]
pub struct KeepAliveConfig {
    pub interval_ms: u64,
    pub timeout_ms: u64,
}
