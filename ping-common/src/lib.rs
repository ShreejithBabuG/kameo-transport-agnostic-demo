use serde::{Deserialize, Serialize};

/// Ping message - used across all transports (WebSocket, TCP, etc.)
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Ping {
    pub message: String,
    pub sequence: u64,
}

/// Pong response - used across all transports (WebSocket, TCP, etc.)
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Pong {
    pub message: String,
    pub sequence: u64,
    pub total_pings: u64,
}

// Actor implementation - only compiled when "actor" feature is enabled
// This keeps the Wasm client lightweight (no Kameo dependency)
#[cfg(feature = "actor")]
pub mod actor {
    use super::*;
    use kameo::prelude::*;

    /// PingActor - core business logic, completely transport-agnostic
    /// This same code handles messages from WebSocket and TCP clients
    #[derive(Actor)]
    pub struct PingActor {
        pub ping_count: u64,
    }

    impl RemoteActor for PingActor {
        const REMOTE_ID: &'static str = "ping_pong_app::PingActor";
    }

    /// Reply wrapper implementing Kameo's Reply trait
    /// Required for remote messaging with Serialize/Deserialize
    #[derive(Reply, Serialize, Deserialize, Clone, Debug)]
    pub struct PongReply(pub Pong);

    /// Message handler - THE SAME CODE FOR ALL TRANSPORTS
    /// Handles Ping messages from WebSocket clients (browser) and TCP clients (CLI)
    #[remote_message("a1b2c3d4-e5f6-7890-abcd-ef1234567890")]
    impl Message<Ping> for PingActor {
        type Reply = PongReply;

        async fn handle(
            &mut self,
            msg: Ping,
            _ctx: &mut Context<Self, Self::Reply>,
        ) -> Self::Reply {
            // Increment ping counter
            self.ping_count += 1;

            // Create response with current state
            let pong = Pong {
                message: format!("Pong! Responding to: {}", msg.message),
                sequence: msg.sequence,
                total_pings: self.ping_count,
            };
            
            PongReply(pong)
        }
    }
}

// Re-export actor types when feature is enabled
#[cfg(feature = "actor")]
pub use actor::*;