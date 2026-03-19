# Dev workflow — run inside `nix develop`

default:
    @just --list

# Install ghostty-web for local dev
install:
    cd client && npm install ghostty-web

# Run server with auto-restart
server:
    cargo watch -w server -w common -x 'run -p ghostty-agent-web-server'

# Run client dev server (trunk serves WASM + proxies API to :7681)
client:
    cd client && trunk serve

# Run both server and client in parallel (Ctrl+C kills both)
dev:
    trap 'kill 0' EXIT; just server & just client & wait

# Build client for production
build-client:
    cd client && trunk build --release

# Build server for production
build-server:
    cargo build -p ghostty-agent-web-server --release

# Build everything
build: build-client build-server
