use crustcrab::{
    client::Client,
    server::{
        Server,
        connection::{Connection, ConnectionEvent, ConnectionId},
    },
    shared::pool,
};
use std::thread;

fn main() {
    env_logger::init();
    pool::init(3);

    let (connection_tx, connection_rx) = crossbeam_channel::bounded(0);
    let (player_tx, transport_rx) = crossbeam_channel::unbounded();
    let (transport_tx, player_rx) = crossbeam_channel::unbounded();
    let (control_tx, control_rx) = crossbeam_channel::unbounded();
    let (chunk_tx, chunk_rx) = crossbeam_channel::unbounded();

    let mut server = Server::new(connection_rx, player_rx);
    thread::spawn(move || server.run());

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

    thread::spawn(move || {
        for event in transport_rx {
            _ = transport_tx.send((id, event));
        }
    });

    let client = Client::new(player_tx, control_rx, chunk_rx);
    client.run();
}
