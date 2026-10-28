//! Starting and stopping the relay.
//!
//! Shutdown is graceful: `/healthz` turns to 503 and new sessions are
//! refused, and every open session finishes as if its client had sent
//! `stop`, so drafts become finals before the socket closes with 1012
//! (service restart).

use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use tokio::net::TcpListener;
use tokio::sync::watch;
use tokio::task::JoinHandle;
use tokio_util::task::TaskTracker;

use crate::app;
use crate::app::App;
use crate::app::SharedApp;
use crate::auth::TokenSigner;
use crate::auth::WeakSecret;
use crate::config::Config;
use crate::polish;
use crate::polish::Polisher;

/// Time beyond the finish and polish timeouts that shutdown waits for
/// sessions to close.
const DRAIN_SLACK: Duration = Duration::from_secs(2);

/// A failure to start.
#[derive(Debug, thiserror::Error)]
pub enum StartError {
	/// The listener could not bind.
	#[error("could not listen: {0}")]
	Bind(#[from] std::io::Error),
	/// The token secret is too short.
	#[error(transparent)]
	Secret(#[from] WeakSecret),
	/// The HTTP client for polishing could not be built.
	#[error("could not build the HTTP client: {0}")]
	Http(#[from] reqwest::Error),
}

/// A running relay.
#[derive(Debug)]
pub struct Server {
	addr: SocketAddr,
	stop: watch::Sender<bool>,
	task: JoinHandle<()>,
}

impl Server {
	/// Bind and start serving `config`.
	pub async fn start(config: Config) -> Result<Self, StartError> {
		let signer = TokenSigner::new(config.token_secret.expose())?;

		let polisher = match &config.polish {
			Some(polish) => {
				let client = polish::client(polish.timeout)?;
				Some(Polisher::new(polish.clone(), client))
			}
			None => None,
		};

		let listener = TcpListener::bind(config.bind).await?;
		let addr = listener.local_addr()?;
		let (stop, stopped) = watch::channel(false);
		let sessions = TaskTracker::new();
		let drain = config.limits.finish_timeout
			+ config
				.polish
				.as_ref()
				.map_or(Duration::ZERO, |polish| polish.timeout)
			+ DRAIN_SLACK;

		tracing::info!(
			%addr,
			provider = ?config.provider.kind,
			polish = ?config.polish.as_ref().map(|polish| polish.kind),
			"canvist dictation relay listening"
		);

		let app = Arc::new(App::new(
			config,
			signer,
			polisher,
			stopped.clone(),
			sessions.clone(),
		));
		let task = tokio::spawn(run(app, listener, stopped, sessions, drain));

		Ok(Self { addr, stop, task })
	}

	/// Where the relay listens.
	pub fn addr(&self) -> SocketAddr {
		self.addr
	}

	/// Shut down gracefully: finish open sessions, then stop.
	pub async fn shutdown(self) {
		let _ = self.stop.send(true);

		if let Err(error) = self.task.await {
			tracing::error!(%error, "the server task failed");
		}
	}

	/// Run until SIGTERM or Ctrl-C, then shut down gracefully.
	pub async fn run_until_signal(self) {
		wait_for_signal().await;
		tracing::info!("shutting down");
		self.shutdown().await;
	}
}

async fn run(
	app: SharedApp,
	listener: TcpListener,
	mut stopped: watch::Receiver<bool>,
	sessions: TaskTracker,
	drain: Duration,
) {
	let served = axum::serve(listener, app::router(app))
		.with_graceful_shutdown(async move {
			let _ = stopped.wait_for(|stopping| *stopping).await;
		})
		.await;

	if let Err(error) = served {
		tracing::error!(%error, "the HTTP server failed");
	}

	sessions.close();

	if tokio::time::timeout(drain, sessions.wait()).await.is_err() {
		tracing::warn!("sessions still open after the drain period");
	}

	tracing::info!("shutdown complete");
}

async fn wait_for_signal() {
	let interrupt = async {
		let _ = tokio::signal::ctrl_c().await;
	};

	#[cfg(unix)]
	let terminate = async {
		match tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()) {
			Ok(mut signal) => {
				signal.recv().await;
			}
			Err(error) => {
				tracing::error!(%error, "cannot listen for SIGTERM");
				std::future::pending::<()>().await;
			}
		}
	};

	#[cfg(not(unix))]
	let terminate = std::future::pending::<()>();

	tokio::select! {
		() = interrupt => {}
		() = terminate => {}
	}
}
