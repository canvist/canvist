//! The batch a browser sends to `POST /v1/events`, and its validation.
//!
//! ```json
//! { "sent_at": 1767225600000, "events": [{
//!   "uuid": "0193…", "name": "$pageview", "visitor": "…", "session": "…",
//!   "timestamp": 1767225599000, "path": "/pricing", "source": "news.ycombinator.com",
//!   "properties": { "plan": "builder" }
//! }] }
//! ```
//!
//! The endpoint is public, so every field is checked and anything invalid
//! is dropped one event at a time rather than failing the batch.
//! Timestamps are moved by the gap between the sender's clock (`sent_at`)
//! and the server's, so a skewed clock does not move events in time.

use serde_json::Map;
use serde_json::Value as Json;

use super::Agent;
use super::Event;

/// The largest batch body accepted, in bytes.
pub const MAX_BATCH_BYTES: usize = 64 * 1024;
/// Events read from one batch; the rest are dropped.
const MAX_EVENTS: usize = 50;
const MAX_NAME: usize = 100;
const MAX_ID: usize = 64;
const MAX_PATH: usize = 300;
const MAX_SOURCE: usize = 100;
const MAX_PROPERTIES: usize = 20;
const MAX_KEY: usize = 50;
const MAX_VALUE: usize = 200;
/// Older events are dropped: a browser offline for a week sends history,
/// not activity.
const MAX_AGE_MILLIS: i64 = 7 * 24 * 60 * 60 * 1000;

/// A validated batch.
#[derive(Debug, PartialEq)]
pub struct Batch {
	/// The events to store.
	pub events: Vec<Event>,
	/// Events that were invalid, too old, or past [`MAX_EVENTS`].
	pub dropped: usize,
}

impl Batch {
	/// Validate `body`, sent by a browser described by `agent`, as received
	/// at `received_at` (Unix milliseconds). `None` when the body is not a
	/// batch at all.
	pub fn parse(body: &[u8], agent: &Agent, received_at: i64) -> Option<Self> {
		let batch: Json = serde_json::from_slice(body).ok()?;
		let raw = batch.get("events")?.as_array()?;
		let skew = batch
			.get("sent_at")
			.and_then(Json::as_i64)
			.map_or(0, |sent_at| received_at - sent_at);
		let events: Vec<Event> = raw
			.iter()
			.take(MAX_EVENTS)
			.filter_map(|event| parse_event(event, agent, skew, received_at))
			.collect();

		Some(Self {
			dropped: raw.len() - events.len(),
			events,
		})
	}
}

fn parse_event(raw: &Json, agent: &Agent, skew: i64, received_at: i64) -> Option<Event> {
	let at = raw.get("timestamp")?.as_i64()?.saturating_add(skew);

	if at < received_at - MAX_AGE_MILLIS {
		return None;
	}

	Some(Event {
		uuid: id(raw.get("uuid"))?,
		name: text(raw.get("name"), MAX_NAME)?,
		visitor: id(raw.get("visitor"))?,
		session: Some(id(raw.get("session"))?),
		// An event from the future happened now.
		at: at.min(received_at),
		path: Some(path(raw.get("path"))?),
		source: raw.get("source").and_then(source),
		device: agent.device,
		browser: Some(agent.browser.to_owned()),
		os: Some(agent.os.to_owned()),
		properties: raw.get("properties").map(properties).unwrap_or_default(),
	})
}

/// Trimmed text of 1 to `max` characters.
fn text(value: Option<&Json>, max: usize) -> Option<String> {
	let text = value?.as_str()?.trim();
	(!text.is_empty() && text.chars().count() <= max).then(|| text.to_owned())
}

/// An id the sender made: letters, digits, `-` and `_`.
fn id(value: Option<&Json>) -> Option<String> {
	text(value, MAX_ID).filter(|id| {
		id.chars()
			.all(|character| character.is_ascii_alphanumeric() || matches!(character, '-' | '_'))
	})
}

/// A path without its query string or fragment.
fn path(value: Option<&Json>) -> Option<String> {
	let path = text(value, MAX_PATH)?;
	let end = path.find(['?', '#']).unwrap_or(path.len());
	let path = &path[..end];
	path.starts_with('/').then(|| path.to_owned())
}

/// A host or campaign name, lowercased, without `www.`.
fn source(value: &Json) -> Option<String> {
	let source = text(Some(value), MAX_SOURCE)?.to_ascii_lowercase();
	let source = source.strip_prefix("www.").unwrap_or(&source);
	source
		.chars()
		.all(|character| character.is_ascii_alphanumeric() || matches!(character, '.' | '-' | '_'))
		.then(|| source.to_owned())
}

/// Flat properties: strings (cut to [`MAX_VALUE`] characters), finite
/// numbers, and booleans, at most [`MAX_PROPERTIES`] of them.
fn properties(value: &Json) -> Map<String, Json> {
	let Some(object) = value.as_object() else {
		return Map::new();
	};

	object
		.iter()
		.filter(|(key, _)| !key.is_empty() && key.chars().count() <= MAX_KEY)
		.filter_map(|(key, value)| {
			let value = match value {
				Json::String(text) => Json::String(text.chars().take(MAX_VALUE).collect()),
				Json::Number(number) if number.as_f64().is_some_and(f64::is_finite) => {
					value.clone()
				}
				Json::Bool(_) => value.clone(),
				_ => return None,
			};
			Some((key.clone(), value))
		})
		.take(MAX_PROPERTIES)
		.collect()
}

#[cfg(test)]
mod tests {
	use serde_json::json;

	use super::*;
	use crate::analytics::Device;

	const NOW: i64 = 1_767_225_600_000;

	fn agent() -> Agent {
		Agent {
			device: Device::Mobile,
			browser: "Safari",
			os: "iOS",
		}
	}

	fn event(overrides: &Json) -> Json {
		let mut event = json!({
			"uuid": "e1",
			"name": "$pageview",
			"visitor": "v1",
			"session": "s1",
			"timestamp": NOW - 1_000,
			"path": "/pricing",
		});
		if let (Some(event), Some(overrides)) = (event.as_object_mut(), overrides.as_object()) {
			event.extend(overrides.clone());
		}
		event
	}

	fn parse(batch: &Json) -> Batch {
		let body = serde_json::to_vec(batch).expect("serializes");
		Batch::parse(&body, &agent(), NOW).expect("a batch")
	}

	#[test]
	fn valid_events_carry_the_agent_and_lose_their_query_strings() {
		let batch = parse(&json!({ "events": [event(&json!({
			"path": "/docs?token=secret#top",
			"source": "WWW.News.Ycombinator.com",
			"properties": { "plan": "builder", "seats": 3, "nested": { "no": 1 } },
		}))] }));

		assert_eq!(batch.dropped, 0);
		let event = &batch.events[0];
		assert_eq!(event.path.as_deref(), Some("/docs"));
		assert_eq!(event.source.as_deref(), Some("news.ycombinator.com"));
		assert_eq!(event.device, Device::Mobile);
		assert_eq!(event.browser.as_deref(), Some("Safari"));
		assert_eq!(
			Json::Object(event.properties.clone()),
			json!({ "plan": "builder", "seats": 3 })
		);
	}

	#[test]
	fn invalid_events_are_dropped_one_by_one() {
		let batch = parse(&json!({ "events": [
			event(&json!({})),
			event(&json!({ "uuid": "has spaces" })),
			event(&json!({ "path": "no-slash" })),
			event(&json!({ "name": "" })),
			event(&json!({ "timestamp": NOW - MAX_AGE_MILLIS - 1 })),
			event(&json!({ "session": null })),
			"not an event",
		] }));

		assert_eq!(batch.events.len(), 1);
		assert_eq!(batch.dropped, 6);
	}

	#[test]
	fn skewed_clocks_are_corrected_and_the_future_is_now() {
		// The sender's clock runs an hour behind.
		let hour = 3_600_000;
		let batch = parse(&json!({
			"sent_at": NOW - hour,
			"events": [event(&json!({ "timestamp": NOW - hour - 5_000 }))],
		}));
		assert_eq!(batch.events[0].at, NOW - 5_000);

		let batch = parse(&json!({ "events": [event(&json!({ "timestamp": NOW + hour }))] }));
		assert_eq!(batch.events[0].at, NOW);
	}

	#[test]
	fn batches_are_capped() {
		let events: Vec<Json> = (0..MAX_EVENTS + 5)
			.map(|index| event(&json!({ "uuid": format!("e{index}") })))
			.collect();
		let batch = parse(&json!({ "events": events }));
		assert_eq!(batch.events.len(), MAX_EVENTS);
		assert_eq!(batch.dropped, 5);
	}

	#[test]
	fn bodies_that_are_not_batches_are_refused() {
		assert_eq!(Batch::parse(b"nope", &agent(), NOW), None);
		assert_eq!(Batch::parse(b"{}", &agent(), NOW), None);
	}
}
