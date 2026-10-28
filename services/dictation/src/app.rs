//! Shared state and the HTTP routes.

use std::collections::HashMap;
use std::sync::Arc;

use axum::Router;
use axum::extract::Query;
use axum::extract::State;
use axum::extract::WebSocketUpgrade;
use axum::http::HeaderMap;
use axum::http::StatusCode;
use axum::http::header::AUTHORIZATION;
use axum::response::IntoResponse;
use axum::response::Response;
use axum::routing::get;
use tokio::sync::watch;
use tokio_util::task::TaskTracker;

use crate::auth;
use crate::auth::AuthError;
use crate::auth::Claims;
use crate::auth::TokenSigner;
use crate::config::Config;
use crate::polish::Polisher;
use crate::session;

/// Room above the frame limit before axum drops a message unread, so a
/// slightly oversized frame still gets a `limit` error.
const FRAME_SLACK_BYTES: usize = 4 * 1024;

/// What every session shares.
#[derive(Debug)]
pub struct App {
	/// The configuration the relay started with.
	pub config: Config,
	signer: TokenSigner,
	/// The polish pass, when one is configured.
	pub polisher: Option<Polisher>,
	/// Turns `true` when the relay starts shutting down.
	pub stopped: watch::Receiver<bool>,
	sessions: TaskTracker,
}

/// The state handlers share.
pub type SharedApp = Arc<App>;

impl App {
	/// State for `config`.
	pub fn new(
		config: Config,
		signer: TokenSigner,
		polisher: Option<Polisher>,
		stopped: watch::Receiver<bool>,
		sessions: TaskTracker,
	) -> Self {
		Self {
			config,
			signer,
			polisher,
			stopped,
			sessions,
		}
	}

	/// Verify the token in `headers` or the `token` query parameter.
	fn authenticate(&self, headers: &HeaderMap, query: Option<&str>) -> Result<Claims, AuthError> {
		let header = headers
			.get(AUTHORIZATION)
			.and_then(|value| value.to_str().ok())
			.and_then(auth::bearer);
		let token = header.or(query).ok_or(AuthError::Missing)?;

		self.signer.verify(token, auth::unix_now())
	}
}

/// The relay's routes: `GET /v1/dictation` and `GET /healthz`.
pub fn router(app: SharedApp) -> Router {
	Router::new()
		.route(canvist_dictation::PATH, get(dictation))
		.route("/healthz", get(healthz))
		.with_state(app)
}

/// Upgrade to a dictation session. A refused token is reported over the
/// socket as an `unauthorized` error, because browsers cannot read the
/// status of a failed WebSocket handshake.
async fn dictation(
	State(app): State<SharedApp>,
	Query(query): Query<HashMap<String, String>>,
	headers: HeaderMap,
	upgrade: WebSocketUpgrade,
) -> Response {
	if *app.stopped.borrow() {
		return StatusCode::SERVICE_UNAVAILABLE.into_response();
	}

	let auth = app.authenticate(&headers, query.get("token").map(String::as_str));
	let sessions = app.sessions.clone();
	let max_message = app.config.limits.max_frame_bytes + FRAME_SLACK_BYTES;

	upgrade
		.max_message_size(max_message)
		.max_frame_size(max_message)
		.on_upgrade(move |socket| sessions.track_future(session::run(app, socket, auth)))
}

/// 200 while serving, 503 once shutdown begins.
async fn healthz(State(app): State<SharedApp>) -> StatusCode {
	if *app.stopped.borrow() {
		StatusCode::SERVICE_UNAVAILABLE
	} else {
		StatusCode::OK
	}
}
