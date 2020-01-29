VERSION := $(shell cat Cargo.toml | head -5 | grep version | sed -e 's/.*version\s*=\s*"\(.*\)"/\1/')
TARGET := $(shell uname -p)-$(shell uname -s)
EXE := minuteman
BUILD_DIR := target/release
CARGO_ARGS :=
ifeq ($(shell uname -s), Linux)
  CARGO_ARGS := $(CARGO_ARGS) --target x86_64-unknown-linux-musl
  BUILD_DIR := target/x86_64-unknown-linux-musl/release
endif

default: run-dev-coordinator

init:
	npm install .

check: webapp-dev
	cargo check $(CARGO_ARGS)

run-dev-coordinator: webapp-dev
	cargo run $(CARGO_ARGS)

run-dev-worker:
	cargo run $(CARGO_ARGS) -- "ws://localhost:5556"

webapp-dev:
	npm run "build:dev"

webapp-prod:
	npm run "build:prod"

build-dev: webapp-dev
	cargo build $(CARGO_ARGS)

build-prod: webapp-prod
	cargo build --release $(CARGO_ARGS)

run-prod-coordinator: webapp-prod
	cargo run --release $(CARGO_ARGS)

run-prod-worker:
	cargo run --release $(CARGO_ARGS) -- "ws://localhost:5556"

lint:
	cargo fmt --all -- --check
	cargo clippy $(CARGO_ARGS) -- -D 'clippy::all'

dist-clean:
	rm -rf dist

dist: dist-clean
	mkdir -p dist/$(EXE)/bin
	cp scripts/* dist/$(EXE)/
	cp -R systemd dist/$(EXE)/
	cp "$(BUILD_DIR)/$(EXE)" dist/$(EXE)/bin
	tar czf "dist/$(EXE)-$(TARGET)-$(VERSION).tar.gz" -C dist $(EXE)

clean: dist-clean
	cargo clean


.PHONY: default run-dev-coordinator run-dev-worker webapp-dev webapp-prod build-dev build-prod run-prod-coordinator run-prod-worker init lint clean dist-clean dist check
