# ghostty-agent-web

> [!WARNING]
> This is an AI-generated prototype built to explore what [ghostty-web](https://github.com/coder/ghostty-web) can provide as a browser-based terminal. Not production-ready — expect rough edges.

A web-based terminal dashboard for managing coding agent sessions. Spawn, attach, detach, and monitor multiple terminal sessions (shell, opencode, claude-code) from a browser using [ghostty-web](https://github.com/coder/ghostty-web) for terminal rendering.

<img width="1227" height="982" alt="image" src="https://github.com/user-attachments/assets/30b11937-d1b7-4291-aa51-c27f9ad408c4" />


## Architecture

```
Browser (ghostty-web)  ←WebSocket→  Node.js Server  ←PTY→  shell / opencode / claude
```

- **Frontend**: React + Vite SPA. Terminal rendering via `ghostty-web` — Ghostty's Zig-based VT100 parser compiled to WebAssembly (~400KB). Provides native-quality terminal emulation in the browser (colors, alternate screen buffer, cursor movement, etc).
- **Backend**: Express server managing PTY sessions via `node-pty`. Each session gets its own pseudo-terminal. WebSocket bridge forwards raw PTY I/O to connected browser clients. Scrollback buffer (100KB) enables reattach without losing context.
- **Nix**: Single `flake.nix` builds everything — client static assets via `buildNpmPackage` + Vite, server with native `node-pty` addon. `nix run` starts the production server.

## Features

- **Session management**: Create, list, delete terminal sessions via REST API + web UI
- **Multiple agent variants**: Shell, opencode, claude-code — selectable from the UI
- **Attach/detach**: Sessions persist independently of browser connections. Close the tab, reopen, reattach to running sessions
- **Scrollback replay**: Reconnecting replays the last 100KB of output
- **Multi-client**: Multiple browser tabs can view the same session (spectator mode)
- **Resize**: Terminal auto-fits to browser viewport, resize propagates to PTY

## Usage

```bash
nix run github:srid/ghostty-agent-web-oneshot
```

Then open http://localhost:7681 in your browser.

## Development

### Hot reload

```bash
nix develop
just install   # npm install for both client and server
just dev       # runs server (node --watch) + Vite dev server (HMR) in parallel
```

Open http://localhost:5173 (Vite proxies API/WS to the server on :7681).

### Available just recipes

```
just install   # install npm deps
just dev       # run server + client with hot reload
just server    # run server only (auto-restart on changes)
just client    # run Vite dev server only
just build     # production client build
just prod      # build + run production server
```

## Tech Stack

| Component | Technology |
|-----------|-----------|
| Terminal emulation | [ghostty-web](https://github.com/coder/ghostty-web) (Ghostty's parser → WASM) |
| Frontend | React 19, Vite 6 |
| Backend | Node.js, Express, WebSocket (`ws`) |
| PTY management | [node-pty](https://github.com/nickel-org/node-pty) |
| Packaging | Nix flakes, `buildNpmPackage` |
| Theme | Tokyo Night |

## API

### HTTP

| Method | Endpoint | Description |
|--------|----------|-------------|
| `GET` | `/api/sessions` | List all sessions |
| `POST` | `/api/sessions` | Create session. Body: `{ variant?, command?, cwd?, cols?, rows? }` |
| `DELETE` | `/api/sessions/:id` | Kill and remove session |
| `POST` | `/api/sessions/:id/resize` | Resize PTY. Body: `{ cols, rows }` |

### WebSocket

Connect to `/ws/:sessionId` for bidirectional PTY I/O.

- **Server → Client**: Raw binary PTY output, or JSON `{"type":"exit","exitCode":N,"signal":N}`
- **Client → Server**: Raw text (stdin), or JSON `{"type":"resize","cols":N,"rows":N}`

## Project Structure

```
├── flake.nix                 # Nix build: client + server + wrapper
├── justfile                  # Dev workflow recipes
├── server/
│   ├── package.json
│   └── src/index.js          # Express + WebSocket + node-pty session broker
└── client/
    ├── package.json
    ├── vite.config.js
    └── src/
        ├── App.jsx           # Dashboard layout + session list
        ├── TerminalView.jsx  # ghostty-web terminal + WebSocket bridge
        ├── NewSessionDialog.jsx
        └── App.css           # Tokyo Night theme
```
