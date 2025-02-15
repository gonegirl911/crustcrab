#![feature(let_chains)]

use anyhow::Context;
use clap::Parser;
use crustcrab::client::Client;
use std::{
    io::{self, BufReader, BufWriter, Write},
    net::{Shutdown, TcpStream},
    thread,
};

#[derive(Parser)]
struct Args {
    #[arg(long, default_value_t = 8008)]
    port: u16,
}

fn main() -> anyhow::Result<()> {
    let (client_tx, client_rx) = crossbeam_channel::unbounded();
    let client = Client::new(client_tx);
    let proxy = client.create_proxy();

    let Args { port } = Args::parse();
    let addr = format!("127.0.0.1:{port}");
    let stream = TcpStream::connect(&addr)
        .with_context(|| format!("failed to open a TCP connection to address {addr}"))?;
    stream
        .set_nodelay(true)
        .context("failed to disable the Nagle algorithm")?;

    thread::scope(|s| {
        s.spawn(|| {
            let mut stream = BufWriter::new(&stream);
            for event in client_rx {
                if let Err(e) = bincode::serialize_into(&mut stream, &event) {
                    if let bincode::ErrorKind::Io(e) = &*e
                        && e.kind() == io::ErrorKind::BrokenPipe
                    {
                        break;
                    }
                    eprintln!("failed to serialize and write client event: {e:?}");
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
                            && let io::ErrorKind::UnexpectedEof | io::ErrorKind::ConnectionReset =
                                e.kind()
                        {
                            eprintln!("server disconnected");
                            break;
                        }
                        eprintln!("failed to read and deserialize server event: {e:?}");
                        continue;
                    }
                };
                if proxy.send_event(event).is_err() {
                    break;
                }
            }
        });

        client.run();

        stream
            .shutdown(Shutdown::Both)
            .context("failed to shutdown connection")
    })
}
