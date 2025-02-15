#![feature(let_chains)]

use anyhow::Context;
use clap::Parser;
use crustcrab::server::{Server, ServerSender};
use std::{
    io::{self, BufReader, BufWriter, Write},
    net::TcpListener,
    thread,
};

#[derive(Parser)]
struct Args {
    #[arg(long, default_value_t = 8008)]
    port: u16,
}

fn main() -> anyhow::Result<()> {
    let (client_tx, client_rx) = crossbeam_channel::unbounded();
    let (server_tx, server_rx) = crossbeam_channel::unbounded();
    let server_tx = ServerSender::Sender(server_tx);
    let server = Server::new(server_tx, client_rx);

    let Args { port } = Args::parse();
    let addr = format!("127.0.0.1:{port}");
    let listener = TcpListener::bind(&addr)
        .with_context(|| format!("failed to open a TCP listener to address {addr}"))?;

    thread::scope(|s| {
        let handle = s.spawn(|| {
            for stream in listener.incoming() {
                let stream = stream.with_context(|| {
                    format!("failed to open a TCP connection to address {addr}")
                })?;
                eprintln!("client connected");
                stream
                    .set_nodelay(true)
                    .context("failed to disable the Nagle algorithm")?;

                thread::scope(|s| {
                    s.spawn(|| {
                        let mut stream = BufWriter::new(&stream);
                        for event in &server_rx {
                            if let Err(e) = bincode::serialize_into(&mut stream, &event) {
                                if let bincode::ErrorKind::Io(e) = &*e
                                    && e.kind() == io::ErrorKind::BrokenPipe
                                {
                                    break;
                                }
                                eprintln!("failed to serialize and write server event: {e:?}");
                                continue;
                            }
                            if let Err(e) = stream.flush() {
                                if e.kind() == io::ErrorKind::BrokenPipe {
                                    break;
                                }
                                eprintln!("failed to flush buffered stream: {e:?}");
                            }
                        }
                    });

                    s.spawn(|| {
                        let mut stream = BufReader::new(&stream);
                        loop {
                            let event = match bincode::deserialize_from(&mut stream) {
                                Ok(event) => event,
                                Err(e) => {
                                    if let bincode::ErrorKind::Io(e) = &*e
                                        && let io::ErrorKind::UnexpectedEof
                                        | io::ErrorKind::ConnectionReset = e.kind()
                                    {
                                        break;
                                    }
                                    eprintln!("failed to read and deserialize client event: {e:?}");
                                    continue;
                                }
                            };
                            if client_tx.send(event).is_err() {
                                break;
                            }
                        }
                    });
                });

                eprintln!("client disconnected");
            }

            Ok(())
        });

        server.run();

        handle.join().unwrap()
    })
}
