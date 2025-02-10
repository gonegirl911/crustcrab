use crustcrab::{client::Client, server::Server};
use std::thread;

fn main() {
    let (client_tx, client_rx) = crossbeam_channel::unbounded();
    let client = Client::new(client_tx);
    let server = Server::new(client.create_proxy(), client_rx);
    thread::spawn(move || server.run());
    client.run();
}
