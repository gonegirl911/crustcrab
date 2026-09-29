use clap::Parser;
use crustcrab::{
    server::{
        Server,
        connection::{Connection, ConnectionEvent, ConnectionId},
    },
    shared::{codec, pool},
};
use std::{
    io::{BufReader, BufWriter},
    net::TcpListener,
    sync::Arc,
    thread,
};

#[derive(Parser)]
struct Args {
    #[arg(long, default_value = "localhost:8008")]
    event_addr: Arc<str>,
    #[arg(long, default_value = "localhost:8009")]
    chunk_addr: Arc<str>,
}

fn main() {
    pool::init(2);

    let (connection_tx, connection_rx) = crossbeam_channel::unbounded();
    let (player_tx, player_rx) = crossbeam_channel::unbounded();

    let Args {
        event_addr,
        chunk_addr,
    } = Parser::parse();

    let event_listener = match TcpListener::bind(&*event_addr) {
        Ok(listener) => {
            eprintln!("[{event_addr}] create TCP listener SUCCEEDED");
            listener
        }
        Err(e) => {
            eprintln!("[{event_addr}] create TCP listener FAILED: {e}");
            return;
        }
    };
    let chunk_listener = match TcpListener::bind(&*chunk_addr) {
        Ok(listener) => {
            eprintln!("[{chunk_addr}] create TCP listener SUCCEEDED");
            listener
        }
        Err(e) => {
            eprintln!("[{chunk_addr}] create TCP listener FAILED: {e}");
            return;
        }
    };

    thread::spawn(move || {
        for (event_stream, chunk_stream) in event_listener.incoming().zip(chunk_listener.incoming())
        {
            let event_stream = match event_stream {
                Ok(stream) => {
                    eprintln!("[{event_addr}] open TCP connection SUCCEEDED");
                    stream
                }
                Err(e) => {
                    eprintln!("[{event_addr}] open TCP connection FAILED: {e}");
                    continue;
                }
            };
            if let Err(e) = event_stream.set_nodelay(true) {
                eprintln!("[{event_addr}] disable Nagle algorithm FAILED: {e}");
            }
            let chunk_stream = match chunk_stream {
                Ok(stream) => {
                    eprintln!("[{chunk_addr}] open TCP connection SUCCEEDED");
                    stream
                }
                Err(e) => {
                    eprintln!("[{chunk_addr}] open TCP connection FAILED: {e}");
                    continue;
                }
            };
            if let Err(e) = chunk_stream.set_nodelay(true) {
                eprintln!("[{chunk_addr}] disable Nagle algorithm FAILED: {e}");
            }

            let id = ConnectionId::new();
            let (control_tx, control_rx) = crossbeam_channel::unbounded();
            let (chunk_tx, chunk_rx) = crossbeam_channel::unbounded();
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
                let event_addr = event_addr.clone();
                let event_stream = match event_stream.try_clone() {
                    Ok(stream) => stream,
                    Err(e) => {
                        eprintln!("[{event_addr}] clone event stream FAILED: {e}");
                        continue;
                    }
                };
                move || {
                    let mut control_writer = BufWriter::new(event_stream);
                    let mut buf = Vec::new();
                    for event in control_rx {
                        if let Err(e) = codec::send(&mut control_writer, &event, &mut buf) {
                            eprintln!("[{event_addr}] write control event FAILED: {e}");
                            break;
                        }
                    }
                    eprintln!("[{event_addr}] writing CLOSED");
                }
            });

            thread::spawn({
                let chunk_addr = chunk_addr.clone();
                move || {
                    let mut chunk_writer = BufWriter::new(chunk_stream);
                    let mut buf = Vec::new();
                    for event in chunk_rx {
                        if let Err(e) = codec::send(&mut chunk_writer, &event, &mut buf) {
                            eprintln!("[{chunk_addr}] write chunk event FAILED: {e}");
                            break;
                        }
                    }
                    eprintln!("[{chunk_addr}] writing CLOSED");
                }
            });

            thread::spawn({
                let player_tx = player_tx.clone();
                let event_addr = event_addr.clone();
                let connection_tx = connection_tx.clone();
                move || {
                    let mut player_reader = BufReader::new(event_stream);
                    let mut buf = Vec::new();
                    loop {
                        let event = match codec::recv(&mut player_reader, &mut buf) {
                            Ok(event) => event,
                            Err(codec::Error::ConnectionClosed) => break,
                            Err(e) => {
                                eprintln!("[{event_addr}] read player event FAILED: {e}");
                                break;
                            }
                        };
                        if player_tx.send((id, event)).is_err() {
                            break;
                        }
                    }
                    eprintln!("[{event_addr}] reading CLOSED");
                    _ = connection_tx.send(ConnectionEvent::Closed(id));
                }
            });
        }
    });

    let mut server = Server::new(connection_rx, player_rx);
    server.run();
}
