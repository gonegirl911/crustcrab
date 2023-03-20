# Crustcrab
For a quick and dirty build, run:
```
cargo run --release
```
For a much faster and smaller binary at the cost of compilation speed, run:
```
RUSTFLAGS='-C target-cpu=native' cargo run --profile beast
```
