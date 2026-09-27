use clap::Parser;
use crustcrab::{
    server::{Connection, Server},
    shared::{codec, pool},
};
use std::{
    io::{BufReader, BufWriter},
    net::TcpListener,
    thread,
};

#[derive(Parser)]
struct Args {
    #[arg(long, default_value = "localhost:8008")]
    control_addr: String,
    #[arg(long, default_value = "localhost:8009")]
    chunk_addr: String,
}

fn main() {
    pool::init(2);

    let (connection_tx, connection_rx) = crossbeam_channel::unbounded();
    let (player_tx, player_rx) = crossbeam_channel::unbounded();

    let mut server = Server::new(connection_rx, player_rx);

    thread::spawn(move || {
        let Args {
            control_addr,
            chunk_addr,
        } = Parser::parse();
        let control_listener = match TcpListener::bind(&control_addr) {
            Ok(listener) => {
                eprintln!("[{control_addr}] create TCP listener SUCCEEDED");
                listener
            }
            Err(e) => {
                eprintln!("[{control_addr}] create TCP listener FAILED: {e}");
                return;
            }
        };
        let chunk_listener = match TcpListener::bind(&chunk_addr) {
            Ok(listener) => {
                eprintln!("[{chunk_addr}] create TCP listener SUCCEEDED");
                listener
            }
            Err(e) => {
                eprintln!("[{chunk_addr}] create TCP listener FAILED: {e}");
                return;
            }
        };

        for (control_stream, chunk_stream) in
            control_listener.incoming().zip(chunk_listener.incoming())
        {
            let control_stream = match control_stream {
                Ok(stream) => {
                    eprintln!("[{control_addr}] open TCP connection SUCCEEDED");
                    stream
                }
                Err(e) => {
                    eprintln!("[{control_addr}] open TCP connection FAILED: {e}");
                    continue;
                }
            };
            if let Err(e) = control_stream.set_nodelay(true) {
                eprintln!("[{control_addr}] disable Nagle algorithm FAILED: {e}");
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

            let (control_tx, control_rx) = crossbeam_channel::unbounded();
            let (chunk_tx, chunk_rx) = crossbeam_channel::unbounded();
            connection_tx
                .send(Connection {
                    control_tx,
                    chunk_tx,
                })
                .unwrap();

            thread::scope(|s| {
                s.spawn(|| {
                    let mut control_writer = BufWriter::new(&control_stream);
                    let mut buf = Vec::new();
                    for event in control_rx {
                        if let Err(e) = codec::send(&mut control_writer, &event, &mut buf) {
                            eprintln!("[{control_addr}] write server event FAILED: {e}");
                            break;
                        }
                    }
                    eprintln!("[{control_addr}] writing CLOSED");
                });

                s.spawn(|| {
                    let mut chunk_writer = BufWriter::new(&chunk_stream);
                    let mut buf = Vec::new();
                    for event in chunk_rx {
                        if let Err(e) = codec::send(&mut chunk_writer, &event, &mut buf) {
                            eprintln!("[{chunk_addr}] write server event FAILED: {e}");
                            break;
                        }
                    }
                    eprintln!("[{chunk_addr}] writing CLOSED");
                });

                let mut control_reader = BufReader::new(&control_stream);
                let mut buf = Vec::new();
                loop {
                    let event = match codec::recv(&mut control_reader, &mut buf) {
                        Ok(event) => event,
                        Err(codec::Error::ConnectionClosed) => break,
                        Err(e) => {
                            eprintln!("[{control_addr}] read client event FAILED: {e}");
                            break;
                        }
                    };
                    player_tx.send(event).unwrap();
                }
                eprintln!("[{control_addr}] reading CLOSED");

                connection_tx.send(Connection::closed()).unwrap();
            });
        }
    });

    server.run();
}
