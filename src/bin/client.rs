use clap::Parser;
use crustcrab::client::{Client, net};
use std::net::SocketAddr;

#[derive(Parser)]
struct Args {
    #[arg(long, default_value = "127.0.0.1:8008")]
    event_addr: SocketAddr,
    #[arg(long, default_value = "127.0.0.1:8009")]
    chunk_addr: SocketAddr,
}

fn main() {
    env_logger::init();

    let Args {
        event_addr,
        chunk_addr,
    } = Parser::parse();

    let (player_tx, control_rx, chunk_rx, disconnect_rx) = net::connect(event_addr, chunk_addr);

    let client = Client::new(player_tx, control_rx, chunk_rx, disconnect_rx);
    client.run();
}
