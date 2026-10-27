use base::rayon;
use clap::Parser;
use server::net;
use std::net::SocketAddr;

#[derive(Parser)]
struct Args {
    #[arg(long, default_value = "0.0.0.0:8008")]
    event_addr: SocketAddr,
    #[arg(long, default_value = "0.0.0.0:8009")]
    chunk_addr: SocketAddr,
}

fn main() {
    env_logger::init();
    rayon::global::init(1);

    let Args {
        event_addr,
        chunk_addr,
    } = Parser::parse();

    let (connection_rx, player_rx) = net::listen(event_addr, chunk_addr);

    server::run(connection_rx, player_rx);
}
