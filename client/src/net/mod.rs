pub mod disconnect;

use crossbeam_channel::{Receiver, Sender};
use domain::{
    net::{KEEP_ALIVE_TIMEOUT, attach, codec},
    protocol::{chunk, control, player},
};
use log::{error, info, warn};
use std::{
    io::{self, BufReader, BufWriter},
    net::SocketAddr,
    sync::Arc,
    thread,
    time::Duration,
};
use winit::event_loop::EventLoopProxy;

pub fn connect(
    event_addr: SocketAddr,
    chunk_addr: SocketAddr,
    proxy: EventLoopProxy,
) -> (
    Sender<player::Event>,
    Receiver<control::Event>,
    Receiver<chunk::Event>,
    Receiver<disconnect::Reason>,
) {
    let [event_stream, chunk_stream] = attach::connect([event_addr, chunk_addr]);
    let event_stream = Arc::new(event_stream);
    let chunk_stream = Arc::new(chunk_stream);

    if let Err(e) = event_stream.set_read_timeout(Some(READ_TIMEOUT)) {
        warn!("[{event_addr}] set read timeout: {e}");
    }
    if let Err(e) = event_stream.set_write_timeout(Some(WRITE_TIMEOUT)) {
        warn!("[{event_addr}] set write timeout: {e}");
    }

    let (player_tx, player_rx) = crossbeam_channel::unbounded();
    let (control_tx, control_rx) = crossbeam_channel::unbounded();
    let (chunk_tx, chunk_rx) = crossbeam_channel::unbounded();
    let (disconnect_tx, disconnect_rx) = disconnect::channel(proxy);

    thread::spawn({
        let event_stream = event_stream.clone();
        move || {
            let mut player_writer = BufWriter::new(&*event_stream);
            let mut buf = Vec::new();
            for event in player_rx {
                match codec::send(&mut player_writer, &event, &mut buf) {
                    Ok(()) => {}
                    Err(codec::Error::Io(e))
                        if matches!(
                            e.kind(),
                            io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut
                        ) =>
                    {
                        info!("[{event_addr}] server stopped responding");
                        break;
                    }
                    Err(e) => {
                        error!("[{event_addr}] write player event: {e}");
                        break;
                    }
                }
            }
            info!("[{event_addr}] writing closed");
        }
    });

    thread::spawn({
        let player_tx = player_tx.clone();
        let disconnect_tx = disconnect_tx.clone();
        move || {
            let mut control_reader = BufReader::new(&*event_stream);
            let mut buf = Vec::new();
            let disconnect_reason = loop {
                let event = match codec::recv(&mut control_reader, &mut buf) {
                    Ok(event) => event,
                    Err(codec::Error::ConnectionClosed) => {
                        break disconnect::Reason::Closed;
                    }
                    Err(codec::Error::Io(e))
                        if matches!(
                            e.kind(),
                            io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut
                        ) =>
                    {
                        info!("[{event_addr}] server stopped responding");
                        break disconnect::Reason::ServerUnresponsive;
                    }
                    Err(e) => {
                        error!("[{event_addr}] read control event: {e}");
                        break disconnect::Reason::Error(Box::new(e));
                    }
                };

                if let Err(reason) = relay(event, &player_tx, &control_tx) {
                    if matches!(reason, disconnect::Reason::Kicked(_)) {
                        info!("[{event_addr}] kicked");
                    }
                    break reason;
                }
            };
            info!("[{event_addr}] reading closed");
            let _ = disconnect_tx.send(disconnect_reason);
        }
    });

    thread::spawn(move || {
        let mut chunk_reader = BufReader::new(&*chunk_stream);
        let mut buf = Vec::new();
        let disconnect_reason = loop {
            match codec::recv(&mut chunk_reader, &mut buf) {
                Ok(event) => {
                    if chunk_tx.send(event).is_err() {
                        break disconnect::Reason::Closed;
                    }
                }
                Err(codec::Error::ConnectionClosed) => {
                    break disconnect::Reason::Closed;
                }
                Err(e) => {
                    error!("[{chunk_addr}] read chunk event: {e}");
                    break disconnect::Reason::Error(Box::new(e));
                }
            }
        };
        info!("[{chunk_addr}] reading closed");
        let _ = disconnect_tx.send(disconnect_reason);
    });

    (player_tx, control_rx, chunk_rx, disconnect_rx)
}

pub fn relay(
    event: control::Event,
    player_tx: &Sender<player::Event>,
    control_tx: &Sender<control::Event>,
) -> Result<(), disconnect::Reason> {
    match event {
        control::Event::KeepAlive { tag } => {
            if player_tx.send(player::Event::KeepAlive { tag }).is_err() {
                return Err(disconnect::Reason::Closed);
            }
        }
        control::Event::Kicked { reason } => {
            return Err(disconnect::Reason::Kicked(reason));
        }
        _ => {
            if control_tx.send(event).is_err() {
                return Err(disconnect::Reason::Closed);
            }
        }
    }
    Ok(())
}

const READ_TIMEOUT: Duration = KEEP_ALIVE_TIMEOUT;
const WRITE_TIMEOUT: Duration = Duration::from_secs(30);
