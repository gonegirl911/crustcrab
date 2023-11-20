mod app;
mod client;
mod server;
mod shared;

fn main() {
    pollster::block_on(app::App::new()).run();
}
