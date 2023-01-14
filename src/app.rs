use crate::{client::Client, server::Server};
use mimalloc::MiMalloc;
use std::thread;

#[global_allocator]
static GLOBAL: MiMalloc = MiMalloc;

pub struct App {
    client: Client,
    server: Server,
}

impl App {
    pub async fn new() -> Self {
        let (client_tx, client_rx) = flume::unbounded();
        let (server_tx, server_rx) = flume::unbounded();
        Self {
            client: Client::new(client_tx, server_rx).await,
            server: Server::new(server_tx, client_rx),
        }
    }

    pub fn run(self) -> ! {
        thread::spawn(move || self.server.run());
        self.client.run()
    }
}
