build: pre
	cargo build

release: pre
	cargo build --release

pre:
	cargo deny --all-features check licenses
	cargo fmt --all -- --check
	cargo clippy --all

profile:
	RUSTFLAGS='-Cforce-frame-pointers' cargo build --release

test: pre
	cargo test

audit:
	cargo audit
