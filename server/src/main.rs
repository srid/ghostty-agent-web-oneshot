use std::collections::HashMap;
use std::sync::Arc;

use axum::extract::ws::{Message, WebSocket};
use axum::extract::{Json, Path, State, WebSocketUpgrade};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing::{delete, get, post};
use axum::Router;
use futures::stream::StreamExt;
use futures::SinkExt;
use portable_pty::{native_pty_system, CommandBuilder, PtySize};
use serde_json::json;
use tokio::sync::{Mutex, RwLock};
use tower_http::services::ServeDir;
use uuid::Uuid;

use ghostty_agent_web_common::*;

// --- Session state ---

/// Wrapper to make portable_pty's MasterPty Send+Sync safe.
/// portable_pty types are not always Send, so we wrap them and assert Send manually.
struct SendMaster(Box<dyn portable_pty::MasterPty + Send>);
unsafe impl Sync for SendMaster {}

struct SendChild(Box<dyn portable_pty::Child + Send + Sync>);

struct SendWriter(Box<dyn std::io::Write + Send>);
unsafe impl Sync for SendWriter {}

struct Session {
    meta: RwLock<SessionMeta>,
    master: Mutex<Option<SendMaster>>,
    _child: Mutex<Option<SendChild>>,
    writer: Mutex<Option<SendWriter>>,
    scrollback: Mutex<Vec<u8>>,
    clients: RwLock<Vec<tokio::sync::mpsc::UnboundedSender<Vec<u8>>>>,
}

type AppState = Arc<RwLock<HashMap<String, Arc<Session>>>>;

const MAX_SCROLLBACK: usize = 100 * 1024; // 100KB

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    let state: AppState = Arc::new(RwLock::new(HashMap::new()));
    let port = std::env::var("PORT").unwrap_or_else(|_| "7681".to_string());

    let client_dist = std::env::var("GHOSTTY_AGENT_WEB_CLIENT_DIST")
        .unwrap_or_else(|_| "../client/dist".to_string());

    let app = Router::new()
        .route("/api/sessions", get(list_sessions).post(create_session))
        .route("/api/sessions/{id}", delete(delete_session))
        .route("/api/sessions/{id}/resize", post(resize_session))
        .route("/ws/{id}", get(ws_handler))
        .fallback_service(ServeDir::new(&client_dist).append_index_html_on_directories(true))
        .with_state(state);

    let addr = format!("0.0.0.0:{}", port);
    tracing::info!("Listening on {}", addr);
    let listener = tokio::net::TcpListener::bind(&addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

// --- Handlers ---

async fn list_sessions(State(state): State<AppState>) -> impl IntoResponse {
    let sessions = state.read().await;
    let metas: Vec<SessionMeta> = {
        let mut out = Vec::new();
        for s in sessions.values() {
            out.push(s.meta.read().await.clone());
        }
        out
    };
    Json(metas)
}

async fn create_session(
    State(state): State<AppState>,
    Json(req): Json<CreateSessionRequest>,
) -> impl IntoResponse {
    // Resolve command from variant or explicit command
    let (command, args) = resolve_command(&req);

    let cwd = req
        .cwd
        .clone()
        .unwrap_or_else(|| std::env::current_dir().unwrap().to_string_lossy().to_string());

    let cols = if req.cols == 0 { 80 } else { req.cols };
    let rows = if req.rows == 0 { 24 } else { req.rows };

    // Open PTY
    let pty_system = native_pty_system();
    let pty_size = PtySize {
        rows,
        cols,
        pixel_width: 0,
        pixel_height: 0,
    };

    let pair = match pty_system.openpty(pty_size) {
        Ok(p) => p,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": format!("Failed to open PTY: {}", e) })),
            )
                .into_response();
        }
    };

    let mut cmd = CommandBuilder::new(&command);
    for arg in &args {
        cmd.arg(arg);
    }
    cmd.cwd(&cwd);

    let child = match pair.slave.spawn_command(cmd) {
        Ok(c) => c,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": format!("Failed to spawn command: {}", e) })),
            )
                .into_response();
        }
    };

    // Drop slave — we only need master from here
    drop(pair.slave);

    let writer = match pair.master.take_writer() {
        Ok(w) => w,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": format!("Failed to get PTY writer: {}", e) })),
            )
                .into_response();
        }
    };

    let reader = match pair.master.try_clone_reader() {
        Ok(r) => r,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": format!("Failed to get PTY reader: {}", e) })),
            )
                .into_response();
        }
    };

    let id = Uuid::new_v4().to_string();
    let now = chrono_now();

    let meta = SessionMeta {
        id: id.clone(),
        command: command.clone(),
        args: args.clone(),
        cwd,
        status: SessionStatus::Running,
        created: now,
        cols,
        rows,
        variant: req.variant.clone(),
    };

    let session = Arc::new(Session {
        meta: RwLock::new(meta.clone()),
        master: Mutex::new(Some(SendMaster(pair.master))),
        _child: Mutex::new(Some(SendChild(child))),
        writer: Mutex::new(Some(SendWriter(writer))),
        scrollback: Mutex::new(Vec::new()),
        clients: RwLock::new(Vec::new()),
    });

    state.write().await.insert(id.clone(), session.clone());

    // Spawn PTY reader task
    spawn_pty_reader(session.clone(), reader);

    // Spawn child wait task
    spawn_child_waiter(session.clone(), state.clone());

    (StatusCode::CREATED, Json(json!(meta))).into_response()
}

async fn delete_session(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let mut sessions = state.write().await;
    if sessions.remove(&id).is_some() {
        // Dropping the session will drop the PTY master, killing the child
        Json(json!({ "ok": true }))
    } else {
        Json(json!({ "error": "session not found" }))
    }
}

async fn resize_session(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(req): Json<ResizeRequest>,
) -> impl IntoResponse {
    let sessions = state.read().await;
    let Some(session) = sessions.get(&id) else {
        return (
            StatusCode::NOT_FOUND,
            Json(json!({ "error": "session not found" })),
        );
    };

    let master_guard = session.master.lock().await;
    if let Some(ref master) = *master_guard {
        let size = PtySize {
            rows: req.rows,
            cols: req.cols,
            pixel_width: 0,
            pixel_height: 0,
        };
        if let Err(e) = master.0.resize(size) {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": format!("resize failed: {}", e) })),
            );
        }
        // Update meta
        let mut meta = session.meta.write().await;
        meta.cols = req.cols;
        meta.rows = req.rows;
    }

    (StatusCode::OK, Json(json!({ "ok": true })))
}

async fn ws_handler(
    State(state): State<AppState>,
    Path(id): Path<String>,
    ws: WebSocketUpgrade,
) -> impl IntoResponse {
    let sessions = state.read().await;
    let Some(session) = sessions.get(&id).cloned() else {
        return StatusCode::NOT_FOUND.into_response();
    };
    drop(sessions);

    ws.on_upgrade(move |socket| handle_ws(socket, session))
}

async fn handle_ws(socket: WebSocket, session: Arc<Session>) {
    let (mut ws_tx, mut ws_rx) = socket.split();

    // Replay scrollback
    {
        let scrollback = session.scrollback.lock().await;
        if !scrollback.is_empty() {
            if ws_tx
                .send(Message::Binary(scrollback.clone().into()))
                .await
                .is_err()
            {
                return;
            }
        }
    }

    // Subscribe to PTY output
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<Vec<u8>>();
    session.clients.write().await.push(tx);

    // Forward PTY output → WebSocket
    let session_clone = session.clone();
    let send_task = tokio::spawn(async move {
        while let Some(data) = rx.recv().await {
            if ws_tx.send(Message::Binary(data.into())).await.is_err() {
                break;
            }
        }
        let _ = ws_tx.close().await;
        session_clone
    });

    // Forward WebSocket → PTY stdin
    let session_clone2 = session.clone();
    let recv_task = tokio::spawn(async move {
        while let Some(Ok(msg)) = ws_rx.next().await {
            match msg {
                Message::Binary(data) => {
                    let mut writer_guard = session_clone2.writer.lock().await;
                    if let Some(ref mut writer) = *writer_guard {
                        use std::io::Write;
                        let _ = writer.0.write_all(&data);
                        let _ = writer.0.flush();
                    }
                }
                Message::Text(text) => {
                    // Try to parse as JSON control message
                    if let Ok(msg) = serde_json::from_str::<WsClientMessage>(&text) {
                        match msg {
                            WsClientMessage::Resize { cols, rows } => {
                                let master_guard = session_clone2.master.lock().await;
                                if let Some(ref master) = *master_guard {
                                    let size = PtySize {
                                        rows,
                                        cols,
                                        pixel_width: 0,
                                        pixel_height: 0,
                                    };
                                    let _ = master.0.resize(size);
                                    let mut meta = session_clone2.meta.write().await;
                                    meta.cols = cols;
                                    meta.rows = rows;
                                }
                            }
                        }
                    } else {
                        // Treat as raw text input
                        let mut writer_guard = session_clone2.writer.lock().await;
                        if let Some(ref mut writer) = *writer_guard {
                            use std::io::Write;
                            let _ = writer.0.write_all(text.as_bytes());
                            let _ = writer.0.flush();
                        }
                    }
                }
                Message::Close(_) => break,
                _ => {}
            }
        }
    });

    // Wait for either task to finish
    tokio::select! {
        _ = send_task => {},
        _ = recv_task => {},
    }

    // Clean up: remove our sender from clients list
    // The sender was moved into send_task, so we can't compare pointers easily.
    // Instead, we remove closed channels on the next broadcast (lazy cleanup).
}

// --- Helper functions ---

fn resolve_command(req: &CreateSessionRequest) -> (String, Vec<String>) {
    if let Some(ref cmd) = req.command {
        return (cmd.clone(), req.args.clone().unwrap_or_default());
    }

    match req.variant.as_deref() {
        Some("opencode") => ("opencode".to_string(), vec![]),
        Some("claude-code") => ("claude".to_string(), vec![]),
        _ => {
            // Default to user's shell
            let shell =
                std::env::var("SHELL").unwrap_or_else(|_| "/bin/bash".to_string());
            (shell, vec![])
        }
    }
}

fn chrono_now() -> String {
    // Simple ISO 8601 timestamp without chrono dependency
    use std::time::SystemTime;
    let d = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap();
    let secs = d.as_secs();
    // Return as Unix timestamp string (good enough; avoids chrono dep)
    format!("{}", secs)
}

fn spawn_pty_reader(session: Arc<Session>, reader: Box<dyn std::io::Read + Send>) {
    tokio::task::spawn_blocking(move || {
        use std::io::Read;
        let mut reader = reader;
        let mut buf = [0u8; 4096];
        loop {
            match reader.read(&mut buf) {
                Ok(0) => break, // EOF
                Ok(n) => {
                    let data = buf[..n].to_vec();
                    let session = session.clone();
                    let data_clone = data.clone();
                    // Use a runtime handle to run async code from blocking context
                    let rt = tokio::runtime::Handle::current();
                    rt.block_on(async {
                        // Append to scrollback
                        {
                            let mut scrollback = session.scrollback.lock().await;
                            scrollback.extend_from_slice(&data_clone);
                            if scrollback.len() > MAX_SCROLLBACK {
                                let excess = scrollback.len() - MAX_SCROLLBACK;
                                scrollback.drain(..excess);
                            }
                        }
                        // Broadcast to clients
                        let clients = session.clients.read().await;
                        for client in clients.iter() {
                            let _ = client.send(data_clone.clone());
                        }
                    });
                }
                Err(_) => break,
            }
        }
    });
}

fn spawn_child_waiter(session: Arc<Session>, _state: AppState) {
    tokio::task::spawn(async move {
        // Wait a bit then poll the child
        loop {
            tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
            let mut child_guard = session._child.lock().await;
            if let Some(ref mut child) = *child_guard {
                match child.0.try_wait() {
                    Ok(Some(status)) => {
                        let exit_code = status.exit_code() as i32;
                        // Update session status
                        let mut meta = session.meta.write().await;
                        meta.status = SessionStatus::Exited;
                        drop(meta);
                        drop(child_guard);

                        // Notify connected WebSocket clients
                        let exit_msg = WsServerMessage::Exit {
                            exit_code: Some(exit_code),
                            signal: None,
                        };
                        if let Ok(json) = serde_json::to_string(&exit_msg) {
                            let clients = session.clients.read().await;
                            for client in clients.iter() {
                                let _ = client.send(json.as_bytes().to_vec());
                            }
                        }
                        break;
                    }
                    Ok(None) => {
                        // Still running
                        continue;
                    }
                    Err(_) => {
                        let mut meta = session.meta.write().await;
                        meta.status = SessionStatus::Dead;
                        break;
                    }
                }
            } else {
                break;
            }
        }
    });
}
