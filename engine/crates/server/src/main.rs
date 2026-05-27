use std::net::{IpAddr, Ipv4Addr, SocketAddr};

use anyhow::Context;
use cex_server::{EngineServer, EngineServerConfig};
use tokio_util::sync::CancellationToken;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .json()
        .with_env_filter(EnvFilter::from_default_env())
        .try_init()
        .map_err(|error| anyhow::anyhow!("initialising tracing subscriber: {error}"))?;

    // Allow the bind address to be overridden at deploy time.
    // Defaults to 127.0.0.1 (loopback) for local dev; set ENGINE_HOST=0.0.0.0
    // in containerised deployments to accept connections from the private network.
    let mut config = EngineServerConfig::default();
    if let Some(bind_addr) = engine_bind_addr() {
        config.bind_addr = bind_addr;
    }

    tracing::info!(bind_addr = %config.bind_addr, "engine.starting");

    let cancellation = CancellationToken::new();
    let server = EngineServer::new(config);
    let run = server.run(cancellation.clone());
    tokio::pin!(run);

    // `tokio::select!` expands helper items that trip `redundant_pub_crate`.
    #[allow(clippy::redundant_pub_crate)]
    {
        tokio::select! {
            result = &mut run => result.context("running engine server")?,
            signal = tokio::signal::ctrl_c() => {
                signal.context("waiting for ctrl-c")?;
                cancellation.cancel();
                run.await.context("draining engine server after shutdown")?;
            }
        }
    }

    Ok(())
}

/// Reads `ENGINE_HOST` + `ENGINE_PORT` from the environment.
///
/// Returns `None` when neither variable is set (local dev default is used).
fn engine_bind_addr() -> Option<SocketAddr> {
    let host = std::env::var("ENGINE_HOST").ok()?;
    let port = std::env::var("ENGINE_PORT")
        .ok()
        .and_then(|v| v.parse::<u16>().ok())
        .unwrap_or(7878);
    let ip: IpAddr = host
        .parse()
        .unwrap_or(IpAddr::V4(Ipv4Addr::LOCALHOST));
    Some(SocketAddr::new(ip, port))
}
