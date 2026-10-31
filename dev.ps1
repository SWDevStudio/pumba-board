# Dev mode: rebuild + restart pumba on every change in src/, crates/ or Cargo.toml.
# Board data lives in .\sandbox\.pumba (created on first run, answer "y").
cargo watch -q -c -w src -w crates -w Cargo.toml -x "run -- sandbox"
