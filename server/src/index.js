import express from 'express';
import { createServer } from 'http';
import { WebSocketServer } from 'ws';
import * as pty from 'node-pty';
import { randomUUID } from 'crypto';
import fs from 'fs';
import path from 'path';
import os from 'os';
import { fileURLToPath } from 'url';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const PORT = process.env.PORT || 7681;
const SCROLLBACK_LIMIT = 100 * 1024; // 100KB
const CONFIG_DIR = path.join(os.homedir(), '.config', 'ghostty-agent-web');
const SESSIONS_FILE = path.join(CONFIG_DIR, 'sessions.json');

// --- Session store ---
const sessions = new Map(); // id -> { id, command, args, cwd, status, created, cols, rows, variant, ptyProcess, scrollback, scrollbackSize, clients }

function sessionMeta(s) {
  return {
    id: s.id,
    command: s.command,
    args: s.args,
    cwd: s.cwd,
    status: s.status,
    created: s.created,
    cols: s.cols,
    rows: s.rows,
    variant: s.variant,
  };
}

// --- Persistence ---
function ensureConfigDir() {
  fs.mkdirSync(CONFIG_DIR, { recursive: true });
}

function saveSessions() {
  ensureConfigDir();
  const data = [];
  for (const s of sessions.values()) {
    data.push(sessionMeta(s));
  }
  fs.writeFileSync(SESSIONS_FILE, JSON.stringify(data, null, 2));
}

function loadSessions() {
  // Clear stale sessions file on startup — old PTYs can't be reattached
  try {
    fs.unlinkSync(SESSIONS_FILE);
  } catch {
    // No file — fine
  }
}

// --- Scrollback buffer ---
function appendScrollback(session, data) {
  const buf = Buffer.from(data);
  session.scrollback.push(buf);
  session.scrollbackSize += buf.length;

  // Trim from front if over limit
  while (session.scrollbackSize > SCROLLBACK_LIMIT && session.scrollback.length > 0) {
    const removed = session.scrollback.shift();
    session.scrollbackSize -= removed.length;
  }
}

function getScrollbackBuffer(session) {
  return Buffer.concat(session.scrollback);
}

// --- Default shell ---
function defaultShell() {
  return process.env.SHELL || (os.platform() === 'win32' ? 'powershell.exe' : '/bin/sh');
}

// --- Express app ---
const app = express();
app.use(express.json());

// Static files from client dist
const clientDist = process.env.GHOSTTY_AGENT_WEB_CLIENT_DIST || path.resolve(__dirname, '../../client/dist');
if (fs.existsSync(clientDist)) {
  app.use(express.static(clientDist));
  // SPA fallback — serve index.html for non-API, non-WS routes
  app.get('*', (req, res, next) => {
    if (req.path.startsWith('/api/') || req.path.startsWith('/ws/')) return next();
    res.sendFile(path.join(clientDist, 'index.html'));
  });
}

// List sessions
app.get('/api/sessions', (_req, res) => {
  const list = [];
  for (const s of sessions.values()) {
    list.push(sessionMeta(s));
  }
  res.json(list);
});

// Resolve variant to command + args
function resolveVariant(variant, command, args) {
  if (command) return { command, args };
  switch (variant) {
    case 'opencode':
      return { command: 'opencode', args: [] };
    case 'claude-code':
      return { command: 'claude', args: [] };
    default:
      return { command: defaultShell(), args: [] };
  }
}

// Create session
app.post('/api/sessions', (req, res) => {
  const {
    command: rawCommand,
    args: rawArgs = [],
    cwd = os.homedir(),
    cols = 80,
    rows = 24,
    variant = 'shell',
  } = req.body || {};

  const { command, args } = resolveVariant(variant, rawCommand, rawArgs);
  const id = randomUUID();

  let ptyProcess;
  try {
    ptyProcess = pty.spawn(command, args, {
      name: 'xterm-256color',
      cols,
      rows,
      cwd,
      env: { ...process.env, TERM: 'xterm-256color' },
    });
  } catch (err) {
    return res.status(500).json({ error: `Failed to spawn PTY: ${err.message}` });
  }

  const session = {
    id,
    command,
    args,
    cwd,
    status: 'running',
    created: new Date().toISOString(),
    cols,
    rows,
    variant,
    ptyProcess,
    scrollback: [],
    scrollbackSize: 0,
    clients: new Set(),
  };

  sessions.set(id, session);

  ptyProcess.onData((data) => {
    appendScrollback(session, data);
    const buf = Buffer.from(data);
    for (const ws of session.clients) {
      if (ws.readyState === 1) {
        ws.send(buf);
      }
    }
  });

  ptyProcess.onExit(({ exitCode, signal }) => {
    session.status = 'exited';
    session.ptyProcess = null;
    saveSessions();

    const msg = JSON.stringify({ type: 'exit', exitCode, signal });
    for (const ws of session.clients) {
      if (ws.readyState === 1) {
        ws.send(msg);
      }
    }
  });

  saveSessions();
  res.status(201).json(sessionMeta(session));
});

// Delete session
app.delete('/api/sessions/:id', (req, res) => {
  const session = sessions.get(req.params.id);
  if (!session) return res.status(404).json({ error: 'Session not found' });

  if (session.ptyProcess) {
    try {
      session.ptyProcess.kill();
    } catch {
      // already dead
    }
  }

  // Close all websocket clients
  for (const ws of session.clients) {
    ws.close();
  }

  sessions.delete(req.params.id);
  saveSessions();
  res.json({ ok: true });
});

// Resize session
app.post('/api/sessions/:id/resize', (req, res) => {
  const session = sessions.get(req.params.id);
  if (!session) return res.status(404).json({ error: 'Session not found' });
  if (!session.ptyProcess) return res.status(400).json({ error: 'Session not running' });

  const { cols, rows } = req.body;
  if (!cols || !rows) return res.status(400).json({ error: 'cols and rows required' });

  session.ptyProcess.resize(cols, rows);
  session.cols = cols;
  session.rows = rows;
  res.json({ ok: true });
});

// --- HTTP + WebSocket server ---
const server = createServer(app);
const wss = new WebSocketServer({ noServer: true });

server.on('upgrade', (req, socket, head) => {
  // Parse /ws/:id
  const match = req.url.match(/^\/ws\/([^/?]+)/);
  if (!match) {
    socket.destroy();
    return;
  }

  const sessionId = match[1];
  const session = sessions.get(sessionId);
  if (!session) {
    socket.destroy();
    return;
  }

  wss.handleUpgrade(req, socket, head, (ws) => {
    wss.emit('connection', ws, req, session);
  });
});

wss.on('connection', (ws, _req, session) => {
  session.clients.add(ws);

  // Replay scrollback
  const scrollback = getScrollbackBuffer(session);
  if (scrollback.length > 0) {
    ws.send(scrollback);
  }

  // If session already exited, notify immediately
  if (session.status === 'exited' || session.status === 'dead') {
    ws.send(JSON.stringify({ type: 'exit', exitCode: null, signal: null }));
  }

  ws.on('message', (data, isBinary) => {
    if (!session.ptyProcess) return;

    // Try to parse as JSON for resize messages
    if (!isBinary) {
      try {
        const msg = JSON.parse(data.toString());
        if (msg.type === 'resize' && msg.cols && msg.rows) {
          session.ptyProcess.resize(msg.cols, msg.rows);
          session.cols = msg.cols;
          session.rows = msg.rows;
          return;
        }
      } catch {
        // Not JSON — treat as stdin input
      }
    }

    // Forward to PTY stdin
    session.ptyProcess.write(typeof data === 'string' ? data : data.toString());
  });

  ws.on('close', () => {
    session.clients.delete(ws);
  });
});

// --- Startup ---
loadSessions();

server.listen(PORT, () => {
  console.log(`ghostty-agent-web-server listening on port ${PORT}`);
});

// --- Graceful shutdown ---
function shutdown() {
  console.log('Shutting down...');
  for (const session of sessions.values()) {
    if (session.ptyProcess) {
      try {
        session.ptyProcess.kill();
      } catch {
        // ignore
      }
    }
  }
  server.close(() => process.exit(0));
  // Force exit after 3s
  setTimeout(() => process.exit(1), 3000);
}

process.on('SIGTERM', shutdown);
process.on('SIGINT', shutdown);
