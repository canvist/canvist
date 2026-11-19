//! First-party product analytics: page views and clicks from canvist.dev,
//! and events the server records itself (a project created, a checkout
//! started), in canvist's own database.
//!
//! - **Private by construction.** IP addresses are never read, query strings
//!   never arrive, and a visitor is a random id the browser keeps. The site
//!   sends nothing when the browser asks not to be tracked.
//! - **Bots are dropped** at the endpoint, by user agent.
//! - **Reports read raw events**, at most [`SCAN_LIMIT`] of them, and say
//!   when they stopped short ([`report`]). Aggregation is pure, so the memory
//!   and Postgres stores answer the same way.
//!
//! The browser half is `website/src/scripts/analytics.ts`.

mod agent;
pub mod report;
mod wire;

pub use agent::Agent;
pub use agent::Device;
pub use report::Report;
use serde_json::Map;
use serde_json::Value as Json;
pub use wire::Batch;
pub use wire::MAX_BATCH_BYTES;

use crate::store::Store;

/// The most events one report reads.
pub const SCAN_LIMIT: usize = 200_000;

/// The event name of a page view.
pub const PAGEVIEW: &str = "$pageview";

/// One stored event.
#[derive(Clone, Debug, PartialEq)]
pub struct Event {
	/// Made by the sender, so a batch sent twice is stored once.
	pub uuid: String,
	/// `$pageview`, or a name like `checkout started`.
	pub name: String,
	/// An anonymous browser id, or `project:<id>` for server events.
	pub visitor: String,
	/// The browser session; `None` for server events.
	pub session: Option<String>,
	/// When it happened, in Unix milliseconds.
	pub at: i64,
	/// The page's path, without its query string.
	pub path: Option<String>,
	/// Where the visit came from: a referring host or a `utm_source`.
	pub source: Option<String>,
	/// The kind of device, or [`Device::Server`].
	pub device: Device,
	/// The browser family.
	pub browser: Option<String>,
	/// The operating system.
	pub os: Option<String>,
	/// Flat properties: strings, numbers, and booleans.
	pub properties: Map<String, Json>,
}

/// Record an event the server saw, such as `project created`. A failure is
/// logged, never passed on: analytics must not break what it describes.
pub async fn capture(store: &Store, name: &str, visitor: &str, properties: Map<String, Json>) {
	let event = Event {
		uuid: crate::names::random_token(32),
		name: name.to_owned(),
		visitor: visitor.to_owned(),
		session: None,
		at: unix_millis(),
		path: None,
		source: None,
		device: Device::Server,
		browser: None,
		os: None,
		properties,
	};

	if let Err(error) = store.insert_events(&[event]).await {
		tracing::warn!(name, %error, "could not record an analytics event");
	}
}

/// The current time in Unix milliseconds.
pub fn unix_millis() -> i64 {
	std::time::SystemTime::now()
		.duration_since(std::time::UNIX_EPOCH)
		.map_or(0, |elapsed| {
			i64::try_from(elapsed.as_millis()).unwrap_or(i64::MAX)
		})
}
