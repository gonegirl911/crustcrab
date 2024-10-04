#![feature(array_chunks)]
#![feature(generic_arg_infer)]
#![feature(impl_trait_in_assoc_type)]
#![feature(int_roundings)]
#![feature(let_chains)]
#![feature(trait_alias)]

mod app;
mod client;
mod server;
mod shared;

fn main() {
    app::App::new().run();
}
