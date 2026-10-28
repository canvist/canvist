//! Authentication, protocol errors, limits, and shutdown, over a real
//! socket.

mod support;

use std::time::Duration;

use canvist_dictation::ClientMessage;
use canvist_dictation::ErrorCode;
use canvist_dictation::RelayMessage;
use canvist_dictation_relay::auth::Claims;
use canvist_dictation_relay::auth::unix_now;
use canvist_dictation_relay::config::ProviderConfig;
use canvist_dictation_relay::config::ProviderKind;
use futures_util::SinkExt;
use futures_util::StreamExt;
use support::*;
use tokio_tungstenite::tungstenite::Message;

/// A Deepgram fake that drafts after 200 ms of audio, and closes after
/// `CloseStream` unless `answer_stop` is false.
async fn deepgram(answer_stop: bool) -> FakeProvider {
	let draft = deepgram_results("half a thought", false);
	scripted_deepgram(vec![draft], 10, answer_stop).await
}

/// Assert the session ends with one fatal error of `code`.
async fn assert_fatal(socket: &mut Socket, code: ErrorCode, close: u16) {
	let (messages, close_code) = until_closed(socket).await;

	assert!(
		matches!(
			messages.as_slice(),
			[RelayMessage::Error { code: actual, fatal: true, .. }] if *actual == code
		),
		"{messages:?}"
	);
	assert_eq!(close_code, Some(close));
}

#[tokio::test]
async fn missing_and_bad_tokens_are_unauthorized() {
	let provider = deepgram(true).await;
	let server = relay(provider.config(ProviderKind::Deepgram), |_| {}).await;
	let expired = sign(&Claims {
		exp: unix_now() - 60,
		..Claims::new("user-1", unix_now() - 120, Duration::ZERO)
	});
	let wrong_scope = sign(&Claims {
		scope: "write".to_owned(),
		..Claims::new("user-1", unix_now(), Duration::from_mins(5))
	});

	for token in [
		None,
		Some("not-a-token"),
		Some(&expired[..]),
		Some(&wrong_scope[..]),
	] {
		let mut socket = connect(&server, token).await;
		assert_fatal(&mut socket, ErrorCode::Unauthorized, 1008).await;
	}

	assert!(
		provider.received().is_empty(),
		"no provider session was opened"
	);
}

#[tokio::test]
async fn browsers_pass_the_token_in_the_query() {
	let provider = deepgram(true).await;
	let server = relay(provider.config(ProviderKind::Deepgram), |_| {}).await;
	let mut socket = connect_with_query(&server, &token()).await;
	start(&mut socket, true).await;
	send(&mut socket, &ClientMessage::Stop).await;

	let (messages, code) = until_closed(&mut socket).await;
	assert_eq!(messages, [RelayMessage::Done]);
	assert_eq!(code, NORMAL);
}

#[tokio::test]
async fn health_is_reported() {
	let provider = deepgram(true).await;
	let server = relay(provider.config(ProviderKind::Deepgram), |_| {}).await;
	let response = reqwest::get(format!("http://{}/healthz", server.addr()))
		.await
		.expect("responds");

	assert_eq!(response.status(), 200);
}

#[tokio::test]
async fn a_malformed_first_message_is_a_bad_request() {
	let provider = deepgram(true).await;
	let server = relay(provider.config(ProviderKind::Deepgram), |_| {}).await;
	let first_messages = [
		Message::text("hello"),
		Message::text(r#"{"type":"stop"}"#),
		Message::text(r#"{"type":"start","language":"en&model=other"}"#),
		Message::binary(vec![0; 640]),
	];

	for message in first_messages {
		let mut socket = connect(&server, Some(&token())).await;
		socket.send(message).await.expect("sends");
		assert_fatal(&mut socket, ErrorCode::BadRequest, 1007).await;
	}
}

#[tokio::test]
async fn protocol_errors_after_start_are_bad_requests() {
	let provider = deepgram(true).await;
	let server = relay(provider.config(ProviderKind::Deepgram), |_| {}).await;

	let mut socket = connect(&server, Some(&token())).await;
	start(&mut socket, false).await;
	socket
		.send(Message::binary(vec![0; 3]))
		.await
		.expect("sends");
	assert_fatal(&mut socket, ErrorCode::BadRequest, 1007).await;

	let mut socket = connect(&server, Some(&token())).await;
	start(&mut socket, false).await;
	send(&mut socket, &ClientMessage::start()).await;
	assert_fatal(&mut socket, ErrorCode::BadRequest, 1007).await;
}

#[tokio::test]
async fn oversized_frames_hit_the_limit() {
	let provider = deepgram(true).await;
	let server = relay(provider.config(ProviderKind::Deepgram), |config| {
		config.limits.max_frame_bytes = 1_024;
	})
	.await;
	let mut socket = connect(&server, Some(&token())).await;
	start(&mut socket, false).await;
	socket
		.send(Message::binary(vec![0; 2_048]))
		.await
		.expect("sends");

	assert_fatal(&mut socket, ErrorCode::Limit, 1009).await;
}

#[tokio::test]
async fn a_client_that_never_starts_is_dropped() {
	let provider = deepgram(true).await;
	let server = relay(provider.config(ProviderKind::Deepgram), |config| {
		config.limits.idle = Duration::from_millis(200);
	})
	.await;
	let mut socket = connect(&server, Some(&token())).await;

	assert_fatal(&mut socket, ErrorCode::Limit, 1008).await;
}

#[tokio::test]
async fn idle_and_long_sessions_finish_with_a_limit_error() {
	let provider = deepgram(true).await;
	let server = relay(provider.config(ProviderKind::Deepgram), |config| {
		config.limits.idle = Duration::from_millis(300);
		config.limits.max_session = Duration::from_millis(600);
	})
	.await;

	// Idle: audio stops, the session finishes on its own.
	let mut socket = connect(&server, Some(&token())).await;
	start(&mut socket, false).await;
	send_audio(&mut socket, &tone(100)).await;
	let (messages, code) = until_closed(&mut socket).await;
	assert!(
		matches!(
			messages.as_slice(),
			[RelayMessage::Error { code: ErrorCode::Limit, fatal: false, message }, RelayMessage::Done]
				if message.contains("no audio")
		),
		"{messages:?}"
	);
	assert_eq!(code, NORMAL);

	// Too long: audio keeps coming past the duration limit.
	let mut socket = connect(&server, Some(&token())).await;
	start(&mut socket, false).await;
	let (mut sink, mut stream) = socket.split();
	tokio::spawn(async move {
		let frame = canvist_dictation_relay::audio::encode_pcm16(&silence(20));

		while sink.send(Message::binary(frame.clone())).await.is_ok() {
			tokio::time::sleep(Duration::from_millis(20)).await;
		}
	});
	let (messages, _) = until_closed(&mut stream).await;
	assert!(
		messages.iter().any(|message| matches!(
			message,
			RelayMessage::Error { code: ErrorCode::Limit, fatal: false, message } if message.contains("limit")
		)),
		"{messages:?}"
	);
	assert_eq!(messages.last(), Some(&RelayMessage::Done));
}

#[tokio::test]
async fn an_unreachable_provider_is_a_provider_error() {
	// Nothing listens on port 9 of localhost.
	let provider = ProviderConfig {
		url: "ws://127.0.0.1:9".to_owned(),
		..ProviderConfig::new(ProviderKind::Vllm, None)
	};
	let server = relay(provider, |_| {}).await;
	let mut socket = connect(&server, Some(&token())).await;
	send(&mut socket, &ClientMessage::start()).await;

	assert_fatal(&mut socket, ErrorCode::Provider, 1011).await;
}

#[tokio::test]
async fn a_provider_that_drops_mid_session_is_fatal() {
	let provider = FakeProvider::start(|incoming| {
		match incoming {
			Incoming::Binary(_) => vec![Reply::Close],
			_ => Vec::new(),
		}
	})
	.await;
	let server = relay(provider.config(ProviderKind::Deepgram), |_| {}).await;
	let mut socket = connect(&server, Some(&token())).await;
	start(&mut socket, false).await;
	send_audio(&mut socket, &tone(20)).await;

	assert_fatal(&mut socket, ErrorCode::Provider, 1011).await;
}

#[tokio::test]
async fn a_provider_that_never_finishes_has_its_drafts_finalized() {
	let provider = deepgram(false).await;
	let server = relay(provider.config(ProviderKind::Deepgram), |config| {
		config.limits.finish_timeout = Duration::from_millis(300);
	})
	.await;
	let mut socket = connect(&server, Some(&token())).await;
	start(&mut socket, false).await;
	send_audio(&mut socket, &tone(400)).await;
	assert_eq!(next(&mut socket).await, Some(partial(0, "half a thought")));

	send(&mut socket, &ClientMessage::Stop).await;
	let (messages, code) = until_closed(&mut socket).await;

	assert!(
		matches!(
			&messages[0],
			RelayMessage::Error {
				code: ErrorCode::Provider,
				fatal: false,
				..
			}
		),
		"{messages:?}"
	);
	assert_eq!(
		messages[1..],
		[final_message(0, "half a thought"), RelayMessage::Done]
	);
	assert_eq!(code, NORMAL);
}

#[tokio::test]
async fn shutdown_finishes_open_sessions_and_asks_clients_to_reconnect() {
	let provider = deepgram(true).await;
	let server = relay(provider.config(ProviderKind::Deepgram), |_| {}).await;
	let addr = server.addr();
	let mut socket = connect(&server, Some(&token())).await;
	start(&mut socket, false).await;
	send_audio(&mut socket, &tone(400)).await;
	assert_eq!(next(&mut socket).await, Some(partial(0, "half a thought")));

	let shutdown = tokio::spawn(server.shutdown());
	let (messages, code) = until_closed(&mut socket).await;
	shutdown.await.expect("shuts down");

	assert_eq!(
		messages,
		[final_message(0, "half a thought"), RelayMessage::Done]
	);
	assert_eq!(code, Some(1012));
	assert!(
		tokio::net::TcpStream::connect(addr).await.is_err(),
		"the listener is closed"
	);
}

#[tokio::test]
async fn shutdown_turns_away_clients_that_have_not_started() {
	let provider = deepgram(true).await;
	let server = relay(provider.config(ProviderKind::Deepgram), |_| {}).await;
	let mut socket = connect(&server, Some(&token())).await;

	let shutdown = tokio::spawn(server.shutdown());
	assert_fatal(&mut socket, ErrorCode::Internal, 1012).await;
	shutdown.await.expect("shuts down");
}
