//! A stand-in for Resend's `POST /emails`: every message it receives, in
//! order.

use std::time::Duration;

use axum::Json;
use axum::Router;
use axum::extract::State;
use axum::http::HeaderMap;
use axum::routing::post;
use canvist_cloud::config::Delivery;
use canvist_cloud::config::ResendConfig;
use canvist_cloud::config::Secret;
use serde_json::Value;
use serde_json::json;
use tokio::sync::mpsc;

/// The API key the fake expects.
const API_KEY: &str = "re_test_canvist";

/// A running fake.
pub struct FakeMail {
	url: String,
	received: mpsc::UnboundedReceiver<Value>,
}

impl FakeMail {
	pub async fn start() -> Self {
		let (sender, received) = mpsc::unbounded_channel();
		let app = Router::new()
			.route("/emails", post(send))
			.with_state(sender);
		let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
			.await
			.expect("binds");
		let url = format!("http://{}", listener.local_addr().expect("addr"));
		tokio::spawn(async move { axum::serve(listener, app).await });

		Self { url, received }
	}

	/// Delivery through this fake.
	pub fn delivery(&self) -> Delivery {
		Delivery::Resend(ResendConfig {
			api_key: Secret::new(API_KEY),
			from: "canvist <admin@canvist.test>".to_owned(),
			api_base: self.url.clone(),
		})
	}

	/// The next message, waiting up to five seconds.
	pub async fn next(&mut self) -> Value {
		tokio::time::timeout(Duration::from_secs(5), self.received.recv())
			.await
			.expect("a message arrives")
			.expect("the fake is running")
	}

	/// Whether no message arrives within `wait`.
	pub async fn nothing_within(&mut self, wait: Duration) -> bool {
		tokio::time::timeout(wait, self.received.recv())
			.await
			.is_err()
	}
}

async fn send(
	State(sender): State<mpsc::UnboundedSender<Value>>,
	headers: HeaderMap,
	Json(message): Json<Value>,
) -> Json<Value> {
	let authorized = headers
		.get("authorization")
		.and_then(|value| value.to_str().ok())
		== Some(&format!("Bearer {API_KEY}"));
	assert!(authorized, "the mail API key is sent");
	let _ = sender.send(message);
	Json(json!({ "id": "email_1" }))
}
