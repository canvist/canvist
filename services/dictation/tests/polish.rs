//! The polish pass end to end: a Deepgram fake supplies raw finals and a
//! fake model polishes them, slowly, wrongly, or not at all.

mod support;

use std::time::Duration;

use axum::http::StatusCode;
use canvist_dictation::ClientMessage;
use canvist_dictation::ErrorCode;
use canvist_dictation::RelayMessage;
use canvist_dictation_relay::config::PolishConfig;
use canvist_dictation_relay::config::ProviderKind;
use canvist_dictation_relay::config::Secret;
use canvist_dictation_relay::polish::SYSTEM_PROMPT;
use serde_json::Value;
use serde_json::json;
use support::*;

const RAW: &str = "um the meeting is on tuesday no wednesday";

/// Client frames per scripted Deepgram result: 200 ms of audio.
const FRAMES_PER_RESULT: usize = 10;

/// A Deepgram fake that finalizes `segments` in order, one per
/// [`FRAMES_PER_RESULT`] frames, and closes after `CloseStream`.
async fn deepgram(segments: &[&str]) -> FakeProvider {
	let finals = segments
		.iter()
		.map(|text| deepgram_results(text, true))
		.collect();
	scripted_deepgram(finals, FRAMES_PER_RESULT, true).await
}

/// Dictate `millis` of speech with polish on, stop, and collect the rest.
async fn dictate(
	provider: &FakeProvider,
	polish: PolishConfig,
	millis: u32,
	wants_polish: bool,
) -> Vec<RelayMessage> {
	let server = relay(provider.config(ProviderKind::Deepgram), |config| {
		config.polish = Some(polish);
	})
	.await;
	let mut socket = connect(&server, Some(&token())).await;
	start(&mut socket, wants_polish).await;
	send_audio(&mut socket, &tone(millis)).await;
	send(&mut socket, &ClientMessage::Stop).await;

	let (messages, code) = until_closed(&mut socket).await;
	assert_eq!(code, NORMAL);
	messages
}

fn anthropic(model: &FakePolish) -> PolishConfig {
	PolishConfig {
		url: model.url.clone(),
		..PolishConfig::anthropic(Secret::new("anthropic-key"))
	}
}

#[tokio::test]
async fn anthropic_polishes_each_final_with_context() {
	let model = FakePolish::start(|_| {
		(
			Duration::ZERO,
			StatusCode::OK,
			anthropic_reply("The meeting is on Wednesday."),
		)
	})
	.await;
	let provider = deepgram(&[RAW]).await;
	let messages = dictate(&provider, anthropic(&model), 200, true).await;

	assert_eq!(
		messages,
		[
			partial(0, RAW),
			final_message(0, "The meeting is on Wednesday."),
			RelayMessage::Done,
		]
	);

	let requests = model.received();
	assert_eq!(requests.len(), 1);
	let request = &requests[0];
	assert_eq!(request.headers["x-api-key"], "anthropic-key");
	assert_eq!(request.headers["anthropic-version"], "2023-06-01");
	assert_eq!(request.body["model"], "claude-haiku-5-5");
	assert_eq!(request.body["system"], SYSTEM_PROMPT);
	assert_eq!(request.body["output_config"], json!({ "effort": "low" }));
	assert!(request.body.get("temperature").is_none());
	assert_eq!(
		request.user_message(),
		format!("<context>\nNotes from Monday.\n</context>\n<transcript>\n{RAW}\n</transcript>")
	);
}

#[tokio::test]
async fn openai_compatible_servers_polish_too() {
	let model = FakePolish::start(|_| {
		let body = json!({
			"choices": [{ "message": { "role": "assistant", "content": "Polished." }, "finish_reason": "stop" }],
		});
		(Duration::ZERO, StatusCode::OK, body)
	})
	.await;
	let provider = deepgram(&["polish me"]).await;
	let polish = PolishConfig::openai(
		format!("{}/v1", model.url),
		"llama3.2",
		Some(Secret::new("local-key")),
	);
	let messages = dictate(&provider, polish, 200, true).await;

	assert_eq!(
		messages,
		[
			partial(0, "polish me"),
			final_message(0, "Polished."),
			RelayMessage::Done,
		]
	);

	let request = &model.received()[0];
	assert_eq!(request.headers["authorization"], "Bearer local-key");
	assert_eq!(request.body["model"], "llama3.2");
	assert_eq!(request.body["messages"][0]["role"], "system");
}

#[tokio::test]
async fn a_slow_model_falls_back_to_the_raw_transcript() {
	let model = FakePolish::start(|_| {
		(
			Duration::from_secs(2),
			StatusCode::OK,
			anthropic_reply("Too late."),
		)
	})
	.await;
	let provider = deepgram(&[RAW]).await;
	let polish = PolishConfig {
		timeout: Duration::from_millis(100),
		..anthropic(&model)
	};
	let messages = dictate(&provider, polish, 200, true).await;

	assert_eq!(messages.len(), 4, "{messages:?}");
	assert_eq!(messages[0], partial(0, RAW));
	assert!(matches!(
		&messages[1],
		RelayMessage::Error { code: ErrorCode::Polish, fatal: false, message } if message.contains("timed out")
	));
	assert_eq!(messages[2], final_message(0, RAW));
	assert_eq!(messages[3], RelayMessage::Done);
}

#[tokio::test]
async fn a_failing_or_inventive_model_falls_back_too() {
	let model = FakePolish::start(|request| {
		if request.user_message().contains("fails") {
			return (
				Duration::ZERO,
				StatusCode::INTERNAL_SERVER_ERROR,
				Value::Null,
			);
		}

		let essay = "Here is a much longer answer that the speaker never said, with extra detail.";
		(Duration::ZERO, StatusCode::OK, anthropic_reply(essay))
	})
	.await;
	let provider = deepgram(&["this fails", "what is the time"]).await;
	let messages = dictate(&provider, anthropic(&model), 400, true).await;

	let finals: Vec<_> = messages
		.iter()
		.filter(|message| matches!(message, RelayMessage::Final { .. }))
		.cloned()
		.collect();
	let polish_errors = messages
		.iter()
		.filter(|message| {
			matches!(
				message,
				RelayMessage::Error {
					code: ErrorCode::Polish,
					fatal: false,
					..
				}
			)
		})
		.count();

	assert_eq!(
		finals,
		[
			final_message(0, "this fails"),
			final_message(1, "what is the time"),
		]
	);
	assert_eq!(polish_errors, 2);
	assert_eq!(messages.last(), Some(&RelayMessage::Done));
}

#[tokio::test]
async fn finals_stay_in_order_when_an_earlier_polish_is_slower() {
	let model = FakePolish::start(|request| {
		let message = request.user_message();
		let (delay, text) = if message.contains("<transcript>\nfirst") {
			(Duration::from_millis(400), "First.")
		} else {
			(Duration::ZERO, "Second.")
		};
		(delay, StatusCode::OK, anthropic_reply(text))
	})
	.await;
	let provider = deepgram(&["first", "second"]).await;
	let messages = dictate(&provider, anthropic(&model), 400, true).await;

	assert_eq!(
		messages,
		[
			partial(0, "first"),
			partial(1, "second"),
			final_message(0, "First."),
			final_message(1, "Second."),
			RelayMessage::Done,
		]
	);

	// The second segment was polished with the first as context.
	let second = model
		.received()
		.into_iter()
		.find(|request| request.user_message().contains("<transcript>\nsecond"))
		.expect("the second segment was polished");
	assert!(second.user_message().contains("Notes from Monday. first"));
}

#[tokio::test]
async fn clients_can_turn_polish_off() {
	let model =
		FakePolish::start(|_| (Duration::ZERO, StatusCode::OK, anthropic_reply("No."))).await;
	let provider = deepgram(&[RAW]).await;
	let messages = dictate(&provider, anthropic(&model), 200, false).await;

	assert_eq!(messages, [final_message(0, RAW), RelayMessage::Done]);
	assert!(model.received().is_empty());
}
