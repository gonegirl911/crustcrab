use super::connection::{Connection, ConnectionEvent, ConnectionId};
use crate::{
    client::PlayerEvent,
    server::ControlEvent,
    shared::{
        enum_map::EnumMap,
        net::{KEEP_ALIVE_TIMEOUT, attach, codec},
    },
};
use crossbeam_channel::Receiver;
use log::{error, info, warn};
use macros::Enum;
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
    let attached_rx = attach::serve(EnumMap::from_iter([
        (Role::Event, event_addr),
        (Role::Chunk, chunk_addr),
    ]));

    let (connection_tx, connection_rx) = crossbeam_channel::unbounded();
    let (player_tx, player_rx) = crossbeam_channel::unbounded();

    thread::spawn(move || {
        for attached in attached_rx {
            let mut attached = attached.map(|_, (stream, addr)| Some((stream, addr)));
            let (event_stream, event_addr) = attached[Role::Event].take().unwrap();
            let (chunk_stream, chunk_addr) = attached[Role::Chunk].take().unwrap();
            let event_stream = Arc::new(event_stream);

            if let Err(e) = event_stream.set_read_timeout(Some(PEER_TIMEOUT)) {
                warn!("[{event_addr}] set read timeout: {e}");
            }
            if let Err(e) = event_stream.set_write_timeout(Some(PEER_TIMEOUT)) {
                warn!("[{event_addr}] set write timeout: {e}");
            }
            if let Err(e) = chunk_stream.set_write_timeout(Some(PEER_TIMEOUT)) {
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
            info!("[{event_addr}] opened connection {id}");

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

                        if matches!(event, ControlEvent::Kicked { .. }) {
                            info!("[{event_addr}] kicked connection closed");
                            _ = event_stream.shutdown(Shutdown::Both);
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
                                error!("[{chunk_addr}] write chunk event: {e}");
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
                        match codec::recv(&mut player_reader, &mut buf) {
                            Ok(event) => {
                                if player_tx.send((id, event)).is_err() {
                                    break;
                                }
                            }
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

const PEER_TIMEOUT: Duration = KEEP_ALIVE_TIMEOUT;
