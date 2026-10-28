//! A relay, fake providers that replay scripted events, a fake polish
//! model, and a client to drive them. Nothing leaves localhost.

#![allow(dead_code, reason = "each test binary uses a different subset")]

use std::sync::Arc;
use std::sync::Mutex;
use std::time::Duration;

use axum::Json;
use axum::Router;
use axum::http::HeaderMap;
use axum::http::StatusCode;
use axum::routing::post;
use canvist_dictation::ClientMessage;
use canvist_dictation::RelayMessage;
use canvist_dictation_relay::Config;
use canvist_dictation_relay::Server;
use canvist_dictation_relay::audio::encode_pcm16;
use canvist_dictation_relay::auth::Claims;
use canvist_dictation_relay::auth::TokenSigner;
use canvist_dictation_relay::auth::unix_now;
use canvist_dictation_relay::config::ProviderConfig;
use canvist_dictation_relay::config::ProviderKind;
use canvist_dictation_relay::config::Secret;
use futures_util::SinkExt;
use futures_util::Stream;
use futures_util::StreamExt;
use serde_json::Value;
use tokio::net::TcpListener;
use tokio::net::TcpStream;
use tokio_tungstenite::MaybeTlsStream;
use tokio_tungstenite::WebSocketStream;
use tokio_tungstenite::tungstenite;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::tungstenite::handshake::server::Request;
use tokio_tungstenite::tungstenite::handshake::server::Response;

/// The token secret every test relay uses.
pub const SECRET: &str = "test-secret-test-secret-test-secret!";
/// How long a test waits for any one message.
const PATIENCE: Duration = Duration::from_secs(5);

/// A dictation token for `user-1`, valid for five minutes.
pub fn token() -> String {
	sign(&Claims::new("user-1", unix_now(), Duration::from_mins(5)))
}

/// `claims` signed with the test secret.
pub fn sign(claims: &Claims) -> String {
	TokenSigner::new(SECRET).expect("long enough").sign(claims)
}

// ---------------------------------------------------------------------------
// The fake provider
// ---------------------------------------------------------------------------

/// What the fake provider received.
#[derive(Clone, Debug, PartialEq)]
pub enum Incoming {
	/// A connection, with its path and query and `Authorization` header.
	Connected {
		/// The request path and query.
		uri: String,
		/// The `Authorization` header.
		authorization: Option<String>,
	},
	/// A JSON text message.
	Json(Value),
	/// A binary message of this many bytes.
	Binary(usize),
}

/// What the fake provider does in reply.
#[derive(Clone, Debug)]
pub enum Reply {
	/// Send a JSON text message.
	Send(Value),
	/// Wait before the next reply.
	Wait(Duration),
	/// Close the connection.
	Close,
}

/// A provider the relay connects to, which answers each message with a
/// script.
#[derive(Clone, Debug)]
pub struct FakeProvider {
	/// Its `ws://` origin.
	pub url: String,
	received: Arc<Mutex<Vec<Incoming>>>,
}

impl FakeProvider {
	/// Start a provider that answers each incoming message with
	/// `respond(message)`.
	pub async fn start(
		respond: impl FnMut(&Incoming) -> Vec<Reply> + Send + 'static,
	) -> FakeProvider {
		let listener = TcpListener::bind("127.0.0.1:0").await.expect("binds");
		let url = format!("ws://{}", listener.local_addr().expect("bound"));
		let received = Arc::new(Mutex::new(Vec::new()));
		let log = received.clone();
		let respond = Arc::new(Mutex::new(respond));

		tokio::spawn(async move {
			while let Ok((stream, _)) = listener.accept().await {
				let log = log.clone();
				let respond = respond.clone();
				tokio::spawn(serve(stream, log, respond));
			}
		});

		FakeProvider { url, received }
	}

	/// Everything received so far.
	pub fn received(&self) -> Vec<Incoming> {
		self.received.lock().expect("unpoisoned").clone()
	}

	/// The JSON messages received so far.
	pub fn json(&self) -> Vec<Value> {
		self.received()
			.into_iter()
			.filter_map(|incoming| {
				match incoming {
					Incoming::Json(value) => Some(value),
					_ => None,
				}
			})
			.collect()
	}

	/// The provider configuration that points at this fake.
	pub fn config(&self, kind: ProviderKind) -> ProviderConfig {
		ProviderConfig {
			url: self.url.clone(),
			..ProviderConfig::new(kind, Some(Secret::new("provider-key")))
		}
	}
}

type Respond = Arc<Mutex<dyn FnMut(&Incoming) -> Vec<Reply> + Send>>;
type ProviderSocket = WebSocketStream<TcpStream>;

/// One provider connection: log each incoming message and play the
/// script's replies, until the script closes or the relay goes away.
async fn serve(stream: TcpStream, log: Arc<Mutex<Vec<Incoming>>>, respond: Respond) {
	let Some((mut socket, connected)) = accept(stream).await else {
		return;
	};
	let mut incoming = Some(connected);

	while let Some(message) = incoming {
		log.lock().expect("unpoisoned").push(message.clone());
		let replies = (respond.lock().expect("unpoisoned"))(&message);

		if !play(&mut socket, replies).await {
			return;
		}

		incoming = next_incoming(&mut socket).await;
	}
}

/// Complete the handshake, noting the request's URI and `Authorization`.
#[allow(
	clippy::result_large_err,
	reason = "the handshake callback returns tungstenite's error response"
)]
async fn accept(stream: TcpStream) -> Option<(ProviderSocket, Incoming)> {
	let mut connected = None;
	let callback = |request: &Request, response: Response| {
		let authorization = request
			.headers()
			.get("authorization")
			.and_then(|value| value.to_str().ok())
			.map(str::to_owned);
		connected = Some(Incoming::Connected {
			uri: request.uri().to_string(),
			authorization,
		});
		Ok(response)
	};
	let socket = tokio_tungstenite::accept_hdr_async(stream, callback)
		.await
		.ok()?;

	Some((socket, connected?))
}

/// The next text or binary message, or `None` once the relay goes away.
async fn next_incoming(socket: &mut ProviderSocket) -> Option<Incoming> {
	loop {
		match socket.next().await? {
			Ok(Message::Text(text)) => {
				return Some(Incoming::Json(serde_json::from_str(&text).expect("JSON")));
			}
			Ok(Message::Binary(bytes)) => return Some(Incoming::Binary(bytes.len())),
			Ok(_) => {}
			Err(_) => return None,
		}
	}
}

/// Play `replies`. Returns whether the connection stays open.
async fn play(socket: &mut ProviderSocket, replies: Vec<Reply>) -> bool {
	for reply in replies {
		match reply {
			Reply::Send(value) => {
				if socket.send(Message::text(value.to_string())).await.is_err() {
					return false;
				}
			}
			Reply::Wait(duration) => tokio::time::sleep(duration).await,
			Reply::Close => {
				let _ = socket.close(None).await;

				return false;
			}
		}
	}

	true
}

/// Whether `incoming` is a JSON message of `kind`.
pub fn is_type(incoming: &Incoming, kind: &str) -> bool {
	matches!(incoming, Incoming::Json(value) if value["type"] == kind)
}

// ---------------------------------------------------------------------------
// The fake polish model
// ---------------------------------------------------------------------------

/// A request the fake polish model received.
#[derive(Clone, Debug)]
pub struct PolishRequest {
	/// The request headers.
	pub headers: HeaderMap,
	/// The JSON body.
	pub body: Value,
}

impl PolishRequest {
	/// The transcript inside the request's user message.
	pub fn user_message(&self) -> String {
		let messages = self.body["messages"].as_array().expect("messages");
		let user = messages
			.iter()
			.find(|message| message["role"] == "user")
			.expect("a user message");
		user["content"].as_str().expect("text").to_owned()
	}
}

/// How the fake polish model answers.
pub type PolishReply = Arc<dyn Fn(&PolishRequest) -> (Duration, StatusCode, Value) + Send + Sync>;

/// A polish model serving the Messages and Chat Completions APIs.
#[derive(Clone)]
pub struct FakePolish {
	/// Its `http://` origin.
	pub url: String,
	received: Arc<Mutex<Vec<PolishRequest>>>,
}

impl FakePolish {
	/// Start a model that answers with `reply(request)`: a delay, a status,
	/// and a body.
	pub async fn start(
		reply: impl Fn(&PolishRequest) -> (Duration, StatusCode, Value) + Send + Sync + 'static,
	) -> FakePolish {
		let received = Arc::new(Mutex::new(Vec::new()));
		let reply: PolishReply = Arc::new(reply);
		let handler = {
			let received = received.clone();
			move |headers: HeaderMap, Json(body): Json<Value>| {
				let received = received.clone();
				let reply = reply.clone();
				async move {
					let request = PolishRequest { headers, body };
					received.lock().expect("unpoisoned").push(request.clone());
					let (delay, status, body) = reply(&request);
					tokio::time::sleep(delay).await;
					(status, Json(body))
				}
			}
		};
		let router = Router::new()
			.route("/v1/messages", post(handler.clone()))
			.route("/v1/chat/completions", post(handler));
		let listener = TcpListener::bind("127.0.0.1:0").await.expect("binds");
		let url = format!("http://{}", listener.local_addr().expect("bound"));
		tokio::spawn(async move { axum::serve(listener, router).await });

		FakePolish { url, received }
	}

	/// Every request so far.
	pub fn received(&self) -> Vec<PolishRequest> {
		self.received.lock().expect("unpoisoned").clone()
	}
}

/// A Messages API response with `text`.
pub fn anthropic_reply(text: &str) -> Value {
	serde_json::json!({
		"type": "message",
		"role": "assistant",
		"content": [{ "type": "thinking", "thinking": "", "signature": "x" }, { "type": "text", "text": text }],
		"stop_reason": "end_turn",
	})
}

// ---------------------------------------------------------------------------
// The relay and its client
// ---------------------------------------------------------------------------

/// Start a relay for `provider`, adjusted by `adjust`.
pub async fn relay(provider: ProviderConfig, adjust: impl FnOnce(&mut Config)) -> Server {
	let mut config = Config::local(SECRET, provider);
	adjust(&mut config);
	Server::start(config).await.expect("starts")
}

/// A client's socket.
pub type Socket = WebSocketStream<MaybeTlsStream<TcpStream>>;

/// Connect to `server` with `token` in the `Authorization` header.
pub async fn connect(server: &Server, token: Option<&str>) -> Socket {
	let url = format!("ws://{}/v1/dictation", server.addr());
	let mut request = url.into_client_request().expect("valid");

	if let Some(token) = token {
		request.headers_mut().insert(
			"authorization",
			format!("Bearer {token}").parse().expect("valid"),
		);
	}

	let (socket, _) = tokio_tungstenite::connect_async(request)
		.await
		.expect("connects");
	socket
}

/// Connect with `token` in the query string, as a browser does.
pub async fn connect_with_query(server: &Server, token: &str) -> Socket {
	let url = format!("ws://{}/v1/dictation?token={token}", server.addr());
	let (socket, _) = tokio_tungstenite::connect_async(url)
		.await
		.expect("connects");
	socket
}

/// Send a client message.
pub async fn send(socket: &mut Socket, message: &ClientMessage) {
	let json = message.to_json().expect("encodes");
	socket.send(Message::text(json)).await.expect("sends");
}

/// Send `start` with `polish`, and wait for `ready`.
pub async fn start(socket: &mut Socket, polish: bool) {
	let start = ClientMessage::Start {
		language: Some("en".to_owned()),
		context: Some("Notes from Monday.".to_owned()),
		polish,
	};
	send(socket, &start).await;
	assert_eq!(next(socket).await, Some(RelayMessage::Ready));
}

/// Send `samples` as 20 ms frames.
pub async fn send_audio(socket: &mut Socket, samples: &[i16]) {
	for frame in samples.chunks(320) {
		socket
			.send(Message::binary(encode_pcm16(frame)))
			.await
			.expect("sends");
	}
}

/// The next relay message, or `None` once the socket closes.
pub async fn next(socket: &mut Socket) -> Option<RelayMessage> {
	loop {
		let message = tokio::time::timeout(PATIENCE, socket.next())
			.await
			.expect("a message arrives in time");

		match message {
			Some(Ok(Message::Text(text))) => {
				return Some(RelayMessage::from_json(&text).expect("a relay message"));
			}
			Some(Ok(Message::Close(_)) | Err(_)) | None => return None,
			Some(Ok(_)) => {}
		}
	}
}

/// Every relay message until the socket closes, and the close code.
pub async fn until_closed<S>(socket: &mut S) -> (Vec<RelayMessage>, Option<u16>)
where
	S: Stream<Item = Result<Message, tungstenite::Error>> + Unpin,
{
	let mut messages = Vec::new();

	loop {
		let message = tokio::time::timeout(PATIENCE, socket.next())
			.await
			.expect("a message arrives in time");

		match message {
			Some(Ok(Message::Text(text))) => {
				messages.push(RelayMessage::from_json(&text).expect("a relay message"));
			}
			Some(Ok(Message::Close(frame))) => {
				let code = frame.map(|frame| u16::from(frame.code));
				return (messages, code);
			}
			Some(Ok(_)) => {}
			Some(Err(_)) | None => return (messages, None),
		}
	}
}

/// The close code for a normal end.
pub const NORMAL: Option<u16> = Some(1000);

// ---------------------------------------------------------------------------
// Audio
// ---------------------------------------------------------------------------

/// A 220 Hz tone, loud enough to be speech, lasting `millis`.
pub fn tone(millis: u32) -> Vec<i16> {
	let count = 16 * millis as usize;
	(0..count)
		.map(|index| {
			let phase = 2.0 * std::f64::consts::PI * 220.0 * index as f64 / 16_000.0;
			(8_000.0 * phase.sin()) as i16
		})
		.collect()
}

/// Digital silence lasting `millis`.
pub fn silence(millis: u32) -> Vec<i16> {
	vec![0; 16 * millis as usize]
}

/// A spoken turn: a second of speech and the pause that ends it.
pub fn turn() -> Vec<i16> {
	[tone(1_000), silence(1_000)].concat()
}

/// A finalized segment message.
pub fn final_message(segment: u32, text: &str) -> RelayMessage {
	RelayMessage::Final {
		segment,
		text: text.to_owned(),
	}
}

/// A draft message.
pub fn partial(segment: u32, text: &str) -> RelayMessage {
	RelayMessage::Partial {
		segment,
		text: text.to_owned(),
	}
}

// ---------------------------------------------------------------------------
// Deepgram scripts
// ---------------------------------------------------------------------------

/// A Deepgram `Results` message.
pub fn deepgram_results(transcript: &str, is_final: bool) -> Reply {
	Reply::Send(serde_json::json!({
		"type": "Results",
		"is_final": is_final,
		"channel": { "alternatives": [{ "transcript": transcript }] },
	}))
}

/// A Deepgram fake that sends the next of `results` after every `every`
/// audio frames, and closes after `CloseStream` when `close_on_stop`.
pub async fn scripted_deepgram(
	results: Vec<Reply>,
	every: usize,
	close_on_stop: bool,
) -> FakeProvider {
	let mut pending = results.into_iter();
	let mut frames = 0;

	FakeProvider::start(move |incoming| {
		if matches!(incoming, Incoming::Binary(_)) {
			frames += 1;
			let due = frames % every == 0;
			return due.then(|| pending.next()).flatten().into_iter().collect();
		}

		if close_on_stop && is_type(incoming, "CloseStream") {
			return vec![Reply::Close];
		}

		Vec::new()
	})
	.await
}
