# Dev workflow — run inside `nix develop`

default:
    @just --list

# Install deps for both client and server
install:
    cd server && npm install && chmod +x node_modules/node-pty/prebuilds/darwin-arm64/spawn-helper
    cd client && npm install

# Run server with auto-restart on changes
server:
    cd server && node --watch src/index.js

# Run client dev server (Vite HMR, proxies API/WS to :7681)
client:
    cd client && npx vite --port 5173 --open

# Run both server and client in parallel (Ctrl+C kills both)
dev:
    trap 'kill 0' EXIT; cd server && node --watch src/index.js & cd client && npx vite --port 5173 --open & wait

# Build client for production
build:
    cd client && npx vite build

# Run production build (server serves client dist)
prod:
    just build
    cd server && node src/index.js
