# Kameo Transport-Agnostic Actor Demonstration

A proof-of-concept implementation demonstrating that Kameo actor code can remain completely transport-agnostic, handling messages identically across WebSocket and TCP/libp2p protocols.

## Project Overview

This project validates that actor-based message handling logic can be written once and deployed across multiple transport layers without modification. The same `PingActor` implementation processes messages from:

- **Browser clients** (JavaScript and WebAssembly via WebSocket)
- **CLI clients** (Rust via TCP/libp2p)

## Architecture
```
┌─────────────────────────────────────────────────────────┐
│                     ping-common                         │
│  ┌──────────────────────────────────────────────────┐   │
│  │  Ping, Pong (message types)                      │   │
│  │  PingActor (transport-agnostic business logic)   │   │
│  └──────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────┘
                          │
        ┌─────────────────┴─────────────────┐
        │                                   │
        ▼                                   ▼
┌──────────────────┐              ┌──────────────────┐
│  WebSocket       │              │  TCP/libp2p      │
│  (Browser)       │              │  (CLI)           │
├──────────────────┤              ├──────────────────┤
│ • JavaScript     │              │ • ping-cli-      │
│ • WebAssembly    │              │   client         │
│ • ping-http-     │              │ • ping-cli-      │
│   server         │              │   server         │
└──────────────────┘              └──────────────────┘
```

### Key Components

- **`ping-common`**: Core message types and actor logic (transport-agnostic)
- **`ping-http-server`**: HTTP/WebSocket server hosting browser clients
- **`ping-wasm-client`**: WebAssembly client (Rust compiled to Wasm)
- **`ping-cli-server`**: TCP/libp2p server for CLI clients
- **`ping-cli-client`**: CLI client using Kameo's distributed actor system

## Key Finding

✅ **The same `PingActor` implementation handles messages from all transports without modification**

The actor's message handler remains identical whether processing messages from:
- Browser JavaScript (WebSocket)
- Browser WebAssembly (WebSocket)
- CLI application (TCP/libp2p)

## Prerequisites

- Rust 1.63 or later
- wasm-pack (for WebAssembly builds)
- A modern web browser

### Installing wasm-pack
```bash
curl https://rustwasm.github.io/wasm-pack/installer/init.sh -sSf | sh
```

## Quick Start

### 1. WebSocket Demo (JavaScript Client)

**Start the HTTP server:**
```bash
cargo run -p ping-http-server
```

**Open your browser:**
```
http://localhost:8080
```

Click "Connect" → "Send Ping" to test.

### 2. WebSocket Demo (WebAssembly Client)

**Build the Wasm client:**
```bash
./buildwasm.sh
```

**Start the HTTP server:**
```bash
cargo run -p ping-http-server
```

**Open your browser:**
```
http://localhost:8080/static/wasm.html
```

Click "Connect" → "Send Ping" to test.

### 3. CLI Demo (TCP/libp2p)

**Terminal 1 - Start the server:**
```bash
cargo run -p ping-cli-server
```

Note the server's peer ID from the output:
```
Server Peer ID: 12D3KooW...
```

**Terminal 2 - Run the client:**
```bash
cargo run -p ping-cli-client -- --server "/ip4/127.0.0.1/tcp/36341/p2p/12D3KooW..."
```

Replace `12D3KooW...` with the actual peer ID from Terminal 1.

## Project Structure
```
ping_extended/
├── ping-common/           # Shared message types and actor logic
│   ├── src/lib.rs        # Transport-agnostic PingActor
│   └── Cargo.toml        # Feature-based compilation
│
├── ping-http-server/     # WebSocket server
│   ├── src/main.rs       # Axum HTTP/WebSocket server
│   └── static/           # Static files for browser clients
│
├── ping-wasm-client/     # WebAssembly client
│   ├── src/lib.rs        # Rust code compiled to Wasm
│   └── Cargo.toml        # Wasm-specific dependencies
│
├── ping-cli-server/      # TCP/libp2p server
│   └── src/main.rs       # Custom libp2p swarm with Kameo
│
├── ping-cli-client/      # CLI client
│   └── src/main.rs       # Distributed actor lookup and messaging
│
├── buildwasm.sh          # WebAssembly build script
└── README.md
```

## Technical Implementation

### Transport Abstraction

The `PingActor` in `ping-common/src/lib.rs` contains the core business logic:
```rust
impl Message<Ping> for PingActor {
    type Reply = PongReply;

    async fn handle(&mut self, msg: Ping, _ctx: &mut Context<Self, Self::Reply>) -> Self::Reply {
        self.ping_count += 1;
        let pong = Pong {
            message: format!("Pong! Responding to: {}", msg.message),
            sequence: msg.sequence,
            total_pings: self.ping_count,
        };
        PongReply(pong)
    }
}
```

This code **never changes** regardless of transport.

### WebSocket Bridge

The HTTP server bridges WebSocket messages to Kameo actors:
```rust
// Deserialize incoming WebSocket JSON
let ping: Ping = serde_json::from_str(&text)?;

// Forward to actor (same code as TCP)
let pong_reply = actor.ask(&ping).await?;

// Serialize and send response
let json = serde_json::to_string(&pong_reply.0)?;
socket.send(Message::Text(json)).await?;
```

### Feature-Based Compilation

The `ping-common` crate uses Cargo features to separate concerns:
```toml
[features]
default = []
actor = ["kameo", "tokio"]
```

- **WebSocket/Wasm clients**: Use only message types (no `actor` feature)
- **CLI and HTTP server**: Use full actor implementation (with `actor` feature)

## Testing

### Automated Tests
```bash
# Build all components
cargo build --workspace

# Build WebAssembly
./buildwasm.sh
```

### Manual Testing Checklist

- [ ] JavaScript client connects and sends pings
- [ ] WebAssembly client connects and sends pings
- [ ] CLI client finds remote actor via DHT
- [ ] CLI client sends 10 pings successfully
- [ ] All clients receive correct pong counts

## Performance Notes

- **WebSocket latency**: < 50ms (typical)
- **TCP/libp2p latency**: ~900ms average (includes DHT lookup overhead)
- **DHT propagation time**: ~15 seconds for actor registration

## Deployment Considerations

### Local Testing
All demos default to `localhost` / `127.0.0.1`.

### Network Testing
The CLI components support WAN deployment:

1. Update server to listen on public interface:
```rust
   swarm.listen_on("/ip4/0.0.0.0/tcp/36341".parse()?)?;
```

2. Use server's public IP in client connection string:
```bash
   cargo run -p ping-cli-client -- --server "/ip4/PUBLIC_IP/tcp/36341/p2p/PEER_ID"
```
