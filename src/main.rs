mod app;
mod client;
mod primitives;
mod server;
mod utils;

fn main() -> ! {
    pollster::block_on(app::App::new()).run()
}
