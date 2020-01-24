default: run-dev-coordinator

init:
	npm install .

run-dev-coordinator: webapp-dev
	cargo run

run-dev-worker:
	cargo run -- "ws://localhost:5556"

webapp-dev:
	npm run "build:dev"

webapp-prod:
	npm run "build:prod"

build-dev: webapp-dev
	cargo build

build-prod: webapp-prod
	cargo build --release

run-prod-coordinator: webapp-prod
	cargo run --release

run-prod-worker: webapp-prod
	cargo run --release -- "ws://localhost:5556"

.PHONY: default run-dev-coordinator run-dev-worker webapp-dev webapp-prod build-dev build-prod run-prod-coordinator run-prod-worker init
