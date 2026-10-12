.PHONY: ci fmt lint test build install
ci:      ; ./scripts/ci.sh
fmt:     ; cargo fmt --all
lint:    ; cargo clippy --workspace --all-targets -- -D warnings
test:    ; cargo test --workspace
build:   ; cargo build --release -p aether
install: ; cargo install --path crates/aether --locked
