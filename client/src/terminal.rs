use leptos::prelude::*;
use wasm_bindgen::prelude::*;
use web_sys::{WebSocket, MessageEvent, BinaryType};
use ghostty_agent_web_common::*;

#[wasm_bindgen(module = "/js/ghostty-bridge.js")]
extern "C" {
    type GhosttyTerminal;

    #[wasm_bindgen(constructor)]
    fn new() -> GhosttyTerminal;

    #[wasm_bindgen(method, catch)]
    async fn init(this: &GhosttyTerminal) -> Result<JsValue, JsValue>;

    #[wasm_bindgen(method)]
    fn open(this: &GhosttyTerminal, element: &web_sys::HtmlElement);

    #[wasm_bindgen(method, js_name = "writeBytes")]
    fn write_bytes(this: &GhosttyTerminal, data: &js_sys::Uint8Array);

    #[wasm_bindgen(method, js_name = "writeString")]
    fn write_string(this: &GhosttyTerminal, data: &str);

    #[wasm_bindgen(method)]
    fn resize(this: &GhosttyTerminal, cols: u16, rows: u16);

    #[wasm_bindgen(method)]
    fn dispose(this: &GhosttyTerminal);

    #[wasm_bindgen(method, js_name = "onData")]
    fn on_data(this: &GhosttyTerminal, callback: &Closure<dyn FnMut(String)>);

    #[wasm_bindgen(method, js_name = "onResize")]
    fn on_resize(this: &GhosttyTerminal, callback: &Closure<dyn FnMut(u16, u16)>);

    #[wasm_bindgen(method, js_name = "fitToContainer")]
    fn fit_to_container(this: &GhosttyTerminal) -> JsValue;
}

#[component]
pub fn TerminalView(session_id: String) -> impl IntoView {
    let container_ref = NodeRef::<leptos::html::Div>::new();
    let session_id_clone = session_id.clone();

    Effect::new(move |_| {
        let session_id = session_id_clone.clone();
        let container = container_ref.get();
        if container.is_none() { return; }
        let container: web_sys::HtmlElement = container.unwrap().into();

        wasm_bindgen_futures::spawn_local(async move {
            let term = GhosttyTerminal::new();
            let _ = term.init().await;
            term.open(&container);

            // Fit to container and get dimensions
            let size = term.fit_to_container();
            let (cols, rows) = if !size.is_null() && !size.is_undefined() {
                let cols = js_sys::Reflect::get(&size, &"cols".into())
                    .ok().and_then(|v| v.as_f64()).unwrap_or(80.0) as u16;
                let rows = js_sys::Reflect::get(&size, &"rows".into())
                    .ok().and_then(|v| v.as_f64()).unwrap_or(24.0) as u16;
                (cols, rows)
            } else {
                (80, 24)
            };

            // Connect WebSocket
            let protocol = if web_sys::window().unwrap().location().protocol().unwrap() == "https:" { "wss:" } else { "ws:" };
            let host = web_sys::window().unwrap().location().host().unwrap();
            let ws_url = format!("{}//{}/ws/{}", protocol, host, session_id);
            let ws = WebSocket::new(&ws_url).unwrap();
            ws.set_binary_type(BinaryType::Arraybuffer);

            // On WS open: send initial resize
            let ws_clone = ws.clone();
            let onopen = Closure::wrap(Box::new(move |_: JsValue| {
                let msg = serde_json::to_string(&WsClientMessage::Resize { cols, rows }).unwrap();
                let _ = ws_clone.send_with_str(&msg);
            }) as Box<dyn FnMut(JsValue)>);
            ws.set_onopen(Some(onopen.as_ref().unchecked_ref()));
            onopen.forget();

            // On WS message: write to terminal
            // We need to call term methods from inside the closure, but term isn't Clone.
            // Store it in an Rc so we can share it.
            let term = std::rc::Rc::new(term);
            let term_for_msg = term.clone();
            let onmessage = Closure::wrap(Box::new(move |e: MessageEvent| {
                if let Ok(buf) = e.data().dyn_into::<js_sys::ArrayBuffer>() {
                    let array = js_sys::Uint8Array::new(&buf);
                    term_for_msg.write_bytes(&array);
                } else if let Some(text) = e.data().as_string() {
                    if let Ok(msg) = serde_json::from_str::<WsServerMessage>(&text) {
                        match msg {
                            WsServerMessage::Exit { .. } => {
                                term_for_msg.write_string("\r\n\x1b[90m[session exited]\x1b[0m\r\n");
                            }
                        }
                    } else {
                        term_for_msg.write_string(&text);
                    }
                }
            }) as Box<dyn FnMut(MessageEvent)>);
            ws.set_onmessage(Some(onmessage.as_ref().unchecked_ref()));
            onmessage.forget();

            // Terminal input -> WS
            let ws_for_data = ws.clone();
            let on_data_cb = Closure::wrap(Box::new(move |data: String| {
                let _ = ws_for_data.send_with_str(&data);
            }) as Box<dyn FnMut(String)>);
            term.on_data(&on_data_cb);
            on_data_cb.forget();

            // Terminal resize -> WS
            let ws_for_resize = ws.clone();
            let on_resize_cb = Closure::wrap(Box::new(move |cols: u16, rows: u16| {
                let msg = serde_json::to_string(&WsClientMessage::Resize { cols, rows }).unwrap();
                let _ = ws_for_resize.send_with_str(&msg);
            }) as Box<dyn FnMut(u16, u16)>);
            term.on_resize(&on_resize_cb);
            on_resize_cb.forget();

            // TODO: ResizeObserver for container resize
            // TODO: cleanup on unmount
        });
    });

    view! {
        <div node_ref=container_ref class="terminal-container"></div>
    }
}
