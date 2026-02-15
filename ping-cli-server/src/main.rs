use kameo::prelude::*;
use kameo::remote;
use libp2p::{
    noise, tcp, yamux,
    swarm::{NetworkBehaviour, SwarmEvent},
};
use ping_common::PingActor;
use std::time::Duration;
use tracing::info;
use tracing_subscriber::EnvFilter;
use futures::StreamExt;

// Custom network behavior wrapping Kameo's remote messaging
#[derive(NetworkBehaviour)]
struct MyBehaviour {
    kameo: remote::Behaviour,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")))
        .init();

    info!("Starting CLI Ping Server...");

    // Build libp2p swarm with TCP transport and Kameo behavior
    let mut swarm = libp2p::SwarmBuilder::with_new_identity()
        .with_tokio()
        .with_tcp(tcp::Config::default(), noise::Config::new, || yamux::Config::default())?
        .with_behaviour(|key| {
            let peer_id = key.public().to_peer_id();
            let messaging_config = remote::messaging::Config::default()
                .with_request_timeout(Duration::from_secs(120));
            let kameo = remote::Behaviour::new(peer_id, messaging_config);
            Ok(MyBehaviour { kameo })
        })?
        .with_swarm_config(|c| c.with_idle_connection_timeout(Duration::from_secs(600)))
        .build();

    // Initialize Kameo's global actor registry
    swarm.behaviour().kameo.init_global();

    let peer_id = *swarm.local_peer_id();
    info!("Server Peer ID: {}", peer_id);

    // Listen on TCP port 36341
    swarm.listen_on("/ip4/0.0.0.0/tcp/36341".parse()?)?;

    // Spawn and register the PingActor in the distributed registry
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(100)).await;
        let ping_actor = PingActor::spawn(PingActor { ping_count: 0 });
        match ping_actor.register("ping_actor").await {
            Ok(_) => info!("PingActor registered successfully"),
            Err(e) => info!("Failed to register PingActor: {}", e),
        }
    });

    info!("Waiting for connections...");

    // Main event loop - handle swarm events
    loop {
        tokio::select! {
            event = swarm.select_next_some() => {
                match event {
                    SwarmEvent::Behaviour(MyBehaviourEvent::Kameo(event)) => {
                        info!("Kameo event: {:?}", event);
                    }
                    SwarmEvent::ConnectionEstablished { peer_id, endpoint, .. } => {
                        info!("Client connected: {}", peer_id);
                        let remote_addr = endpoint.get_remote_address().clone();
                        swarm.add_peer_address(peer_id, remote_addr);
                    }
                    SwarmEvent::NewListenAddr { address, .. } => {
                        info!("Listening on {}", address);
                        info!("Connection string: /ip4/.../tcp/36341/p2p/{}", peer_id);
                    }
                    _ => {}
                }
            }
            _ = tokio::signal::ctrl_c() => {
                info!("Shutting down...");
                break;
            }
        }
    }

    Ok(())
}