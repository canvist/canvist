//! Debounced `document.updated` webhooks.
//!
//! Rooms collect changes; the flush loop hands them here once a document has
//! been quiet for the debounce period, or has kept changing for the maximum
//! wait. Each delivery is a signed JSON `POST`, retried three times with
//! backoff and then dropped: webhooks are a hint to re-read the document,
//! not a log of every change.
//!
//! The body is signed like Stripe's: the `Canvist-Signature` header is
//! `t=<unix seconds>,v1=<hex HMAC-SHA256 of "<t>.<body>">`, keyed by the
//! webhook secret.

use std::net::IpAddr;
use std::net::Ipv4Addr;
use std::net::Ipv6Addr;
use std::sync::Arc;
use std::time::Duration;
use std::time::Instant;
use std::time::SystemTime;
use std::time::UNIX_EPOCH;

use hmac::Hmac;
use hmac::Mac;
use reqwest::dns::Addrs;
use reqwest::dns::Name;
use reqwest::dns::Resolve;
use reqwest::dns::Resolving;
use serde::Serialize;
use sha2::Sha256;
use tokio::sync::Semaphore;
use tokio::sync::mpsc;
use tokio::sync::watch;
use tokio::task::JoinHandle;

use super::room::Changes;
use crate::names::DocumentKey;
use crate::names::random_token;
use crate::store::Store;

/// Deliveries waiting before new ones are dropped.
const QUEUE: usize = 4096;
/// Deliveries in flight at once.
const CONCURRENCY: usize = 32;
/// Attempts per delivery.
const ATTEMPTS: u32 = 3;
const TIMEOUT: Duration = Duration::from_secs(10);
/// Random characters in an event id: 80 bits.
const EVENT_ID_LENGTH: usize = 16;

/// Queues webhook deliveries.
#[derive(Clone, Debug)]
pub struct Webhooks {
	sender: mpsc::Sender<Event>,
}

#[derive(Debug)]
struct Event {
	key: DocumentKey,
	payload: Payload,
}

/// The JSON body of a `document.updated` webhook.
#[derive(Clone, Debug, Serialize)]
struct Payload {
	#[serde(rename = "type")]
	kind: &'static str,
	id: String,
	project: String,
	environment: String,
	document: String,
	editors: Vec<String>,
	first_change_at: u64,
	last_change_at: u64,
}

/// The webhook worker, to wait for on shutdown.
#[derive(Debug)]
pub struct WebhookWorker {
	handle: JoinHandle<()>,
	stop: watch::Sender<bool>,
}

impl WebhookWorker {
	/// Stop accepting deliveries and wait up to `deadline` for the queued
	/// ones to finish.
	pub async fn drain(self, deadline: Duration) {
		let _ = self.stop.send(true);

		if tokio::time::timeout(deadline, self.handle).await.is_err() {
			tracing::warn!("webhook deliveries still running at shutdown were dropped");
		}
	}
}

impl Webhooks {
	/// Start the delivery worker.
	pub fn start(store: Store, allow_private: bool) -> (Self, WebhookWorker) {
		let (sender, receiver) = mpsc::channel(QUEUE);
		let client = DeliveryClient {
			http: client_builder(allow_private)
				.build()
				.expect("the webhook client configuration is valid"),
			allow_private,
		};
		let (stop, stopped) = watch::channel(false);
		let handle = tokio::spawn(run(store, client, receiver, stopped));
		(Self { sender }, WebhookWorker { handle, stop })
	}

	/// Queue a webhook for `changes` to `key`, if its project environment
	/// has one configured.
	pub fn notify(&self, key: &DocumentKey, changes: &Changes) {
		let now = Instant::now();
		let wall_clock =
			|instant: Instant| unix_millis(SystemTime::now() - now.duration_since(instant));
		let event = Event {
			key: key.clone(),
			payload: Payload {
				kind: "document.updated",
				id: format!("evt_{}", random_token(EVENT_ID_LENGTH)),
				project: key.project.to_string(),
				environment: key.environment.to_string(),
				document: key.name.to_string(),
				editors: changes.editors.iter().map(ToString::to_string).collect(),
				first_change_at: wall_clock(changes.first),
				last_change_at: wall_clock(changes.last),
			},
		};

		if self.sender.try_send(event).is_err() {
			tracing::warn!(document = %key, "webhook queue full; dropping a delivery");
		}
	}
}

/// Keep the URL policy with the connection policy, including for stored URLs.
#[derive(Clone)]
struct DeliveryClient {
	http: reqwest::Client,
	allow_private: bool,
}

/// DNS is checked by the connector itself, so it cannot change between the
/// address check and the connection. The URL keeps its original TLS/SNI host.
fn client_builder(allow_private: bool) -> reqwest::ClientBuilder {
	let builder = reqwest::Client::builder()
		.timeout(TIMEOUT)
		.redirect(reqwest::redirect::Policy::none())
		.https_only(!allow_private)
		// A proxy could resolve the destination without our address checks.
		.no_proxy()
		.user_agent("canvist-cloud-webhooks");

	if allow_private {
		builder
	} else {
		builder.dns_resolver(Arc::new(PublicResolver(SystemResolver)))
	}
}

/// Resolve through the operating system, without caching across connections.
struct SystemResolver;

impl Resolve for SystemResolver {
	fn resolve(&self, name: Name) -> Resolving {
		Box::pin(async move {
			let addresses = tokio::net::lookup_host((name.as_str().to_owned(), 0)).await?;
			Ok(Box::new(addresses) as Addrs)
		})
	}
}

/// Reject the whole DNS answer if any address is non-public. Return exactly
/// the checked addresses to reqwest; it must never resolve the name again.
struct PublicResolver<R>(R);

impl<R: Resolve> Resolve for PublicResolver<R> {
	fn resolve(&self, name: Name) -> Resolving {
		let resolving = self.0.resolve(name);

		Box::pin(async move {
			let addresses: Vec<_> = resolving.await?.collect();

			if addresses.is_empty() || addresses.iter().any(|address| !is_public(address.ip())) {
				return Err(InvalidUrl("host must resolve only to public addresses").into());
			}

			Ok(Box::new(addresses.into_iter()) as Addrs)
		})
	}
}

/// Receive events until stopped, delivering up to [`CONCURRENCY`] at once.
/// Once stopped, the queue is closed but drained, and in-flight deliveries
/// finish, so shutdown loses nothing that was already queued.
async fn run(
	store: Store,
	client: DeliveryClient,
	mut receiver: mpsc::Receiver<Event>,
	mut stopped: watch::Receiver<bool>,
) {
	let permits = Arc::new(Semaphore::new(CONCURRENCY));
	let mut deliveries = tokio::task::JoinSet::new();
	let mut stopping = false;

	loop {
		let event = tokio::select! {
			event = receiver.recv() => event,
			_ = stopped.changed(), if !stopping => {
				stopping = true;
				receiver.close();
				continue;
			}
		};
		let Some(event) = event else {
			break;
		};
		let Ok(permit) = permits.clone().acquire_owned().await else {
			break;
		};
		let store = store.clone();
		let client = client.clone();

		deliveries.spawn(async move {
			deliver(&store, &client, event).await;
			drop(permit);
		});

		while deliveries.try_join_next().is_some() {}
	}

	while deliveries.join_next().await.is_some() {}
}

/// Deliver `event` to its environment's webhook, if one is configured.
async fn deliver(store: &Store, client: &DeliveryClient, event: Event) {
	let configured = store
		.webhook(&event.key.project, &event.key.environment)
		.await;
	let webhook = match configured {
		Ok(Some(webhook)) => webhook,
		Ok(None) => return,
		Err(error) => {
			tracing::warn!(document = %event.key, %error, "could not read the webhook configuration");
			return;
		}
	};
	let body = serde_json::to_string(&event.payload).expect("payloads always serialize");

	for attempt in 0..ATTEMPTS {
		if attempt > 0 {
			tokio::time::sleep(Duration::from_secs(4_u64.pow(attempt - 1))).await;
		}

		if attempt_delivery(client, &webhook, &event, &body, attempt).await {
			return;
		}
	}

	tracing::warn!(document = %event.key, "webhook dropped after {ATTEMPTS} attempts");
}

/// One signed `POST`; returns whether the endpoint accepted it.
async fn attempt_delivery(
	client: &DeliveryClient,
	webhook: &crate::store::Webhook,
	event: &Event,
	body: &str,
	attempt: u32,
) -> bool {
	// Literal IPs bypass DNS in reqwest, and persisted URLs may predate the
	// current policy. Check both before every signed request.
	if let Err(error) = parse_url(&webhook.url, client.allow_private) {
		tracing::info!(document = %event.key, %error, attempt, "webhook destination refused");
		return false;
	}

	let timestamp = unix_millis(SystemTime::now()) / 1000;
	let response = client
		.http
		.post(&webhook.url)
		.header("content-type", "application/json")
		.header("canvist-event", event.payload.kind)
		.header("canvist-signature", sign(&webhook.secret, timestamp, body))
		.body(body.to_owned())
		.send()
		.await;

	match response {
		Ok(response) if response.status().is_success() => true,
		Ok(response) => {
			tracing::info!(document = %event.key, status = %response.status(), attempt, "webhook refused");
			false
		}
		Err(error) => {
			tracing::info!(document = %event.key, %error, attempt, "webhook failed");
			false
		}
	}
}

/// The `Canvist-Signature` header for `body` sent at `timestamp`.
pub fn sign(secret: &str, timestamp: u64, body: &str) -> String {
	let mut mac =
		Hmac::<Sha256>::new_from_slice(secret.as_bytes()).expect("HMAC accepts keys of any length");
	mac.update(format!("{timestamp}.{body}").as_bytes());
	format!("t={timestamp},v1={}", hex(&mac.finalize().into_bytes()))
}

/// Lowercase hex.
pub(crate) fn hex(bytes: &[u8]) -> String {
	use std::fmt::Write;

	bytes.iter().fold(String::new(), |mut out, byte| {
		let _ = write!(out, "{byte:02x}");
		out
	})
}

fn unix_millis(time: SystemTime) -> u64 {
	time.duration_since(UNIX_EPOCH)
		.map_or(0, |elapsed| elapsed.as_millis() as u64)
}

/// Why a webhook URL was refused.
#[derive(Debug, PartialEq, Eq, thiserror::Error)]
#[error("invalid webhook url: {0}")]
pub struct InvalidUrl(&'static str);

/// Check that `url` is an HTTPS URL on a public host. With `allow_private`
/// (for tests and self-hosting), plain HTTP and private hosts are allowed.
///
/// DNS is checked at admission and again by the delivery connector.
pub async fn validate_url(url: &str, allow_private: bool) -> Result<(), InvalidUrl> {
	let parsed = parse_url(url, allow_private)?;
	let host = parsed.host_str().ok_or(InvalidUrl("missing host"))?;

	if allow_private || host.trim_matches(['[', ']']).parse::<IpAddr>().is_ok() {
		return Ok(());
	}

	let name = host.parse().map_err(|_| InvalidUrl("invalid host"))?;
	drop(
		tokio::time::timeout(TIMEOUT, PublicResolver(SystemResolver).resolve(name))
			.await
			.map_err(|_| InvalidUrl("DNS lookup timed out"))?
			.map_err(|_| InvalidUrl("host must resolve only to public addresses"))?,
	);

	Ok(())
}

/// Enforce scheme and literal-host policy even when DNS is bypassed.
fn parse_url(url: &str, allow_private: bool) -> Result<reqwest::Url, InvalidUrl> {
	let parsed = reqwest::Url::parse(url).map_err(|_| InvalidUrl("not a URL"))?;

	match (parsed.scheme(), allow_private) {
		("http" | "https", true) => return Ok(parsed),
		(_, true) => return Err(InvalidUrl("use http or https")),
		("https", false) => {}
		(_, false) => return Err(InvalidUrl("use https")),
	}

	let host = parsed.host_str().ok_or(InvalidUrl("missing host"))?;
	check_public_host(host.trim_matches(['[', ']']))?;

	Ok(parsed)
}

/// Refuse loopback, private, and link-local addresses and internal names.
fn check_public_host(host: &str) -> Result<(), InvalidUrl> {
	if let Ok(ip) = host.parse::<IpAddr>() {
		return if is_public(ip) {
			Ok(())
		} else {
			Err(InvalidUrl("private address"))
		};
	}

	let host = host.trim_end_matches('.');
	let private_name =
		host == "localhost" || PRIVATE_SUFFIXES.iter().any(|suffix| host.ends_with(suffix));

	if private_name {
		Err(InvalidUrl("private host"))
	} else {
		Ok(())
	}
}

/// Name suffixes that only resolve inside a private network; Railway's
/// private network is `*.railway.internal`.
const PRIVATE_SUFFIXES: [&str; 3] = [".localhost", ".internal", ".local"];

fn is_public(ip: IpAddr) -> bool {
	match ip {
		IpAddr::V4(ip) => is_public_v4(ip),
		IpAddr::V6(ip) => is_public_v6(ip),
	}
}

/// Refuse private, shared, multicast, reserved, and documentation ranges.
fn is_public_v4(ip: Ipv4Addr) -> bool {
	let [first, second, ..] = ip.octets();
	let carrier_nat = first == 100 && (second & 0xc0) == 0x40;

	!(ip.is_private()
		|| ip.is_loopback()
		|| ip.is_link_local()
		|| ip.is_unspecified()
		|| ip.is_broadcast()
		|| first == 0
		|| first >= 224
		|| ip.is_documentation()
		|| (first == 198 && (second == 18 || second == 19))
		|| (first == 192 && second == 0 && ip.octets()[2] == 0)
		|| carrier_nat)
}

/// Global unicast or a public IPv4-mapped address, without transition ranges.
fn is_public_v6(ip: Ipv6Addr) -> bool {
	if let Some(v4) = ip.to_ipv4_mapped() {
		return is_public_v4(v4);
	}

	let [first, second, ..] = ip.segments();

	// Limit destinations to global unicast, excluding protocol assignments,
	// documentation, and 6to4 (which can embed a private IPv4 destination).
	(first & 0xe000) == 0x2000
		&& !(first == 0x2001 && (second < 0x200 || second == 0xdb8))
		&& first != 0x2002
		&& !(first == 0x3fff && second < 0x1000)
}

#[cfg(test)]
mod tests {
	use std::collections::VecDeque;
	use std::net::SocketAddr;
	use std::sync::Mutex;

	use super::*;

	/// Supply deterministic DNS answers, including changes between lookups.
	struct Answers(Mutex<VecDeque<Vec<SocketAddr>>>);

	impl Answers {
		fn new(answers: &[&[SocketAddr]]) -> Self {
			Self(Mutex::new(
				answers.iter().map(|answer| answer.to_vec()).collect(),
			))
		}
	}

	impl Resolve for Answers {
		fn resolve(&self, _: Name) -> Resolving {
			let answer = self
				.0
				.lock()
				.expect("answers")
				.pop_front()
				.expect("a lookup");
			Box::pin(async move { Ok(Box::new(answer.into_iter()) as Addrs) })
		}
	}

	#[test]
	fn signatures_are_stable() {
		assert_eq!(
			sign("secret", 1_700_000_000, "{}"),
			sign("secret", 1_700_000_000, "{}")
		);
		assert_ne!(
			sign("secret", 1_700_000_000, "{}"),
			sign("other", 1_700_000_000, "{}")
		);
		assert!(sign("secret", 1, "{}").starts_with("t=1,v1="));
	}

	#[tokio::test]
	async fn resolved_loopback_target_is_refused() {
		assert!(
			validate_url("https://localhost./hook", false)
				.await
				.is_err()
		);
		assert!(
			PublicResolver(SystemResolver)
				.resolve("localhost".parse().expect("name"))
				.await
				.is_err()
		);
		assert!(validate_url("http://localhost./hook", true).await.is_ok());
	}

	#[test]
	fn private_targets_are_refused() {
		assert!(parse_url("https://example.com/hook", false).is_ok());
		assert!(parse_url("http://example.com/hook", false).is_err());
		assert!(parse_url("https://127.0.0.1/hook", false).is_err());
		assert!(parse_url("https://10.1.2.3/hook", false).is_err());
		assert!(parse_url("https://[::1]/hook", false).is_err());
		assert!(parse_url("https://postgres.railway.internal/", false).is_err());
		assert!(parse_url("https://postgres.railway.internal./", false).is_err());
		assert!(parse_url("https://localhost/", false).is_err());
		assert!(parse_url("http://127.0.0.1:9000/hook", true).is_ok());
		assert!(parse_url("ftp://example.com", true).is_err());
	}

	#[tokio::test]
	async fn dns_refuses_non_public_and_mixed_answers() {
		let public = "8.8.8.8:443".parse().expect("address");

		for ip in [
			"127.0.0.1",
			"10.0.0.1",
			"172.16.0.1",
			"192.168.1.1",
			"169.254.169.254",
			"100.64.0.1",
			"0.1.2.3",
			"224.0.0.1",
			"240.0.0.1",
			"192.0.2.1",
			"198.18.0.1",
			"::",
			"::1",
			"fc00::1",
			"fe80::1",
			"ff02::1",
			"::ffff:127.0.0.1",
			"64:ff9b:1::a00:1",
			"2002:7f00:1::1",
			"2001:db8::1",
		] {
			let private = SocketAddr::new(ip.parse().expect("ip"), 443);

			for answer in [vec![private], vec![public, private], vec![private, public]] {
				let resolver = PublicResolver(Answers::new(&[&answer]));
				assert!(
					resolver
						.resolve("hook.example".parse().expect("name"))
						.await
						.is_err(),
					"{ip}"
				);
			}
		}

		let empty = PublicResolver(Answers::new(&[&[]]));
		assert!(
			empty
				.resolve("hook.example".parse().expect("name"))
				.await
				.is_err()
		);
	}

	#[tokio::test]
	async fn dns_returns_checked_addresses_and_rechecks_changes() {
		let public = [
			"8.8.8.8:0".parse().expect("address"),
			"[2606:4700:4700::1111]:0".parse().expect("address"),
		];
		let private = ["127.0.0.1:0".parse().expect("address")];
		let resolver = PublicResolver(Answers::new(&[&public, &private]));
		let checked: Vec<_> = resolver
			.resolve("hook.example".parse().expect("name"))
			.await
			.expect("public")
			.collect();
		assert_eq!(checked, public);
		assert!(
			resolver
				.resolve("hook.example".parse().expect("name"))
				.await
				.is_err()
		);
	}

	#[tokio::test]
	async fn delivery_does_not_connect_to_private_dns_answers() {
		let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
			.await
			.expect("bind");
		let private = listener.local_addr().expect("address");
		let public = "8.8.8.8:0".parse().expect("address");
		let client = client_builder(false)
			.dns_resolver(Arc::new(PublicResolver(Answers::new(&[
				&[public, private],
				&[private],
			]))))
			.build()
			.expect("client");
		let url = format!("https://hook.example:{}/hook", private.port());

		for _ in 0..2 {
			let error = client
				.post(&url)
				.send()
				.await
				.expect_err("private DNS refused");
			assert!(error.is_connect(), "{error}");
		}

		assert!(
			tokio::time::timeout(Duration::from_millis(50), listener.accept())
				.await
				.is_err()
		);
	}

	#[tokio::test]
	async fn delivery_rechecks_stored_literal_destinations() {
		let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
			.await
			.expect("bind");
		let address = listener.local_addr().expect("address");
		let event = Event {
			key: DocumentKey {
				project: crate::names::ProjectId::new("prj_test"),
				environment: crate::names::Environment::parse("production").expect("environment"),
				name: crate::names::DocumentName::parse("test").expect("document"),
			},
			payload: Payload {
				kind: "document.updated",
				id: "evt_test".into(),
				project: "prj_test".into(),
				environment: "production".into(),
				document: "test".into(),
				editors: vec![],
				first_change_at: 0,
				last_change_at: 0,
			},
		};
		let client = DeliveryClient {
			http: client_builder(false).build().expect("client"),
			allow_private: false,
		};

		for scheme in ["http", "https"] {
			let webhook = crate::store::Webhook {
				url: format!("{scheme}://{address}/hook"),
				secret: "secret".into(),
			};
			assert!(!attempt_delivery(&client, &webhook, &event, "{}", 0).await);
		}

		assert!(
			tokio::time::timeout(Duration::from_millis(50), listener.accept())
				.await
				.is_err()
		);
	}

	#[tokio::test]
	async fn redirects_are_not_followed_even_with_private_opt_in() {
		let target = tokio::net::TcpListener::bind("127.0.0.1:0")
			.await
			.expect("target");
		let location = format!("http://{}/hook", target.local_addr().expect("address"));
		let app = axum::Router::new().route(
			"/hook",
			axum::routing::post(
				move || async move { axum::response::Redirect::temporary(&location) },
			),
		);
		let source = tokio::net::TcpListener::bind("127.0.0.1:0")
			.await
			.expect("source");
		let url = format!("http://{}/hook", source.local_addr().expect("address"));
		let server = tokio::spawn(async move { axum::serve(source, app).await.expect("serve") });
		let response = client_builder(true)
			.build()
			.expect("client")
			.post(url)
			.send()
			.await
			.expect("response");
		assert_eq!(response.status(), reqwest::StatusCode::TEMPORARY_REDIRECT);
		assert!(
			tokio::time::timeout(Duration::from_millis(50), target.accept())
				.await
				.is_err()
		);
		server.abort();
	}
}
