
.PHONY: all build install clean test lint fmt publish publish-dry-run

all: build

build:
	@cargo build --release

test:
	@cargo test --workspace

lint:
	@cargo clippy --workspace -- -D warnings

fmt:
	@cargo fmt --all

clean:
	@rm -rf target

install: build
	@cp target/release/reformat /usr/local/bin/
	@echo "reformat installed"

publish-dry-run:
	cargo publish --dry-run -p reformat-core
	cargo publish --dry-run -p reformat-plugins
	cargo publish --dry-run -p reformat

publish:
	cargo publish -p reformat-core
	@echo "waiting for crates.io to index reformat-core..."
	@sleep 15
	cargo publish -p reformat-plugins
	@echo "waiting for crates.io to index reformat-plugins..."
	@sleep 15
	cargo publish -p reformat
