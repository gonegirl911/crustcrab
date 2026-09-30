use super::connection::{Connection, ConnectionEvent, ConnectionId};
use crate::{
    client::PlayerEvent,
    shared::{
        enum_map::EnumMap,
        net::{attach, codec},
    },
};
use crossbeam_channel::Receiver;
use log::{error, info};
use macros::Enum;
use std::{
    io::{BufReader, BufWriter},
    net::SocketAddr,
    thread,
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
                let event_stream = match event_stream.try_clone() {
                    Ok(stream) => stream,
                    Err(e) => {
                        error!("[{event_addr}] clone event stream: {e}");
                        continue;
                    }
                };
                move || {
                    let mut control_writer = BufWriter::new(event_stream);
                    let mut buf = Vec::new();
                    for event in control_rx {
                        if let Err(e) = codec::send(&mut control_writer, &event, &mut buf) {
                            error!("[{event_addr}] write control event: {e}");
                            break;
                        }
                    }
                    info!("[{event_addr}] writing closed");
                }
            });

            thread::spawn({
                move || {
                    let mut chunk_writer = BufWriter::new(chunk_stream);
                    let mut buf = Vec::new();
                    for event in chunk_rx {
                        if let Err(e) = codec::send(&mut chunk_writer, &event, &mut buf) {
                            error!("[{chunk_addr}] write chunk event: {e}");
                            break;
                        }
                    }
                    info!("[{chunk_addr}] writing closed");
                }
            });

            thread::spawn({
                let player_tx = player_tx.clone();
                let connection_tx = connection_tx.clone();
                move || {
                    let mut player_reader = BufReader::new(event_stream);
                    let mut buf = Vec::new();
                    loop {
                        let event = match codec::recv(&mut player_reader, &mut buf) {
                            Ok(event) => event,
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
