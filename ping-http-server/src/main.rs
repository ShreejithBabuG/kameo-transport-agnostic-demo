use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        State,
    },
    response::{Html, Response},
    routing::get,
    Router,
};
use kameo::prelude::*;
use ping_common::{Ping, PingActor};
use std::{net::SocketAddr, sync::Arc};
use tower_http::services::ServeDir;
use tracing::{info, warn, error};
use tracing_subscriber::EnvFilter;

type SharedActor = Arc<ActorRef<PingActor>>;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")))
        .init();

    info!("Starting HTTP Server with WebSocket support...");

    // Spawn the PingActor (same actor used in CLI version)
    let ping_actor = PingActor::spawn(PingActor { ping_count: 0 });
    let shared_actor = Arc::new(ping_actor);
    
    info!("PingActor spawned successfully");

    // Build router with HTTP and WebSocket endpoints
    let app = Router::new()
        .route("/", get(serve_index))
        .route("/ws", get(websocket_handler))
        .nest_service("/static", ServeDir::new("ping-http-server/static"))
        .with_state(shared_actor);

    let addr = SocketAddr::from(([127, 0, 0, 1], 8080));
    info!("HTTP Server listening on: http://{}", addr);
    info!("WebSocket endpoint available at: ws://{}/ws", addr);

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}

// Serve the main HTML page with embedded JavaScript client
async fn serve_index() -> Html<&'static str> {
    Html(r#"<!DOCTYPE html>
<html>
<head>
    <title>Kameo WebSocket Ping</title>
    <style>
        body { font-family: Arial; max-width: 800px; margin: 50px auto; padding: 20px; }
        button { padding: 10px 20px; font-size: 16px; margin: 5px; cursor: pointer; }
        #output { background: #f4f4f4; padding: 15px; border-radius: 5px; height: 400px; overflow-y: auto; font-family: monospace; }
    </style>
</head>
<body>
    <h1>Kameo WebSocket Ping (JavaScript)</h1>
    <p><strong>Same PingActor handling messages from browser!</strong></p>
    <button id="connect">Connect</button>
    <button id="ping" disabled>Send Ping</button>
    <button id="ping10" disabled>Send 10 Pings</button>
    <pre id="output"></pre>
    
    <script>
        let ws = null;
        let pingCount = 0;
        const output = document.getElementById('output');
        
        function log(msg) {
            output.textContent += msg + '\n';
            output.scrollTop = output.scrollHeight;
        }
        
        document.getElementById('connect').onclick = () => {
            ws = new WebSocket('ws://localhost:8080/ws');
            ws.onopen = () => {
                log('Connected');
                document.getElementById('connect').disabled = true;
                document.getElementById('ping').disabled = false;
                document.getElementById('ping10').disabled = false;
            };
            ws.onmessage = (e) => {
                const pong = JSON.parse(e.data);
                log(`PONG #${pong.sequence}: ${pong.message} (total: ${pong.total_pings})`);
            };
            ws.onclose = () => {
                log('Disconnected');
                document.getElementById('connect').disabled = false;
                document.getElementById('ping').disabled = true;
                document.getElementById('ping10').disabled = true;
            };
        };
        
        document.getElementById('ping').onclick = () => {
            pingCount++;
            const ping = { message: `Hello from browser #${pingCount}`, sequence: pingCount };
            ws.send(JSON.stringify(ping));
            log(`PING #${pingCount}`);
        };
        
        document.getElementById('ping10').onclick = async () => {
            for (let i = 0; i < 10; i++) {
                pingCount++;
                const ping = { message: `Hello from browser #${pingCount}`, sequence: pingCount };
                ws.send(JSON.stringify(ping));
                log(`PING #${pingCount}`);
                await new Promise(r => setTimeout(r, 500));
            }
        };
    </script>
</body>
</html>"#)
}

// Handle WebSocket upgrade requests
async fn websocket_handler(ws: WebSocketUpgrade, State(actor): State<SharedActor>) -> Response {
    ws.on_upgrade(|socket| handle_socket(socket, actor))
}

// Handle individual WebSocket connections
// Bridges WebSocket messages to Kameo actor messages
async fn handle_socket(mut socket: WebSocket, actor: SharedActor) {
    info!("WebSocket client connected");

    while let Some(msg) = socket.recv().await {
        match msg {
            Ok(Message::Text(text)) => {
                // Deserialize JSON ping message
                match serde_json::from_str::<Ping>(&text) {
                    Ok(ping) => {
                        info!("Received PING #{}", ping.sequence);
                        
                        // Forward to PingActor (same actor as CLI uses!)
                        match actor.ask(ping).await {
                            Ok(pong_reply) => {
                                let pong = pong_reply.0;
                                info!("Sending PONG #{}", pong.sequence);
                                
                                // Serialize and send response
                                let json = serde_json::to_string(&pong).unwrap();
                                if socket.send(Message::Text(json)).await.is_err() {
                                    break;
                                }
                            }
                            Err(e) => error!("Actor error: {}", e),
                        }
                    }
                    Err(e) => warn!("Parse error: {}", e),
                }
            }
            Ok(Message::Close(_)) => {
                info!("Client closed connection");
                break;
            }
            Err(e) => {
                error!("WebSocket error: {}", e);
                break;
            }
            _ => {}
        }
    }
}
