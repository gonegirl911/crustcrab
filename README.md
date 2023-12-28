# Crustcrab

For a quick and dirty build, run:

```sh
cargo run --release
```

For a much faster and smaller binary at the cost of compilation speed, run:

```sh
RUSTFLAGS='-C target-cpu=native' cargo run --profile lto
```
