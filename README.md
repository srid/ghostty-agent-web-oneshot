# ghostty-agent-web

> [!WARNING]
> This is an AI-generated prototype built to explore what [ghostty-web](https://github.com/coder/ghostty-web) can provide as a browser-based terminal. Not production-ready — expect rough edges.

A web-based terminal dashboard for managing coding agent sessions. Spawn, attach, detach, and monitor multiple terminal sessions (shell, opencode, claude-code) from a browser using [ghostty-web](https://github.com/coder/ghostty-web) for terminal rendering.

<img width="1227" height="982" alt="image" src="https://github.com/user-attachments/assets/30b11937-d1b7-4291-aa51-c27f9ad408c4" />


## How it works

The server spawns real PTY processes (via `portable-pty`) and holds them in memory — each session is a persistent pseudo-terminal running your shell, opencode, or any TUI program. A WebSocket bridge streams raw PTY I/O between the server and the browser. On the browser side, [ghostty-web](https://github.com/coder/ghostty-web) — Ghostty's Zig-based terminal parser compiled to WebAssembly — renders the terminal output onto a canvas with full color, cursor movement, and alternate screen buffer support. Because the PTYs live on the server, sessions survive browser disconnects: close the tab, reopen it, and you're back where you left off with scrollback replayed. This replaces the need for tmux/zellij entirely — the server *is* the multiplexer, and the browser *is* the terminal.

## Architecture

```
Browser (Leptos + ghostty-web WASM)  ←WebSocket→  Rust Server (axum)  ←PTY→  shell / opencode / claude
```

- **Frontend**: Leptos CSR compiled to WASM. Terminal rendering via `ghostty-web` through a thin JS bridge. Tokyo Night dark theme.
- **Backend**: Rust server using axum for HTTP/WebSocket + `portable-pty` for PTY management. Scrollback buffer (100KB) enables reattach without losing context.
- **Common**: Shared Rust crate with types for session metadata, API requests, and WebSocket protocol messages.
- **Nix**: `flake.nix` using `flake-parts` + `rust-flake` (crane). Client WASM built with crane + wasm-bindgen. `nix run` starts the production server.

## Features

- **Session management**: Create, list, delete terminal sessions via REST API + web UI
- **Multiple agent variants**: Shell, opencode, claude-code — selectable from the UI
- **Attach/detach**: Sessions persist independently of browser connections. Close the tab, reopen, reattach to running sessions
- **Scrollback replay**: Reconnecting replays the last 100KB of output
- **Multi-client**: Multiple browser tabs can view the same session (spectator mode)
- **Resize**: Terminal auto-fits to browser viewport, resize propagates to PTY

## Usage

```bash
nix run github:srid/ghostty-agent-web-oneshot/rust
```

Then open http://localhost:7681 in your browser.

## Development

```bash
nix develop
just dev       # runs server (cargo watch) + client (trunk serve) in parallel
```

Open http://localhost:5173 (trunk proxies API/WS to the server on :7681).

### Available just recipes

```
just dev            # run server + client with hot reload
just server         # run server only (cargo watch)
just client         # run trunk dev server only
just build          # build everything for production
just build-client   # build client WASM only
just build-server   # build server only
```

## Tech Stack

| Component | Technology |
|-----------|-----------|
| Terminal emulation | [ghostty-web](https://github.com/coder/ghostty-web) (Ghostty's parser → WASM) |
| Frontend | [Leptos](https://leptos.dev) (Rust → WASM, CSR) |
| Backend | Rust, [axum](https://github.com/tokio-rs/axum), tokio |
| PTY management | [portable-pty](https://docs.rs/portable-pty) |
| Shared types | `common` crate (serde) |
| Packaging | Nix flakes, [rust-flake](https://github.com/juspay/rust-flake) (crane) |
| Theme | Tokyo Night |

## API

### HTTP

| Method | Endpoint | Description |
|--------|----------|-------------|
| `GET` | `/api/sessions` | List all sessions |
| `POST` | `/api/sessions` | Create session. Body: `{ variant?, command?, cwd?, cols?, rows? }` |
| `DELETE` | `/api/sessions/{id}` | Kill and remove session |
| `POST` | `/api/sessions/{id}/resize` | Resize PTY. Body: `{ cols, rows }` |

### WebSocket

Connect to `/ws/{sessionId}` for bidirectional PTY I/O.

- **Server → Client**: Raw binary PTY output, or JSON `{"type":"exit","exit_code":N,"signal":N}`
- **Client → Server**: Raw text (stdin), or JSON `{"type":"resize","cols":N,"rows":N}`

## Project Structure

```
├── Cargo.toml                # Workspace root
├── flake.nix                 # Nix build (flake-parts + rust-flake)
├── justfile                  # Dev workflow recipes
├── common/
│   └── src/lib.rs            # Shared types (SessionMeta, WsMessages, API types)
├── server/
│   └── src/main.rs           # axum + portable-pty + WebSocket session broker
└── client/
    ├── src/main.rs           # Leptos app (sidebar, session list, dialogs)
    ├── src/terminal.rs       # ghostty-web terminal component
    ├── js/ghostty-bridge.js  # Thin JS bridge for ghostty-web WASM interop
    └── style.css             # Tokyo Night theme
```
