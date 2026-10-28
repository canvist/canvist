//! Each provider adapter end to end: a client, the relay, and a fake
//! provider replaying scripted events.

mod support;

use std::time::Duration;

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use canvist_dictation::ClientMessage;
use canvist_dictation::RelayMessage;
use canvist_dictation_relay::config::ProviderKind;
use serde_json::Value;
use serde_json::json;
use support::*;

/// Client frames per scripted Deepgram result: 200 ms of audio.
const FRAMES_PER_RESULT: usize = 10;
/// The OpenAI Realtime API's sample rate.
const OPENAI_RATE: usize = 24_000;

// ---------------------------------------------------------------------------
// OpenAI
// ---------------------------------------------------------------------------

fn openai_event(kind: &str, item_id: &str, field: &str, text: &str) -> Reply {
	let mut event = json!({ "type": kind, "item_id": item_id });
	event[field] = json!(text);
	Reply::Send(event)
}

/// OpenAI's replies to the first commit, and to the second, which
/// completes before the first does.
fn openai_script() -> impl FnMut(&Incoming) -> Vec<Reply> + Send {
	let committed = |item_id| {
		Reply::Send(json!({ "type": "input_audio_buffer.committed", "item_id": item_id }))
	};
	let delta = "conversation.item.input_audio_transcription.delta";
	let completed = "conversation.item.input_audio_transcription.completed";
	let mut commits = 0;

	move |incoming| {
		if !is_type(incoming, "input_audio_buffer.commit") {
			return Vec::new();
		}

		commits += 1;

		if commits == 1 {
			return vec![committed("a"), openai_event(delta, "a", "delta", "hello")];
		}

		vec![
			committed("b"),
			openai_event(completed, "b", "transcript", "World."),
			Reply::Wait(Duration::from_millis(50)),
			openai_event(completed, "a", "transcript", "Hello."),
		]
	}
}

/// Samples of 24 kHz audio in every `input_audio_buffer.append`.
fn appended_samples(sent: &[Value]) -> usize {
	sent.iter()
		.filter(|value| value["type"] == "input_audio_buffer.append")
		.map(|value| {
			let audio = value["audio"].as_str().expect("audio");
			STANDARD.decode(audio).expect("base64").len() / 2
		})
		.sum()
}

#[tokio::test]
async fn openai_finals_follow_commit_order() {
	let provider = FakeProvider::start(openai_script()).await;
	let server = relay(provider.config(ProviderKind::OpenAi), |_| {}).await;
	let mut socket = connect(&server, Some(&token())).await;
	start(&mut socket, false).await;

	send_audio(&mut socket, &turn()).await;
	assert_eq!(next(&mut socket).await, Some(partial(0, "hello")));

	send_audio(&mut socket, &tone(500)).await;
	send(&mut socket, &ClientMessage::Stop).await;
	let (messages, code) = until_closed(&mut socket).await;

	assert_eq!(
		messages,
		[
			final_message(0, "Hello."),
			final_message(1, "World."),
			RelayMessage::Done,
		]
	);
	assert_eq!(code, NORMAL);

	assert_eq!(
		provider.received()[0],
		Incoming::Connected {
			uri: "/v1/realtime?intent=transcription".to_owned(),
			authorization: Some("Bearer provider-key".to_owned()),
		}
	);

	let input = &provider.json()[0]["session"]["audio"]["input"];
	assert_eq!(provider.json()[0]["session"]["type"], "transcription");
	assert_eq!(
		input["format"],
		json!({ "type": "audio/pcm", "rate": 24_000 })
	);
	assert_eq!(
		input["transcription"],
		json!({ "model": "gpt-live-transcribe", "language": "en" })
	);
	assert!(input["turn_detection"].is_null());

	// 2.5 seconds of 16 kHz audio reach OpenAI as 2.5 seconds at 24 kHz,
	// less the one sample interpolation waits on.
	let expected = OPENAI_RATE * 5 / 2;
	assert_eq!(appended_samples(&provider.json()), expected - 1);
}

// ---------------------------------------------------------------------------
// Deepgram
// ---------------------------------------------------------------------------

/// Deepgram's results as audio arrives, then on `Finalize` and
/// `CloseStream`.
fn deepgram_script() -> impl FnMut(&Incoming) -> Vec<Reply> + Send {
	let mut script = vec![
		deepgram_results("thanks", false),
		deepgram_results("The meeting is on Wednesday.", true),
		deepgram_results("the meeting is on Tuesday no Wednesday", false),
		deepgram_results("the meeting is on tuesday", false),
	];
	let mut frames = 0;

	move |incoming| {
		if matches!(incoming, Incoming::Binary(_)) {
			frames += 1;
			let due = frames % FRAMES_PER_RESULT == 0;
			return due.then(|| script.pop()).flatten().into_iter().collect();
		}

		if is_type(incoming, "Finalize") {
			return vec![deepgram_results("Thanks.", true)];
		}

		if is_type(incoming, "CloseStream") {
			return vec![Reply::Send(json!({ "type": "Metadata" })), Reply::Close];
		}

		Vec::new()
	}
}

#[tokio::test]
async fn deepgram_rewrites_drafts_and_flushes_on_stop() {
	let provider = FakeProvider::start(deepgram_script()).await;
	let server = relay(provider.config(ProviderKind::Deepgram), |_| {}).await;
	let mut socket = connect(&server, Some(&token())).await;
	start(&mut socket, false).await;

	send_audio(&mut socket, &tone(1_000)).await;
	send(&mut socket, &ClientMessage::Stop).await;
	let (messages, code) = until_closed(&mut socket).await;

	assert_eq!(
		messages,
		[
			partial(0, "the meeting is on tuesday"),
			partial(0, "the meeting is on Tuesday no Wednesday"),
			final_message(0, "The meeting is on Wednesday."),
			partial(1, "thanks"),
			final_message(1, "Thanks."),
			RelayMessage::Done,
		]
	);
	assert_eq!(code, NORMAL);

	assert_eq!(
		provider.received()[0],
		Incoming::Connected {
			uri: "/v1/listen?model=nova-3&encoding=linear16&sample_rate=16000&channels=1&\
			      interim_results=true&smart_format=true&punctuate=true&language=en"
				.to_owned(),
			authorization: Some("Token provider-key".to_owned()),
		}
	);

	let control: Vec<Value> = provider
		.json()
		.into_iter()
		.map(|value| value["type"].clone())
		.collect();
	assert_eq!(control, ["Finalize", "CloseStream"]);
}

// ---------------------------------------------------------------------------
// vLLM
// ---------------------------------------------------------------------------

fn vllm_delta(text: &str) -> Reply {
	Reply::Send(json!({ "type": "transcription.delta", "delta": text }))
}

/// Whether `incoming` is vLLM's `input_audio_buffer.commit` with `final: true`.
fn is_final_commit(incoming: &Incoming) -> bool {
	matches!(incoming, Incoming::Json(value) if value["type"] == "input_audio_buffer.commit" && value["final"] == true)
}

/// vLLM's deltas as audio arrives, then the last delta and `done` on the
/// final commit. The delta at 1.2 seconds, after the speech, arrives inside
/// the segment's grace period.
fn vllm_script() -> impl FnMut(&Incoming) -> Vec<Reply> + Send {
	let early_append = 5;
	let late_append = 60;
	let mut appends = 0;

	move |incoming| {
		if is_type(incoming, "input_audio_buffer.append") {
			appends += 1;

			return match appends {
				count if count == early_append => vec![vllm_delta(" the"), vllm_delta(" meeting")],
				count if count == late_append => vec![vllm_delta(" is on.")],
				_ => Vec::new(),
			};
		}

		if !is_final_commit(incoming) {
			return Vec::new();
		}

		let done = json!({ "type": "transcription.done", "text": "the meeting is on. Bye" });
		vec![vllm_delta(" Bye"), Reply::Send(done)]
	}
}

#[tokio::test]
async fn vllm_closes_segments_after_a_pause_and_its_grace() {
	let provider = FakeProvider::start(vllm_script()).await;
	let mut config = provider.config(ProviderKind::Vllm);
	config.key = None;
	config.segment_grace = Duration::from_millis(100);
	let server = relay(config, |_| {}).await;
	let mut socket = connect(&server, Some(&token())).await;
	start(&mut socket, false).await;

	send_audio(&mut socket, &turn()).await;

	for expected in [
		partial(0, "the"),
		partial(0, "the meeting"),
		partial(0, "the meeting is on."),
		final_message(0, "the meeting is on."),
	] {
		assert_eq!(next(&mut socket).await, Some(expected));
	}

	send(&mut socket, &ClientMessage::Stop).await;
	let (messages, code) = until_closed(&mut socket).await;

	assert_eq!(
		messages,
		[
			partial(1, "Bye"),
			final_message(1, "Bye"),
			RelayMessage::Done
		]
	);
	assert_eq!(code, NORMAL);

	assert_eq!(
		provider.received()[0],
		Incoming::Connected {
			uri: "/v1/realtime".to_owned(),
			authorization: None,
		}
	);
	assert_eq!(
		provider.json()[..2],
		[
			json!({ "type": "session.update", "model": "mistralai/Voxtral-Mini-4B-Realtime-2602" }),
			json!({ "type": "input_audio_buffer.commit" }),
		]
	);
}
