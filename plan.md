# ghostty-agent-web — Project Phases

## Project Name (working)

`ghostty-agent-web` — a web-based terminal dashboard for attaching/detaching to Nix-managed coding agent sessions.

---

## Phase 0: Spike — "Can I attach ghostty-web to a PTY running opencode?" (1–2 days)

**Goal:** Prove the core technical assumption works. Zero UI polish, zero Nix integration. Just a terminal in a browser connected to a real opencode process.

**Deliverables:**
- A Node/Bun script that spawns a PTY running `opencode`
- A WebSocket server bridging the PTY's stdin/stdout to the browser
- A single HTML page using `ghostty-web` (`npm i ghostty-web`) to render the terminal
- Verify: opencode's TUI renders correctly, keyboard input works, resize works

**Key risks to validate:**
- Does ghostty-web handle opencode's TUI rendering (Bubble Tea / Ink)? Cursor movement, colors, alternate screen buffer?
- Does the WebSocket bridge handle raw binary PTY data correctly?
- What's the latency feel like?

**Non-goals:** No session management, no persistence, no Nix. Just `node server.js` and open `localhost:8080`.

**Demo:** Screen-share showing opencode running in a browser tab, indistinguishable from running in a native terminal.

---

## Phase 1: MVP — "Multi-session attach/detach dashboard" (1–2 weeks)

**Goal:** A demoable product. Launch multiple agent sessions, see them in a web dashboard, detach and reattach. This is the "wow" demo for the team.

### 1a. Session Broker

A lightweight server (Go, Rust, or Node — whichever you're fastest in) that manages PTY sessions:

- **Create session** — spawns a PTY process (initially hardcoded to `opencode`), assigns a session ID, starts recording output to a scrollback buffer
- **List sessions** — returns all sessions with metadata (ID, agent type, working directory, status, created time)
- **Attach** — opens a WebSocket to an existing session's PTY, replays recent scrollback so you see current state
- **Detach** — closes the WebSocket, PTY keeps running
- **Kill** — sends SIGTERM to the PTY process, cleans up

Session state persisted to a simple JSON file (`~/.config/ghostty-agent-web/sessions.json`) so the broker survives restarts.

### 1b. Web Dashboard

A single-page app (React or plain HTML+JS) with:

- **Session list sidebar** — shows all sessions with status indicators (running / idle / needs-input / exited)
- **Terminal pane** — ghostty-web instance that attaches to the selected session
- **New session button** — spawns a new opencode session (just picks a working directory for now)
- **Split view** — show 2–4 sessions side by side (grid layout)

### 1c. CLI Companion

A thin CLI that talks to the broker's HTTP API:

```
gaw new --agent opencode --dir ~/work/my-project    # create session
gaw ls                                                # list sessions
gaw attach <session-id>                               # attach in native terminal (via direct PTY passthrough)
gaw kill <session-id>                                 # kill session
gaw web                                               # open the web dashboard
```

`gaw` = ghostty-agent-web (working name).

### MVP Demo Script

1. `gaw new --dir ~/work/repo-A` — spawns opencode session 1
2. `gaw new --dir ~/work/repo-B` — spawns opencode session 2
3. `gaw web` — opens browser showing both sessions side by side
4. Interact with session 1 in the browser, give it a task
5. Close the browser tab
6. `gaw web` — reopen, both sessions still running, session 1 is mid-task
7. Show session 2 has been idle the whole time, attach and give it work

**This demo shows:** parallel sessions, persistence (detach/reattach), and web-based unified view.

---

## Phase 2: Nix Integration — "One command from juspay/AI" (1–2 weeks)

**Goal:** Wire everything through your existing Nix flake so it's a first-class citizen of `juspay/AI`.

### 2a. Nix Packaging

- Package the session broker + web dashboard as a Nix derivation
- Add to `juspay/AI` flake as new outputs:
  - `nix run github:juspay/AI#agent-web` — starts the dashboard
  - `nix run github:juspay/AI#agent-web-cli` — the `gaw` CLI

### 2b. Agent Variant Integration

- Session creation accepts a `--variant` flag matching existing flake outputs: `opencode-juspay-oneclick`, `opencode-oneclick`, etc.
- The broker spawns the correct `nix run` command for the chosen variant
- `.agents/` skills/config wired automatically via `nix-agent-wire` (already works for opencode variants)

### 2c. Multi-Agent Support

- When Claude Code support lands in `juspay/AI`, the dashboard supports it as another variant
- The session list shows which agent type each session is running
- Same attach/detach semantics regardless of agent

### Demo Addition

Same demo as Phase 1, but now:
- `nix run github:juspay/AI#agent-web` — one command, everything provisioned
- Show two sessions with different variants (e.g., `opencode-juspay` and `opencode-oneclick`)
- Emphasize: zero manual setup, all from the flake

---

## Phase 3: Git Worktree Management (1–2 weeks)

**Goal:** Each session automatically gets its own isolated git worktree. No more "which directory is which agent working in?"

### 3a. Worktree Lifecycle

- `gaw new --repo ~/work/payment-gateway --branch feat/auth-refactor` does:
  1. `git worktree add /tmp/gaw-sessions/<id> -b feat/auth-refactor`
  2. Spawns the agent PTY in that worktree directory
  3. Tracks the worktree path in session metadata
- On session kill: optionally prune the worktree (with confirmation)

### 3b. Git Status in Dashboard

- Dashboard sidebar shows per-session: branch name, ahead/behind, dirty file count
- Visual indicator when two sessions are on the same repo (potential merge conflict)
- "Diff preview" button showing uncommitted changes in a session's worktree

### 3c. Conflict Detection

- Before merging a session's branch, check for conflicts with other active sessions on the same repo
- Warning in the UI: "Session 3 (feat/auth-refactor) conflicts with Session 7 (feat/new-login) on 2 files"

### Demo Addition

- Show spawning 3 sessions on the same repo, each on different feature branches
- Dashboard shows all three with branch names and git status
- One session finishes, show its diff, highlight conflict warning with another

---

## Phase 4: Team Features — "Shared Agent Server" (2–3 weeks)

**Goal:** Run the broker on a shared dev server. Multiple developers attach/detach to sessions from their browsers. This is where it becomes a team tool, not just a personal one.

### 4a. Authentication & Multi-User

- Simple auth layer (initially just shared token, later integrate with Juspay's identity)
- Sessions tagged with owner
- Shared sessions: multiple people can watch the same session (read-only spectator mode)

### 4b. Remote Compute

- Broker runs on a beefy server, sessions use server's CPU/memory
- Developers connect from their laptops via browser — thin client model
- Nix ensures the environment on the server matches what developers expect locally

### 4c. Notifications

- Webhook/callback when a session finishes or errors
- Slack integration: "Session `feat/auth-refactor` on `payment-gateway` completed — [view diff]"
- Browser notifications when a background session needs input

### 4d. Session Templates

- Predefined session configs for common workflows:
  - "Refactor: run opencode on branch X with prompt Y"
  - "Review: run agent to analyze PR #123"
- Launchable from the dashboard with one click

---

## Phase 5: Operational Maturity (ongoing)

### 5a. Auto-PR Pipeline
- Session completes → auto-create PR with agent's diff + session transcript as comment
- Trigger CI, show results in dashboard

### 5b. Metrics & Analytics
- Track: tokens used, time-to-completion, CI pass rate per agent variant
- Surface insights: "opencode-juspay solves Haskell tasks 2x faster than opencode-oneclick"

### 5c. Agent Context Sharing
- Sessions on the same project share context (file embeddings, dependency graphs)
- New session on `payment-gateway` starts with warm context from prior sessions

### 5d. Resource Governance
- Per-session resource limits (CPU, memory, network)
- Org-wide quotas: max concurrent sessions per developer, max tokens per day

---

## Technical Decisions to Make Early

| Decision | Options | Recommendation |
|---|---|---|
| **Broker language** | Go, Rust, Node, Haskell | Go — fast to prototype, good PTY support (`os/exec` + `creack/pty`), easy to Nix-package, team can maintain |
| **Frontend framework** | React, Svelte, plain HTML | React — `ghostty-web` has xterm.js-compatible API, plenty of examples |
| **Session persistence** | JSON file, SQLite, none | JSON file for MVP, migrate to SQLite in Phase 4 |
| **IPC (CLI ↔ Broker)** | HTTP REST, Unix socket, gRPC | HTTP REST for MVP (simplest), Unix socket later for local perf |
| **Web bundler** | Vite, esbuild, none | Vite — fast, handles WASM well |

---

## What This Is NOT

- **Not another Mux.** Mux reimplements the agent loop. This wraps existing agents (opencode, claude-code) as-is.
- **Not another IDE.** No code editing, no file tree, no LSP. Just terminals.
- **Not a tmux replacement.** tmux multiplexes within a single terminal. This provides a web-native multi-terminal view with session persistence.

The value proposition is: **Nix-managed agent environments + git worktree isolation + web-based attach/detach = parallel AI coding with zero friction.**
