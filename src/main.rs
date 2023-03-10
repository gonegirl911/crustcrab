mod app;
mod client;
mod color;
mod server;

fn main() -> ! {
    pollster::block_on(app::App::new()).run()
}
