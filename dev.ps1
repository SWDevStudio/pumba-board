# Dev mode: rebuild + restart pumba on every change in src/ or Cargo.toml.
# Board data lives in .\sandbox\.pumba (created on first run, answer "y").
cargo watch -q -c -w src -w Cargo.toml -x "run -- sandbox"
