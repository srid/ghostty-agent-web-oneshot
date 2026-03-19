use leptos::prelude::*;
use ghostty_agent_web_common::*;

mod terminal;

fn main() {
    console_error_panic_hook::set_once();
    leptos::mount::mount_to_body(App);
}

#[component]
fn App() -> impl IntoView {
    let (sessions, set_sessions) = signal(Vec::<SessionMeta>::new());
    let (selected_id, set_selected_id) = signal(Option::<String>::None);
    let (show_dialog, set_show_dialog) = signal(false);

    // Poll sessions every 3 seconds
    wasm_bindgen_futures::spawn_local(async move {
        loop {
            match fetch_sessions().await {
                Ok(s) => set_sessions.set(s),
                Err(e) => web_sys::console::error_1(&format!("fetch error: {e}").into()),
            }
            gloo_timers::future::sleep(std::time::Duration::from_secs(3)).await;
        }
    });

    let create_session = move |req: CreateSessionRequest| {
        wasm_bindgen_futures::spawn_local(async move {
            if let Ok(session) = post_create_session(req).await {
                set_selected_id.set(Some(session.id));
            }
        });
        set_show_dialog.set(false);
    };

    let delete_session = move |id: String| {
        wasm_bindgen_futures::spawn_local(async move {
            let _ = delete_session_api(&id).await;
        });
    };

    view! {
        <div class="app">
            <aside class="sidebar">
                <div class="sidebar-header">
                    <h1>"ghostty-agent-web"</h1>
                </div>
                <button class="new-session-btn" on:click=move |_| set_show_dialog.set(true)>
                    "+ New Session"
                </button>
                <div class="session-list">
                    <For
                        each=move || sessions.get()
                        key=|s| s.id.clone()
                        children=move |session| {
                            let id = session.id.clone();
                            let id2 = session.id.clone();
                            let id3 = session.id.clone();
                            let short_id = session.id[..8].to_string();
                            let command = session.command.clone();
                            let cwd_base = session.cwd.split('/').last().unwrap_or("").to_string();
                            let is_running = session.status == SessionStatus::Running;
                            let is_selected = move || selected_id.get().as_deref() == Some(&id);

                            view! {
                                <div
                                    class=move || if is_selected() { "session-item selected" } else { "session-item" }
                                    on:click=move |_| set_selected_id.set(Some(id2.clone()))
                                >
                                    <div class="session-info">
                                        <span class="session-id">{short_id.clone()}</span>
                                        <span class=move || if is_running { "status-badge running" } else { "status-badge exited" }></span>
                                    </div>
                                    <div class="session-meta">
                                        <span class="session-command">{command.clone()}</span>
                                        <span class="session-dir">{cwd_base.clone()}</span>
                                    </div>
                                    <button class="delete-btn" on:click=move |e: web_sys::MouseEvent| {
                                        e.stop_propagation();
                                        delete_session(id3.clone());
                                    }>"×"</button>
                                </div>
                            }
                        }
                    />
                </div>
            </aside>

            <main class="terminal-area">
                {move || match selected_id.get() {
                    Some(id) => view! { <terminal::TerminalView session_id=id /> }.into_any(),
                    None => view! { <div class="placeholder">"Select or create a session"</div> }.into_any(),
                }}
            </main>

            <Show when=move || show_dialog.get()>
                <NewSessionDialog
                    on_create=create_session
                    on_cancel=move || set_show_dialog.set(false)
                />
            </Show>
        </div>
    }
}

#[component]
fn NewSessionDialog(
    on_create: impl Fn(CreateSessionRequest) + 'static,
    on_cancel: impl Fn() + 'static,
) -> impl IntoView {
    let (variant, set_variant) = signal("shell".to_string());
    let (command, set_command) = signal(String::new());
    let (cwd, set_cwd) = signal(String::new());

    let on_submit = move |e: web_sys::SubmitEvent| {
        e.prevent_default();
        let req = CreateSessionRequest {
            command: if command.get().is_empty() { None } else { Some(command.get()) },
            cwd: if cwd.get().is_empty() { None } else { Some(cwd.get()) },
            variant: Some(variant.get()),
            ..Default::default()
        };
        on_create(req);
    };

    let on_cancel = std::rc::Rc::new(on_cancel);
    let on_cancel2 = on_cancel.clone();

    view! {
        <div class="dialog-overlay" on:click=move |_| on_cancel()>
            <div class="dialog" on:click=move |e: web_sys::MouseEvent| e.stop_propagation()>
                <h2>"New Session"</h2>
                <form on:submit=on_submit>
                    <label>
                        "Variant"
                        <select on:change=move |e| set_variant.set(event_target_value(&e))>
                            <option value="shell">"shell"</option>
                            <option value="opencode">"opencode"</option>
                            <option value="claude-code">"claude-code"</option>
                        </select>
                    </label>
                    <label>
                        "Command"
                        <input type="text"
                            placeholder="Leave empty for default shell"
                            on:input=move |e| set_command.set(event_target_value(&e))
                        />
                    </label>
                    <label>
                        "Working Directory"
                        <input type="text"
                            placeholder="Default: home directory"
                            on:input=move |e| set_cwd.set(event_target_value(&e))
                        />
                    </label>
                    <div class="dialog-actions">
                        <button type="button" class="cancel-btn" on:click=move |_| on_cancel2()>"Cancel"</button>
                        <button type="submit" class="create-btn">"Create"</button>
                    </div>
                </form>
            </div>
        </div>
    }
}

// --- API helpers ---
async fn fetch_sessions() -> Result<Vec<SessionMeta>, String> {
    let resp = gloo_net::http::Request::get("/api/sessions")
        .send()
        .await
        .map_err(|e| e.to_string())?;
    resp.json().await.map_err(|e| e.to_string())
}

async fn post_create_session(req: CreateSessionRequest) -> Result<SessionMeta, String> {
    let resp = gloo_net::http::Request::post("/api/sessions")
        .json(&req)
        .map_err(|e| e.to_string())?
        .send()
        .await
        .map_err(|e| e.to_string())?;
    resp.json().await.map_err(|e| e.to_string())
}

async fn delete_session_api(id: &str) -> Result<(), String> {
    gloo_net::http::Request::delete(&format!("/api/sessions/{}", id))
        .send()
        .await
        .map_err(|e| e.to_string())?;
    Ok(())
}
