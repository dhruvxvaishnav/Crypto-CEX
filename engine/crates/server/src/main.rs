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

    let cancellation = CancellationToken::new();
    let server = EngineServer::new(EngineServerConfig::default());
    let run = server.run(cancellation.clone());
    tokio::pin!(run);

    tokio::select! {
        result = &mut run => result.context("running engine server")?,
        signal = tokio::signal::ctrl_c() => {
            signal.context("waiting for ctrl-c")?;
            cancellation.cancel();
            run.await.context("draining engine server after shutdown")?;
        }
    }

    Ok(())
}
