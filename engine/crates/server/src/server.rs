use std::sync::Arc;

use cex_proto::{read_json_frame, write_json_frame, EngineRequest, EngineResponse, FrameError};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{broadcast, mpsc, Mutex};
use tokio::task::JoinError;
use tokio_util::sync::CancellationToken;
use tracing::{debug, warn, Instrument};

use crate::config::EngineServerConfig;
use crate::error::ServerError;
use crate::state::EngineState;
use crate::time::{Clock, IdSource, SystemClock, UuidV4Source};

const EVENT_CHANNEL_CAPACITY: usize = 1_024;
const RESPONSE_CHANNEL_CAPACITY: usize = 128;

/// Matching-engine TCP server.
#[allow(clippy::module_name_repetitions)]
pub struct EngineServer {
    config: EngineServerConfig,
    id_source: Arc<dyn IdSource>,
    clock: Arc<dyn Clock>,
}

impl EngineServer {
    /// Creates a server with production system ports.
    #[must_use]
    pub fn new(config: EngineServerConfig) -> Self {
        Self {
            config,
            id_source: Arc::new(UuidV4Source),
            clock: Arc::new(SystemClock),
        }
    }

    /// Creates a server with injected deterministic ports.
    #[must_use]
    pub fn with_ports(
        config: EngineServerConfig,
        id_source: Arc<dyn IdSource>,
        clock: Arc<dyn Clock>,
    ) -> Self {
        Self {
            config,
            id_source,
            clock,
        }
    }

    /// Binds the configured TCP address and serves until cancellation.
    ///
    /// # Errors
    ///
    /// Returns [`ServerError`] if binding, WAL setup, frame IO, or task joins fail.
    pub async fn run(self, cancellation: CancellationToken) -> Result<(), ServerError> {
        if !self.config.bind_addr.ip().is_loopback() {
            return Err(ServerError::NonLoopbackBind(self.config.bind_addr));
        }
        let listener = TcpListener::bind(self.config.bind_addr).await?;
        self.run_listener(listener, cancellation).await
    }

    /// Serves an existing listener until cancellation.
    ///
    /// This is primarily used by integration tests with port `0` bindings.
    ///
    /// # Errors
    ///
    /// Returns [`ServerError`] if WAL setup, frame IO, or task joins fail.
    pub async fn run_listener(
        self,
        listener: TcpListener,
        cancellation: CancellationToken,
    ) -> Result<(), ServerError> {
        let bind_addr = listener.local_addr()?;
        if !bind_addr.ip().is_loopback() {
            return Err(ServerError::NonLoopbackBind(bind_addr));
        }

        let (event_tx, _) = broadcast::channel(EVENT_CHANNEL_CAPACITY);
        let state = Arc::new(Mutex::new(EngineState::open(
            &self.config,
            event_tx,
            self.id_source,
            self.clock,
        )?));

        loop {
            tokio::select! {
                () = cancellation.cancelled() => {
                    debug!("engine.server.shutdown");
                    return Ok(());
                }
                accepted = listener.accept() => {
                    let (stream, peer_addr) = accepted?;
                    if !peer_addr.ip().is_loopback() {
                        warn!(%peer_addr, "engine.connection.rejected_non_loopback_peer");
                        continue;
                    }
                    let connection_state = Arc::clone(&state);
                    tokio::spawn(
                        async move {
                            if let Err(error) = handle_connection(stream, connection_state).await {
                                warn!(%peer_addr, error = %error, "engine.connection.closed_with_error");
                            }
                        }
                        .instrument(tracing::info_span!("engine.connection", %peer_addr)),
                    );
                }
            }
        }
    }
}

async fn handle_connection(
    stream: TcpStream,
    state: Arc<Mutex<EngineState>>,
) -> Result<(), ServerError> {
    let mut event_rx = {
        let guard = state.lock().await;
        guard.subscribe()
    };
    let (mut reader, mut writer) = stream.into_split();
    let (response_tx, mut response_rx) = mpsc::channel::<EngineResponse>(RESPONSE_CHANNEL_CAPACITY);

    let writer_task = tokio::spawn(async move {
        while let Some(response) = response_rx.recv().await {
            write_json_frame(&mut writer, &response).await?;
        }
        Ok::<(), FrameError>(())
    });

    let event_response_tx = response_tx.clone();
    let event_task = tokio::spawn(async move {
        loop {
            match event_rx.recv().await {
                Ok(event) => {
                    if event_response_tx
                        .send(EngineResponse::Event(event))
                        .await
                        .is_err()
                    {
                        break;
                    }
                }
                Err(broadcast::error::RecvError::Lagged(skipped)) => {
                    warn!(skipped, "engine.connection.event_lagged");
                }
                Err(broadcast::error::RecvError::Closed) => break,
            }
        }
    });

    loop {
        let Some(request) = read_json_frame::<_, EngineRequest>(&mut reader).await? else {
            break;
        };
        let bundle = {
            let mut guard = state.lock().await;
            guard.handle_request(request)?
        };
        response_tx.send(bundle.response).await?;
        {
            let guard = state.lock().await;
            for event in bundle.events {
                guard.broadcast(event);
            }
        }
    }

    drop(response_tx);
    event_task.abort();
    handle_event_task_join(event_task.await)?;
    handle_writer_task_join(writer_task.await)
}

fn handle_event_task_join(result: Result<(), JoinError>) -> Result<(), ServerError> {
    match result {
        Ok(()) => Ok(()),
        Err(error) if error.is_cancelled() => Ok(()),
        Err(error) => Err(ServerError::Join(error)),
    }
}

fn handle_writer_task_join(
    result: Result<Result<(), FrameError>, JoinError>,
) -> Result<(), ServerError> {
    match result {
        Ok(Ok(())) => Ok(()),
        Ok(Err(error)) => Err(ServerError::Frame(error)),
        Err(error) => Err(ServerError::Join(error)),
    }
}
