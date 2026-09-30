use super::PlayerEvent;
use crate::server::{ChunkEvent, ControlEvent};
use crate::shared::net::{attach, codec};
use crossbeam_channel::{Receiver, Sender};
use log::{error, info};
use std::net::SocketAddr;
use std::{
    io::{BufReader, BufWriter},
    process, thread,
};

pub fn connect(
    event_addr: SocketAddr,
    chunk_addr: SocketAddr,
) -> (
    Sender<PlayerEvent>,
    Receiver<ControlEvent>,
    Receiver<ChunkEvent>,
) {
    let [event_stream, chunk_stream] = attach::connect([event_addr, chunk_addr]);

    let (player_tx, player_rx) = crossbeam_channel::unbounded();
    let (control_tx, control_rx) = crossbeam_channel::unbounded();
    let (chunk_tx, chunk_rx) = crossbeam_channel::unbounded();

    thread::spawn({
        let event_stream = match event_stream.try_clone() {
            Ok(stream) => stream,
            Err(e) => {
                error!("[{event_addr}] clone event stream: {e}");
                process::exit(1);
            }
        };
        move || {
            let mut control_reader = BufReader::new(event_stream);
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
        let mut player_writer = BufWriter::new(&event_stream);
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

    (player_tx, control_rx, chunk_rx)
}
