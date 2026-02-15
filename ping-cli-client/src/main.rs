use kameo::prelude::*;
use kameo::remote;
use libp2p::{
    noise, tcp, yamux,
    swarm::{NetworkBehaviour, SwarmEvent},
    Multiaddr,
};
use ping_common::{PingActor, Ping};
use std::time::{Duration, Instant};
use tracing::{info, warn, error};
use tracing_subscriber::EnvFilter;
use clap::Parser;
use futures::StreamExt;

// Command-line argument parser
#[derive(Parser, Debug)]
#[command(name = "ping-cli-client")]
struct Args {
    #[arg(short, long)]
    server: Option<String>,
}

// Custom network behavior wrapping Kameo's remote messaging
#[derive(NetworkBehaviour)]
struct MyBehaviour {
    kameo: remote::Behaviour,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();
    
    // Initialize logging
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")))
        .init();

    info!("Starting CLI Ping Client...");

    if let Some(server_addr) = args.server {
        info!("Custom swarm mode");
        info!("Server: {}", server_addr);
        
        let server_multiaddr: Multiaddr = server_addr.parse()?;
        
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
        info!("Client Peer ID: {}", swarm.local_peer_id());

        // Listen on random port and dial the server
        swarm.listen_on("/ip4/0.0.0.0/tcp/0".parse()?)?;
        swarm.dial(server_multiaddr.clone())?;

        // Spawn swarm event handler
        let swarm_handle = tokio::spawn(async move {
            loop {
                match swarm.select_next_some().await {
                    SwarmEvent::Behaviour(MyBehaviourEvent::Kameo(event)) => {
                        info!("Kameo event: {:?}", event);
                    }
                    SwarmEvent::ConnectionEstablished { peer_id, endpoint, .. } => {
                        info!("Connected to {} via {}", peer_id, endpoint.get_remote_address());
                    }
                    SwarmEvent::NewListenAddr { address, .. } => {
                        info!("Listening on {}", address);
                    }
                    _ => {}
                }
            }
        });

        // Wait for DHT to propagate
        info!("Waiting for DHT propagation (15s)...");
        tokio::time::sleep(Duration::from_secs(15)).await;

        // Look up the remote PingActor in the distributed registry
        info!("Looking for PingActor in DHT...");
        let remote_actor = loop {
            match RemoteActorRef::<PingActor>::lookup("ping_actor").await? {
                Some(actor) => {
                    info!("Found PingActor!");
                    break actor;
                }
                None => {
                    warn!("Actor not found, retrying...");
                    tokio::time::sleep(Duration::from_secs(3)).await;
                }
            }
        };

        // Send 10 ping messages to the remote actor
        info!("Starting ping-pong sequence...");
        let start = Instant::now();

        for i in 1..=10 {
            let ping = Ping {
                message: format!("Hello from CLI client #{}", i),
                sequence: i,
            };

            info!("Sending PING #{}", i);
            match remote_actor.ask(&ping).await {
                Ok(pong_reply) => {
                    let pong = pong_reply.0;
                    info!("Received PONG #{} (total: {})", pong.sequence, pong.total_pings);
                }
                Err(e) => {
                    error!("Error: {}", e);
                }
            }

            if i < 10 {
                tokio::time::sleep(Duration::from_secs(1)).await;
            }
        }

        let duration = start.elapsed();
        info!("Complete! Total: {:?}, Avg: {:?}", duration, duration / 10);
        swarm_handle.abort();
        
    } else {
        error!("Usage: --server \"/ip4/IP/tcp/PORT/p2p/PEER_ID\"");
    }

    Ok(())
}