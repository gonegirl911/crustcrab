use super::PlayerEvent;
use crate::server::{ChunkEvent, ControlEvent};
use crate::shared::net::{attach, codec};
use crossbeam_channel::{Receiver, Sender};
use log::{error, info};
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;
use std::{
    io::{BufReader, BufWriter},
    thread,
};

pub fn connect(
    event_addr: SocketAddr,
    chunk_addr: SocketAddr,
) -> (
    Sender<PlayerEvent>,
    Receiver<ControlEvent>,
    Receiver<ChunkEvent>,
) {
    let ([event_stream, chunk_stream], settings) = attach::connect([event_addr, chunk_addr]);
    let event_stream = Arc::new(event_stream);
    let keepalive_interval = Duration::from_millis(settings.keepalive_interval_ms);

    let (player_tx, player_rx) = crossbeam_channel::unbounded();
    let (control_tx, control_rx) = crossbeam_channel::unbounded();
    let (chunk_tx, chunk_rx) = crossbeam_channel::unbounded();

    thread::spawn({
        let event_stream = event_stream.clone();
        move || {
            let mut control_reader = BufReader::new(&*event_stream);
            let mut buf = Vec::new();
            loop {
                let event = match codec::recv(&mut control_reader, &mut buf) {
                    Ok(event) => event,
                    Err(codec::Error::ConnectionClosed) => break,
                    Err(e) => {
                        error!("[{event_addr}] read control event: {e}");
                        break;
                    }
                };
                if control_tx.send(event).is_err() {
                    break;
                }
            }
            info!("[{event_addr}] reading closed");
        }
    });

    thread::spawn(move || {
        let mut player_writer = BufWriter::new(&*event_stream);
        let mut buf = Vec::new();
        for event in player_rx {
            if let Err(e) = codec::send(&mut player_writer, &event, &mut buf) {
                error!("[{event_addr}] write player event: {e}");
                break;
            }
        }
        info!("[{event_addr}] writing closed");
    });

    thread::spawn(move || {
        let mut chunk_reader = BufReader::new(chunk_stream);
        let mut buf = Vec::new();
        loop {
            let event = match codec::recv(&mut chunk_reader, &mut buf) {
                Ok(event) => event,
                Err(codec::Error::ConnectionClosed) => break,
                Err(e) => {
                    error!("[{chunk_addr}] read chunk event: {e}");
                    break;
                }
            };
            if chunk_tx.send(event).is_err() {
                break;
            }
        }
        info!("[{chunk_addr}] reading closed");
    });

    thread::spawn({
        let player_tx = player_tx.clone();
        move || loop {
            thread::sleep(keepalive_interval);
            if player_tx.send(PlayerEvent::KeepAlive).is_err() {
                break;
            }
        }
    });

    (player_tx, control_rx, chunk_rx)
}
