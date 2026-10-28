//! One dictation session: a client socket, a provider socket, and the
//! segments between them.
//!
//! The session reads `start`, connects to the provider, and sends `ready`.
//! Then one loop moves audio to the provider, runs voice activity
//! detection, turns provider events into drafts and finals, polishes finals,
//! and enforces the limits. `stop`, the duration and idle limits, and
//! shutdown all end a session the same way: the provider finishes every open
//! segment, the last finals go out, then `done`.
//!
//! Audio and transcripts are never logged.

use std::future::Future;
use std::pin::Pin;
use std::time::Duration;

use axum::extract::ws::CloseFrame;
use axum::extract::ws::Message;
use axum::extract::ws::WebSocket;
use canvist_dictation::ClientMessage;
use canvist_dictation::ErrorCode;
use canvist_dictation::MAX_CONTEXT_CHARS;
use canvist_dictation::RelayMessage;
use futures_util::SinkExt;
use futures_util::StreamExt;
use futures_util::stream::FuturesUnordered;
use tokio::net::TcpStream;
use tokio::sync::watch;
use tokio::time::Instant;
use tokio::time::sleep_until;
use tokio_tungstenite::MaybeTlsStream;
use tokio_tungstenite::WebSocketStream;
use tokio_tungstenite::tungstenite;
use tokio_tungstenite::tungstenite::handshake::client::Request;

use crate::app::SharedApp;
use crate::audio::decode_pcm16;
use crate::auth::AuthError;
use crate::auth::Claims;
use crate::config::Limits;
use crate::polish::PolishError;
use crate::polish::Polisher;
use crate::provider;
use crate::provider::Adapter;
use crate::provider::Event;
use crate::provider::Outbox;
use crate::provider::ProviderError;
use crate::provider::SessionOptions;
use crate::segments::Segments;
use crate::segments::tail;
use crate::vad::Vad;

/// The longest the provider handshake may take.
const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);

/// Close codes the relay uses.
mod codes {
	/// Every segment is final.
	pub const NORMAL: u16 = 1000;
	/// The client sent something the protocol does not allow.
	pub const INVALID: u16 = 1007;
	/// The token was refused or a limit was reached.
	pub const POLICY: u16 = 1008;
	/// A frame was larger than the limit.
	pub const TOO_BIG: u16 = 1009;
	/// The provider or the relay failed.
	pub const INTERNAL: u16 = 1011;
	/// The relay is restarting; reconnect.
	pub const RESTART: u16 = 1012;
}

type ProviderSocket = WebSocketStream<MaybeTlsStream<TcpStream>>;
type PolishResult = (u32, String, Result<String, PolishError>);
type Polishing = Pin<Box<dyn Future<Output = PolishResult> + Send>>;

/// What a client asked for in `start`.
struct Start {
	options: SessionOptions,
	context: String,
	polish: bool,
}

/// How a session ended, for the log.
#[derive(Debug)]
enum End {
	/// `done` was sent.
	Done,
	/// The client went away.
	Gone,
	/// A fatal error was sent.
	Failed(ErrorCode),
}

impl std::fmt::Display for End {
	fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		match self {
			Self::Done => formatter.write_str("done"),
			Self::Gone => formatter.write_str("client gone"),
			Self::Failed(code) => write!(formatter, "failed ({code:?})"),
		}
	}
}

/// A fatal error to send before closing.
struct Fatal {
	code: ErrorCode,
	message: String,
	close: u16,
}

impl Fatal {
	fn new(code: ErrorCode, message: impl Into<String>) -> Self {
		let close = match code {
			ErrorCode::Unauthorized | ErrorCode::Limit => codes::POLICY,
			ErrorCode::BadRequest => codes::INVALID,
			ErrorCode::Provider | ErrorCode::Polish | ErrorCode::Internal => codes::INTERNAL,
		};

		Self {
			code,
			message: message.into(),
			close,
		}
	}

	fn provider(error: &ProviderError) -> Self {
		// The provider's own message can quote configuration, such as a
		// masked API key, so the client gets a generic one and the log the
		// detail.
		tracing::warn!(%error, "the speech provider failed");
		Self::new(ErrorCode::Provider, "the speech provider failed")
	}
}

/// Run a session on `socket` for a client whose token checked out as
/// `auth`.
pub async fn run(app: SharedApp, socket: WebSocket, auth: Result<Claims, AuthError>) {
	let mut client = Client(socket);

	let claims = match auth {
		Ok(claims) => claims,
		Err(error) => {
			tracing::info!(%error, "dictation session refused");
			client
				.fail(Fatal::new(ErrorCode::Unauthorized, error.to_string()))
				.await;
			return;
		}
	};

	let started = Instant::now();

	let mut session = match open(&app, &mut client).await {
		Ok(Some((adapter, provider, start))) => Session::new(app, client, adapter, provider, start),
		Ok(None) => return,
		Err(fatal) => {
			tracing::info!(subject = %claims.sub, code = ?fatal.code, "dictation session refused");
			client.fail(fatal).await;
			return;
		}
	};

	tracing::info!(
		subject = %claims.sub,
		polish = session.polisher.is_some(),
		"dictation session started"
	);
	let end = session.run().await;
	tracing::info!(
		subject = %claims.sub,
		segments = session.segments.finals_sent(),
		seconds = started.elapsed().as_secs(),
		end = %end,
		"dictation session ended"
	);
}

/// Read `start`, connect to the provider, and send `ready`. `None` means the
/// client left first.
async fn open(
	app: &SharedApp,
	client: &mut Client,
) -> Result<Option<(Box<dyn Adapter>, ProviderSocket, Start)>, Fatal> {
	let limits = &app.config.limits;
	let mut stopped = app.stopped.clone();
	let Some(start) = read_start(client, limits, &mut stopped).await? else {
		return Ok(None);
	};

	let mut adapter = provider::adapter(&app.config.provider, &start.options);
	let request = adapter.request().map_err(|error| Fatal::provider(&error))?;
	let mut provider = connect(request)
		.await
		.map_err(|error| Fatal::provider(&error))?;

	let mut out = Outbox::default();
	adapter.open(&mut out);

	for message in out.messages {
		provider
			.send(message)
			.await
			.map_err(|error| Fatal::provider(&ProviderError::Connect(error.to_string())))?;
	}

	if !client.send(&RelayMessage::Ready).await {
		return Ok(None);
	}

	Ok(Some((adapter, provider, start)))
}

/// Wait for the client's `start`, within the idle limit. A relay that starts
/// shutting down meanwhile turns the client away to reconnect elsewhere.
async fn read_start(
	client: &mut Client,
	limits: &Limits,
	stopped: &mut watch::Receiver<bool>,
) -> Result<Option<Start>, Fatal> {
	let deadline = Instant::now() + limits.idle;

	loop {
		let message = tokio::select! {
			message = client.0.recv() => message,
			() = sleep_until(deadline) => {
				return Err(Fatal::new(ErrorCode::Limit, "no start message arrived in time"));
			}
			Ok(_) = stopped.wait_for(|stopping| *stopping) => {
				return Err(Fatal {
					close: codes::RESTART,
					..Fatal::new(ErrorCode::Internal, "the relay is restarting")
				});
			}
		};

		let text = match message {
			Some(Ok(Message::Text(text))) => text,
			Some(Ok(Message::Binary(_))) => {
				return Err(Fatal::new(ErrorCode::BadRequest, "send start before audio"));
			}
			Some(Ok(Message::Ping(_) | Message::Pong(_))) => continue,
			Some(Ok(Message::Close(_)) | Err(_)) | None => return Ok(None),
		};

		if text.len() > limits.max_frame_bytes {
			return Err(Fatal::new(
				ErrorCode::Limit,
				"the start message is too large",
			));
		}

		return parse_start(&text).map(Some);
	}
}

fn parse_start(text: &str) -> Result<Start, Fatal> {
	let Ok(ClientMessage::Start {
		language,
		context,
		polish,
	}) = ClientMessage::from_json(text)
	else {
		return Err(Fatal::new(
			ErrorCode::BadRequest,
			"the first message must be start",
		));
	};

	if language
		.as_deref()
		.is_some_and(|tag| !provider::is_language_tag(tag))
	{
		return Err(Fatal::new(
			ErrorCode::BadRequest,
			"language must be a BCP 47 tag",
		));
	}

	let context = context.unwrap_or_default();

	Ok(Start {
		options: SessionOptions { language },
		context: tail(&context, MAX_CONTEXT_CHARS).to_owned(),
		polish,
	})
}

async fn connect(request: Request) -> Result<ProviderSocket, ProviderError> {
	let connecting = tokio_tungstenite::connect_async(request);

	match tokio::time::timeout(CONNECT_TIMEOUT, connecting).await {
		Ok(Ok((socket, _))) => Ok(socket),
		Ok(Err(tungstenite::Error::Http(response))) => {
			Err(ProviderError::Refused(response.status().as_u16()))
		}
		Ok(Err(error)) => Err(ProviderError::Connect(error.to_string())),
		Err(_) => Err(ProviderError::Connect("timed out".to_owned())),
	}
}

/// The client's socket.
struct Client(WebSocket);

impl Client {
	/// Send `message`. Returns whether the client is still there.
	async fn send(&mut self, message: &RelayMessage) -> bool {
		match message.to_json() {
			Ok(json) => self.0.send(Message::Text(json.into())).await.is_ok(),
			Err(error) => {
				tracing::error!(%error, "could not encode a relay message");
				false
			}
		}
	}

	/// Send a fatal error and close.
	async fn fail(&mut self, fatal: Fatal) {
		let error = RelayMessage::error(fatal.code, fatal.message, true);

		if self.send(&error).await {
			self.close(fatal.close).await;
		}
	}

	async fn close(&mut self, code: u16) {
		let frame = CloseFrame {
			code,
			reason: "".into(),
		};
		let _ = self.0.send(Message::Close(Some(frame))).await;
	}
}

/// A session after `ready`.
struct Session {
	app: SharedApp,
	client: Client,
	provider: ProviderSocket,
	/// Whether the provider socket is still open.
	provider_open: bool,
	adapter: Box<dyn Adapter>,
	vad: Vad,
	/// The decoded samples of the latest frame, reused between frames.
	samples: Vec<i16>,
	segments: Segments,
	/// The polish pass, when the relay has one and the client wants it.
	polisher: Option<Polisher>,
	polishing: FuturesUnordered<Polishing>,
	/// The text before the caret.
	context: String,
	/// When the session reaches its duration limit.
	ends_at: Instant,
	last_audio: Instant,
	/// When finishing gives up on the provider, once the session is
	/// finishing.
	finishing: Option<Instant>,
	/// Turns `true` when the relay starts shutting down.
	stopped: watch::Receiver<bool>,
	/// Whether the relay is shutting down, so the client should reconnect.
	restarting: bool,
}

/// What one turn of the loop decided.
enum Step {
	Continue,
	End(End),
}

/// What woke the session.
enum Wake {
	/// The client sent a message or went away.
	Client(Option<Result<Message, axum::Error>>),
	/// The provider sent a message or went away.
	Provider(Option<Result<tungstenite::Message, tungstenite::Error>>),
	/// A segment's polish finished.
	Polished(PolishResult),
	/// The adapter's deadline passed.
	Tick,
	/// No audio arrived within the idle limit.
	Idle,
	/// The session reached its duration limit.
	Expired,
	/// The relay's shutdown signal changed.
	Stopping,
	/// The provider did not finish within the finish timeout.
	FinishTimedOut,
}

impl Session {
	fn new(
		app: SharedApp,
		client: Client,
		adapter: Box<dyn Adapter>,
		provider: ProviderSocket,
		start: Start,
	) -> Self {
		let now = Instant::now();

		Self {
			ends_at: now + app.config.limits.max_session,
			polisher: app.polisher.clone().filter(|_| start.polish),
			stopped: app.stopped.clone(),
			app,
			client,
			provider,
			provider_open: true,
			adapter,
			vad: Vad::new(),
			samples: Vec::new(),
			segments: Segments::default(),
			polishing: FuturesUnordered::new(),
			context: start.context,
			last_audio: now,
			finishing: None,
			restarting: false,
		}
	}

	/// Run until the session ends. Each turn waits for the next thing to
	/// happen, then handles it; every limit, `stop`, and shutdown funnels into
	/// finishing, which ends with `done` once every segment is final.
	async fn run(&mut self) -> End {
		loop {
			if let Some(end) = self.end_if_finished().await {
				return end;
			}

			let wake = self.wait().await;

			if let Step::End(end) = self.handle(wake).await {
				return end;
			}
		}
	}

	/// Wait for the next message, polish, or deadline.
	async fn wait(&mut self) -> Wake {
		let limits = &self.app.config.limits;
		let adapter_deadline = self.adapter.deadline();
		let idle_at = self.last_audio + limits.idle;
		let streaming = self.finishing.is_none();
		let finish_by = self.finishing.unwrap_or(self.ends_at);
		let tick_at = adapter_deadline.unwrap_or(finish_by);

		tokio::select! {
			message = self.client.0.recv() => Wake::Client(message),
			message = self.provider.next(), if self.provider_open => Wake::Provider(message),
			Some(polished) = self.polishing.next() => Wake::Polished(polished),
			() = sleep_until(tick_at), if adapter_deadline.is_some() => Wake::Tick,
			() = sleep_until(idle_at), if streaming => Wake::Idle,
			() = sleep_until(self.ends_at), if streaming => Wake::Expired,
			Ok(()) = self.stopped.changed(), if streaming => Wake::Stopping,
			() = sleep_until(finish_by), if !streaming => Wake::FinishTimedOut,
		}
	}

	async fn handle(&mut self, wake: Wake) -> Step {
		let limits = &self.app.config.limits;

		match wake {
			Wake::Client(message) => self.client_message(message).await,
			Wake::Provider(message) => self.provider_message(message).await,
			Wake::Polished(polished) => self.polished(polished).await,
			Wake::Tick => {
				let mut out = Outbox::default();
				self.adapter.tick(Instant::now(), &mut out);
				self.flush(out).await
			}
			Wake::Idle => {
				let message = format!("no audio for {} seconds", limits.idle.as_secs());
				self.limit(message).await
			}
			Wake::Expired => {
				let seconds = limits.max_session.as_secs();
				self.limit(format!("the session reached its {seconds} second limit"))
					.await
			}
			Wake::Stopping if *self.stopped.borrow_and_update() => {
				self.restarting = true;
				self.begin_finish().await
			}
			Wake::Stopping => Step::Continue,
			Wake::FinishTimedOut => self.give_up().await,
		}
	}

	async fn client_message(&mut self, message: Option<Result<Message, axum::Error>>) -> Step {
		match message {
			Some(Ok(Message::Binary(bytes))) => self.audio(&bytes).await,
			Some(Ok(Message::Text(text))) => self.text_message(&text).await,
			Some(Ok(Message::Ping(_) | Message::Pong(_))) => Step::Continue,
			Some(Ok(Message::Close(_)) | Err(_)) | None => Step::End(End::Gone),
		}
	}

	/// After `start`, the only text a client may send is `stop`.
	async fn text_message(&mut self, text: &str) -> Step {
		let problem = match ClientMessage::from_json(text) {
			Ok(ClientMessage::Stop) => return self.begin_finish().await,
			Ok(ClientMessage::Start { .. }) => "the session already started",
			Err(_) => "unreadable message",
		};

		self.fail(Fatal::new(ErrorCode::BadRequest, problem)).await
	}

	async fn audio(&mut self, bytes: &[u8]) -> Step {
		if bytes.len() > self.app.config.limits.max_frame_bytes {
			let fatal = Fatal::new(ErrorCode::Limit, "an audio frame is too large");

			return self
				.fail(Fatal {
					close: codes::TOO_BIG,
					..fatal
				})
				.await;
		}

		// Audio after `stop` is ignored: the provider has been told the stream
		// ended.
		if self.finishing.is_some() {
			return Step::Continue;
		}

		if !decode_pcm16(bytes, &mut self.samples) {
			let message = "audio frames must hold whole 16-bit samples";

			return self.fail(Fatal::new(ErrorCode::BadRequest, message)).await;
		}

		let now = Instant::now();
		let turn_ended = self.vad.push(&self.samples);
		let mut out = Outbox::default();
		self.last_audio = now;
		self.adapter.audio(&self.samples, &mut out);

		if turn_ended {
			self.adapter.end_of_turn(now, &mut out);
		}

		self.flush(out).await
	}

	async fn provider_message(
		&mut self,
		message: Option<Result<tungstenite::Message, tungstenite::Error>>,
	) -> Step {
		let mut out = Outbox::default();

		let result = match message {
			Some(Ok(tungstenite::Message::Text(text))) => self.adapter.receive(&text, &mut out),
			Some(Ok(tungstenite::Message::Close(_))) | None => {
				self.provider_open = false;
				self.adapter.closed()
			}
			Some(Ok(_)) => Ok(()),
			Some(Err(error)) => {
				self.provider_open = false;
				Err(ProviderError::Connect(error.to_string()))
			}
		};

		if let Err(error) = result {
			return self.fail(Fatal::provider(&error)).await;
		}

		self.flush(out).await
	}

	/// Send the adapter's messages to the provider and its events to the
	/// client.
	async fn flush(&mut self, out: Outbox) -> Step {
		for message in out.messages {
			if !self.provider_open {
				break;
			}

			if let Err(error) = self.provider.send(message).await {
				return self
					.fail(Fatal::provider(&ProviderError::Connect(error.to_string())))
					.await;
			}
		}

		for event in out.events {
			if let Step::End(end) = self.event(event).await {
				return Step::End(end);
			}
		}

		Step::Continue
	}

	async fn event(&mut self, event: Event) -> Step {
		match event {
			Event::Partial { segment, text } => {
				let draft = self.segments.partial(segment, text);
				self.send_all(draft).await
			}
			Event::Final { segment, text } => self.final_text(segment, text).await,
			Event::Warning(warning) => {
				tracing::warn!(%warning, "the speech provider lost a segment");
				let error = RelayMessage::error(
					ErrorCode::Provider,
					"part of the speech could not be transcribed",
					false,
				);
				self.send_all([error]).await
			}
		}
	}

	async fn final_text(&mut self, segment: u32, raw: String) -> Step {
		let Some(polisher) = self.polisher.clone().filter(|_| !raw.is_empty()) else {
			let finals = self.segments.finish(segment, raw);

			return self.send_all(finals).await;
		};

		let context = self.segments.context_before(segment, &self.context);
		let Some(draft) = self.segments.polishing(segment, raw.clone()) else {
			return Step::Continue;
		};

		self.polishing.push(Box::pin(async move {
			let result = polisher.polish(&raw, &context).await;
			(segment, raw, result)
		}));
		self.send_all([draft]).await
	}

	async fn polished(&mut self, (segment, raw, polished): PolishResult) -> Step {
		let (text, warning) = match polished {
			Ok(text) => (text, None),
			Err(error) => {
				tracing::warn!(%error, "polish failed; using the raw transcript");
				let message = format!("{error}; the raw transcript was used");
				(
					raw,
					Some(RelayMessage::error(ErrorCode::Polish, message, false)),
				)
			}
		};

		let finals = self.segments.finish(segment, text);
		self.send_all(warning.into_iter().chain(finals)).await
	}

	async fn limit(&mut self, message: String) -> Step {
		let error = RelayMessage::error(ErrorCode::Limit, message, false);

		if let Step::End(end) = self.send_all([error]).await {
			return Step::End(end);
		}

		self.begin_finish().await
	}

	/// Tell the provider no more audio is coming, and give it until the
	/// finish timeout to close every segment.
	async fn begin_finish(&mut self) -> Step {
		if self.finishing.is_some() {
			return Step::Continue;
		}

		self.finishing = Some(Instant::now() + self.app.config.limits.finish_timeout);
		let mut out = Outbox::default();
		self.adapter.finish(&mut out);
		self.flush(out).await
	}

	/// The provider ran out of time: close what it left open.
	async fn give_up(&mut self) -> Step {
		tracing::warn!("the speech provider did not finish in time");
		self.polishing.clear();
		let error = RelayMessage::error(
			ErrorCode::Provider,
			"the speech provider did not finish in time",
			false,
		);
		let finals = self.segments.force_close();

		if let Step::End(end) = self.send_all(std::iter::once(error).chain(finals)).await {
			return Step::End(end);
		}

		Step::End(self.done().await)
	}

	/// Once finishing and everything is final, send `done`.
	async fn end_if_finished(&mut self) -> Option<End> {
		let finished = self.adapter.is_finished() || !self.provider_open;

		if self.finishing.is_none() || !finished || !self.polishing.is_empty() {
			return None;
		}

		// A provider that finished without closing a segment it numbered
		// still owes the client a final for it.
		if !self.segments.is_settled() {
			tracing::warn!("the speech provider finished with segments open");
		}

		let finals = self.segments.force_close();

		if let Step::End(end) = self.send_all(finals).await {
			return Some(end);
		}

		Some(self.done().await)
	}

	async fn done(&mut self) -> End {
		if !self.client.send(&RelayMessage::Done).await {
			return End::Gone;
		}

		let code = if self.restarting {
			codes::RESTART
		} else {
			codes::NORMAL
		};

		self.client.close(code).await;

		if self.provider_open {
			let _ = self.provider.close(None).await;
		}

		End::Done
	}

	async fn fail(&mut self, fatal: Fatal) -> Step {
		let code = fatal.code;
		self.client.fail(fatal).await;

		if self.provider_open {
			let _ = self.provider.close(None).await;
		}

		Step::End(End::Failed(code))
	}

	async fn send_all(&mut self, messages: impl IntoIterator<Item = RelayMessage>) -> Step {
		for message in messages {
			if !self.client.send(&message).await {
				return Step::End(End::Gone);
			}
		}

		Step::Continue
	}
}
