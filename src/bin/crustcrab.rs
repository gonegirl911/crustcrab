use crustcrab::{
    client::Client,
    server::{Server, ServerSender},
};
use std::thread;

fn main() {
    let (client_tx, client_rx) = crossbeam_channel::unbounded();
    let client = Client::default();
    let (proxy, server_rx) = client.create_proxy();
    let mut server = Server::new(ServerSender::Proxy(proxy), client_rx);
    thread::spawn(move || server.run());
    client.run(client_tx, server_rx);
}
