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
    pool::init(3);

    let id = ConnectionId::new();
    let (player_tx, transport_rx) = crossbeam_channel::unbounded();
    let (transport_tx, player_rx) = crossbeam_channel::unbounded();
    let (control_tx, control_rx) = crossbeam_channel::unbounded();
    let (chunk_tx, chunk_rx) = crossbeam_channel::unbounded();
    let (connection_tx, connection_rx) = crossbeam_channel::bounded(0);

    let client = Client::new(player_tx, control_rx, chunk_rx);
    let mut server = Server::new(connection_rx, player_rx);

    thread::spawn(move || server.run());

    thread::spawn(move || {
        for event in transport_rx {
            _ = transport_tx.send((id, event));
        }
    });

    connection_tx
        .send(ConnectionEvent::Opened(
            id,
            Connection {
                control_tx,
                chunk_tx,
            },
        ))
        .unwrap();

    client.run();
}
