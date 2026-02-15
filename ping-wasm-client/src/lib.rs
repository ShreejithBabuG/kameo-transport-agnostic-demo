use wasm_bindgen::prelude::*;
use web_sys::{WebSocket, MessageEvent, ErrorEvent};
use ping_common::{Ping, Pong};

/// WebAssembly ping client
/// Uses the same Ping/Pong message format as the CLI client
#[wasm_bindgen]
pub struct WasmPingClient {
    ws: WebSocket,
    ping_count: u64,
}

#[wasm_bindgen]
impl WasmPingClient {
    /// Create new WebSocket connection to the Kameo server
    #[wasm_bindgen(constructor)]
    pub fn new() -> Result<WasmPingClient, JsValue> {
        // Set up panic handler for better error messages
        console_error_panic_hook::set_once();
        
        // Connect to WebSocket endpoint
        let ws = WebSocket::new("ws://localhost:8080/ws")?;
        
        // Set up connection handler
        let onopen = Closure::wrap(Box::new(move |_| {
            web_sys::console::log_1(&"Connected to Kameo server!".into());
        }) as Box<dyn FnMut(JsValue)>);
        ws.set_onopen(Some(onopen.as_ref().unchecked_ref()));
        onopen.forget();
        
        // Set up message handler - receives Pong responses
        let onmessage = Closure::wrap(Box::new(move |e: MessageEvent| {
            if let Ok(txt) = e.data().dyn_into::<js_sys::JsString>() {
                let s = String::from(txt);
                if let Ok(pong) = serde_json::from_str::<Pong>(&s) {
                    let msg = format!("PONG #{}: {} (total: {})", 
                        pong.sequence, pong.message, pong.total_pings);
                    web_sys::console::log_1(&msg.into());
                }
            }
        }) as Box<dyn FnMut(MessageEvent)>);
        ws.set_onmessage(Some(onmessage.as_ref().unchecked_ref()));
        onmessage.forget();
        
        // Set up error handler
        let onerror = Closure::wrap(Box::new(move |_: ErrorEvent| {
            web_sys::console::log_1(&"WebSocket Error".into());
        }) as Box<dyn FnMut(ErrorEvent)>);
        ws.set_onerror(Some(onerror.as_ref().unchecked_ref()));
        onerror.forget();
        
        Ok(WasmPingClient { ws, ping_count: 0 })
    }
    
    /// Send a ping message to the server
    /// Uses the same message format as the CLI client
    pub fn send_ping(&mut self) -> Result<(), JsValue> {
        self.ping_count += 1;
        
        // Create Ping message (same format as CLI)
        let ping = Ping {
            message: format!("Hello from Wasm #{}", self.ping_count),
            sequence: self.ping_count,
        };
        
        // Serialize to JSON and send
        let json = serde_json::to_string(&ping)
            .map_err(|e| JsValue::from_str(&e.to_string()))?;
        
        web_sys::console::log_1(&format!("Sending PING #{}", self.ping_count).into());
        self.ws.send_with_str(&json)?;
        Ok(())
    }
}