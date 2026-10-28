//! `canvist-dictation`: run the relay configured by the environment.

use std::process::ExitCode;

use canvist_dictation_relay::Config;
use canvist_dictation_relay::Server;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> ExitCode {
	init_tracing();

	let config = match Config::from_env() {
		Ok(config) => config,
		Err(error) => {
			tracing::error!(%error, "invalid configuration");
			return ExitCode::from(2);
		}
	};

	let server = match Server::start(config).await {
		Ok(server) => server,
		Err(error) => {
			tracing::error!(%error, "could not start");
			return ExitCode::FAILURE;
		}
	};

	server.run_until_signal().await;
	ExitCode::SUCCESS
}

/// Log to stderr: JSON when `CANVIST_LOG_FORMAT=json` (the container
/// default), human-readable otherwise. `RUST_LOG` filters, defaulting to
/// `info`.
fn init_tracing() {
	let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
	let builder = tracing_subscriber::fmt()
		.with_env_filter(filter)
		.with_writer(std::io::stderr);

	if std::env::var("CANVIST_LOG_FORMAT").is_ok_and(|format| format == "json") {
		builder.json().init();
	} else {
		builder.init();
	}
}
