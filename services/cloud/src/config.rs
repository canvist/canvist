//! Configuration, read once from the environment at startup.
//!
//! Every setting has a production-safe default except the token secret,
//! which must be set. See `docs/src/cloud/self-hosting.md` for the full
//! list.

use std::net::IpAddr;
use std::net::Ipv6Addr;
use std::net::SocketAddr;
use std::str::FromStr;
use std::time::Duration;

/// Which route groups this process serves.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Role {
	/// Sync and Convert in one process: the self-hosting default.
	All,
	/// Only the y-sync relay and the API it needs.
	Sync,
	/// Only the stateless conversion endpoints.
	Convert,
}

impl Role {
	/// Whether this role serves the sync relay.
	pub fn serves_sync(self) -> bool {
		matches!(self, Self::All | Self::Sync)
	}

	/// Whether this role serves conversions.
	pub fn serves_convert(self) -> bool {
		matches!(self, Self::All | Self::Convert)
	}
}

impl FromStr for Role {
	type Err = ConfigError;

	fn from_str(value: &str) -> Result<Self, Self::Err> {
		match value {
			"all" => Ok(Self::All),
			"sync" => Ok(Self::Sync),
			"convert" => Ok(Self::Convert),
			_ => {
				Err(ConfigError::invalid(
					"CANVIST_ROLE",
					"expected all, sync, or convert",
				))
			}
		}
	}
}

/// A secret that never prints.
#[derive(Clone)]
pub struct Secret(String);

impl Secret {
	/// Wrap `value`.
	pub fn new(value: impl Into<String>) -> Self {
		Self(value.into())
	}

	/// The secret's value.
	pub fn expose(&self) -> &str {
		&self.0
	}
}

impl std::fmt::Debug for Secret {
	fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		formatter.write_str("Secret(..)")
	}
}

const MEBIBYTE: usize = 1024 * 1024;
const FLUSH_MILLIS: u64 = 250;
const COMPACT_AFTER_UPDATES: usize = 200;
const MAX_DOCUMENT_MEBIBYTES: usize = 16;
const WEBHOOK_DEBOUNCE_SECONDS: u64 = 2;
const WEBHOOK_MAX_WAIT_SECONDS: u64 = 30;
const PAUSE_RECHECK_SECONDS: u64 = 30;
const MAX_CONVERT_MEBIBYTES: usize = 8;
const CONVERT_TIMEOUT_SECONDS: u64 = 10;
const STRIPE_REPORT_SECONDS: u64 = 10;
/// The port when `PORT` is unset; Railway always sets it.
const DEFAULT_PORT: u16 = 8080;
/// The shortest token secret: 256 bits, HS256's key size.
const MIN_TOKEN_SECRET_BYTES: usize = 32;

/// Settings for the sync relay.
#[derive(Clone, Debug)]
pub struct SyncConfig {
	/// How long a document with no connections stays in memory.
	pub idle_eviction: Duration,
	/// How often pending updates are written to the store.
	pub flush_interval: Duration,
	/// Stored update rows after which a document is compacted into a
	/// snapshot.
	pub compact_after: usize,
	/// The largest WebSocket message accepted, in bytes.
	pub max_message_bytes: usize,
	/// The largest document accepted, as an encoded Yjs state, in bytes.
	pub max_document_bytes: usize,
	/// Quiet time after a change before its webhook fires.
	pub webhook_debounce: Duration,
	/// The longest a webhook waits while changes keep arriving.
	pub webhook_max_wait: Duration,
	/// Allow plain-HTTP webhooks and private hosts, for tests and
	/// self-hosting inside a private network.
	pub webhook_allow_private: bool,
	/// How often a paused session (a new document on a full free plan)
	/// checks whether it may sync now.
	pub pause_recheck: Duration,
}

impl Default for SyncConfig {
	fn default() -> Self {
		Self {
			idle_eviction: Duration::from_mins(1),
			flush_interval: Duration::from_millis(FLUSH_MILLIS),
			compact_after: COMPACT_AFTER_UPDATES,
			max_message_bytes: MEBIBYTE,
			max_document_bytes: MAX_DOCUMENT_MEBIBYTES * MEBIBYTE,
			webhook_debounce: Duration::from_secs(WEBHOOK_DEBOUNCE_SECONDS),
			webhook_max_wait: Duration::from_secs(WEBHOOK_MAX_WAIT_SECONDS),
			webhook_allow_private: false,
			pause_recheck: Duration::from_secs(PAUSE_RECHECK_SECONDS),
		}
	}
}

/// Settings for conversions.
#[derive(Clone, Debug)]
pub struct ConvertConfig {
	/// The largest request body accepted, in bytes.
	pub max_input_bytes: usize,
	/// The longest one conversion may run before the request gives up.
	pub timeout: Duration,
	/// Conversions admitted at once on this instance, including input reads.
	/// Requests beyond this limit are rejected without queuing.
	pub concurrency: usize,
}

impl Default for ConvertConfig {
	fn default() -> Self {
		Self {
			max_input_bytes: MAX_CONVERT_MEBIBYTES * MEBIBYTE,
			timeout: Duration::from_secs(CONVERT_TIMEOUT_SECONDS),
			concurrency: std::thread::available_parallelism().map_or(2, |count| count.get() * 2),
		}
	}
}

/// Stripe Billing, test mode only.
#[derive(Clone, Debug)]
pub struct StripeConfig {
	/// A `sk_test_` or `rk_test_` key. Live keys are refused at startup.
	pub secret_key: Secret,
	/// The monthly active document meter's event name.
	pub meter_event: String,
	/// The conversion meter's event name.
	pub conversion_meter_event: String,
	/// The Stripe API base URL, overridable for tests.
	pub api_base: String,
	/// How often unreported usage is sent to Stripe.
	pub report_interval: Duration,
	/// Whether Checkout calculates tax with Stripe Tax. Leave it off until
	/// the account has a head office address and an active registration:
	/// without them Stripe silently collects no tax.
	pub automatic_tax: bool,
}

/// The people who may sign in to the admin dashboard, and how their
/// sign-in links reach them.
#[derive(Clone, Debug, Default)]
pub struct AdminConfig {
	/// Addresses that may sign in, lowercased. Empty turns sign-in off.
	pub emails: Vec<String>,
	/// The dashboard's URL (`https://canvist.dev/admin`). Sign-in links
	/// point at it, and cookie-authenticated writes must come from its
	/// origin. Required when `emails` is set.
	pub url: Option<String>,
	/// How sign-in links are sent.
	pub delivery: Delivery,
}

impl AdminConfig {
	/// Whether `email` may sign in.
	pub fn is_admin(&self, email: &str) -> bool {
		let email = email.trim().to_lowercase();
		self.emails.contains(&email)
	}

	/// The dashboard's origin (`https://canvist.dev`), if it is set.
	pub fn origin(&self) -> Option<&str> {
		let url = self.url.as_deref()?;
		let after_scheme = url.find("://")? + 3;
		let end = url[after_scheme..]
			.find('/')
			.map_or(url.len(), |slash| after_scheme + slash);
		Some(&url[..end])
	}
}

/// How sign-in links are sent.
#[derive(Clone, Debug, Default)]
pub enum Delivery {
	/// Written to the server log. Only allowed when the dashboard is on
	/// `localhost`, so a production link never lands in a log.
	#[default]
	Log,
	/// Sent through Resend's HTTP API.
	Resend(ResendConfig),
}

/// Resend, the email API sign-in links go through.
#[derive(Clone, Debug)]
pub struct ResendConfig {
	/// The API key.
	pub api_key: Secret,
	/// The sender, as `Name <address>` on a domain verified with Resend.
	pub from: String,
	/// The API base URL, overridable for tests.
	pub api_base: String,
}

/// Everything the server needs.
#[derive(Clone, Debug)]
pub struct Config {
	/// Where to listen.
	pub bind: SocketAddr,
	/// Which route groups to serve.
	pub role: Role,
	/// Postgres connection string; `None` keeps everything in memory.
	pub database_url: Option<Secret>,
	/// The HMAC key that signs client tokens. At least 32 bytes.
	pub token_secret: Secret,
	/// A bearer token for `/v1/admin`, for scripts. Admin routes are off
	/// without it and without `admin.emails`.
	pub admin_token: Option<Secret>,
	/// Who may sign in to the admin dashboard.
	pub admin: AdminConfig,
	/// Sync relay settings.
	pub sync: SyncConfig,
	/// Conversion settings.
	pub convert: ConvertConfig,
	/// Stripe metering, off when `None`.
	pub stripe: Option<StripeConfig>,
	/// The signing secret of the Stripe webhook endpoint; the endpoint is off
	/// without it.
	pub stripe_webhook_secret: Option<Secret>,
}

impl Config {
	/// A configuration for tests and embedding: in memory, every role, and
	/// an ephemeral port.
	pub fn local(token_secret: impl Into<String>) -> Self {
		Self {
			bind: SocketAddr::new(IpAddr::V6(Ipv6Addr::LOCALHOST), 0),
			role: Role::All,
			database_url: None,
			token_secret: Secret::new(token_secret),
			admin_token: None,
			admin: AdminConfig::default(),
			sync: SyncConfig::default(),
			convert: ConvertConfig::default(),
			stripe: None,
			stripe_webhook_secret: None,
		}
	}

	/// Read the configuration from environment variables.
	pub fn from_env() -> Result<Self, ConfigError> {
		Self::from_lookup(|name| std::env::var(name).ok())
	}

	/// Read the configuration through `lookup`, which returns a variable's
	/// value.
	pub fn from_lookup(lookup: impl Fn(&str) -> Option<String>) -> Result<Self, ConfigError> {
		let env = Env(&lookup);
		let token_secret = env.require("CANVIST_TOKEN_SECRET")?;

		if token_secret.len() < MIN_TOKEN_SECRET_BYTES {
			return Err(ConfigError::invalid(
				"CANVIST_TOKEN_SECRET",
				"must be at least 32 bytes",
			));
		}

		Ok(Self {
			bind: SocketAddr::new(
				env.parse("HOST", IpAddr::V6(Ipv6Addr::UNSPECIFIED))?,
				env.parse("PORT", DEFAULT_PORT)?,
			),
			role: env.parse("CANVIST_ROLE", Role::All)?,
			database_url: env.get("DATABASE_URL").map(Secret::new),
			token_secret: Secret::new(token_secret),
			admin_token: env.get("CANVIST_ADMIN_TOKEN").map(Secret::new),
			admin: admin_config(&env)?,
			sync: sync_config(&env)?,
			convert: convert_config(&env)?,
			stripe: stripe_config(&env)?,
			stripe_webhook_secret: env.get("STRIPE_WEBHOOK_SECRET").map(Secret::new),
		})
	}
}

fn admin_config(env: &Env<'_>) -> Result<AdminConfig, ConfigError> {
	let emails: Vec<String> = env
		.get("CANVIST_ADMIN_EMAILS")
		.unwrap_or_default()
		.split(',')
		.map(|email| email.trim().to_lowercase())
		.filter(|email| !email.is_empty())
		.collect();

	if emails.iter().any(|email| !email.contains('@')) {
		return Err(ConfigError::invalid(
			"CANVIST_ADMIN_EMAILS",
			"must be email addresses separated by commas",
		));
	}

	let url = env.get("CANVIST_ADMIN_URL");

	if emails.is_empty() {
		return Ok(AdminConfig::default());
	}

	let Some(url) = url else {
		return Err(ConfigError::invalid(
			"CANVIST_ADMIN_URL",
			"is required when CANVIST_ADMIN_EMAILS is set",
		));
	};
	let delivery = match env.get("RESEND_API_KEY") {
		Some(api_key) => {
			Delivery::Resend(ResendConfig {
				api_key: Secret::new(api_key),
				from: env.require("CANVIST_MAIL_FROM")?,
				api_base: env
					.get("RESEND_API_BASE")
					.unwrap_or_else(|| "https://api.resend.com".to_owned()),
			})
		}
		None if is_local(&url) => Delivery::Log,
		None => {
			return Err(ConfigError::invalid(
				"RESEND_API_KEY",
				"is required to email sign-in links outside localhost",
			));
		}
	};

	Ok(AdminConfig {
		emails,
		url: Some(url),
		delivery,
	})
}

/// Whether `url` is on this machine.
fn is_local(url: &str) -> bool {
	["http://localhost", "http://127.0.0.1", "http://[::1]"]
		.iter()
		.any(|prefix| {
			url.strip_prefix(prefix)
				.is_some_and(|rest| rest.is_empty() || rest.starts_with([':', '/']))
		})
}

fn sync_config(env: &Env<'_>) -> Result<SyncConfig, ConfigError> {
	let defaults = SyncConfig::default();

	Ok(SyncConfig {
		idle_eviction: env.seconds("CANVIST_IDLE_EVICTION_SECONDS", defaults.idle_eviction)?,
		flush_interval: env.millis("CANVIST_FLUSH_INTERVAL_MS", defaults.flush_interval)?,
		compact_after: env.parse("CANVIST_COMPACT_AFTER_UPDATES", defaults.compact_after)?,
		max_message_bytes: env.parse("CANVIST_MAX_MESSAGE_BYTES", defaults.max_message_bytes)?,
		max_document_bytes: env.parse("CANVIST_MAX_DOCUMENT_BYTES", defaults.max_document_bytes)?,
		webhook_debounce: env.millis("CANVIST_WEBHOOK_DEBOUNCE_MS", defaults.webhook_debounce)?,
		webhook_max_wait: env.millis("CANVIST_WEBHOOK_MAX_WAIT_MS", defaults.webhook_max_wait)?,
		webhook_allow_private: env.parse(
			"CANVIST_WEBHOOK_ALLOW_PRIVATE",
			defaults.webhook_allow_private,
		)?,
		pause_recheck: env.seconds("CANVIST_PAUSE_RECHECK_SECONDS", defaults.pause_recheck)?,
	})
}

fn convert_config(env: &Env<'_>) -> Result<ConvertConfig, ConfigError> {
	let defaults = ConvertConfig::default();

	Ok(ConvertConfig {
		max_input_bytes: env.parse("CANVIST_CONVERT_MAX_BYTES", defaults.max_input_bytes)?,
		timeout: env.millis("CANVIST_CONVERT_TIMEOUT_MS", defaults.timeout)?,
		concurrency: env.parse("CANVIST_CONVERT_CONCURRENCY", defaults.concurrency)?,
	})
}

fn stripe_config(env: &Env<'_>) -> Result<Option<StripeConfig>, ConfigError> {
	let Some(secret_key) = env.get("STRIPE_SECRET_KEY") else {
		return Ok(None);
	};

	if !(secret_key.starts_with("sk_test_") || secret_key.starts_with("rk_test_")) {
		return Err(ConfigError::invalid(
			"STRIPE_SECRET_KEY",
			"only test-mode keys (sk_test_ or rk_test_) are accepted",
		));
	}

	Ok(Some(StripeConfig {
		secret_key: Secret::new(secret_key),
		meter_event: env
			.get("STRIPE_METER_EVENT")
			.unwrap_or_else(|| "canvist_active_document".to_owned()),
		conversion_meter_event: env
			.get("STRIPE_CONVERSION_METER_EVENT")
			.unwrap_or_else(|| "canvist_conversion".to_owned()),
		api_base: env
			.get("STRIPE_API_BASE")
			.unwrap_or_else(|| "https://api.stripe.com".to_owned()),
		report_interval: env.millis(
			"STRIPE_REPORT_INTERVAL_MS",
			Duration::from_secs(STRIPE_REPORT_SECONDS),
		)?,
		automatic_tax: env.parse("STRIPE_AUTOMATIC_TAX", false)?,
	}))
}

/// Typed access to variables, treating empty values as unset.
struct Env<'a>(&'a dyn Fn(&str) -> Option<String>);

impl Env<'_> {
	fn get(&self, name: &str) -> Option<String> {
		(self.0)(name).filter(|value| !value.is_empty())
	}

	fn require(&self, name: &str) -> Result<String, ConfigError> {
		self.get(name)
			.ok_or_else(|| ConfigError::invalid(name, "is required"))
	}

	fn parse<T: FromStr>(&self, name: &str, default: T) -> Result<T, ConfigError> {
		self.get(name).map_or(Ok(default), |value| {
			value
				.parse()
				.map_err(|_| ConfigError::invalid(name, "could not be parsed"))
		})
	}

	fn seconds(&self, name: &str, default: Duration) -> Result<Duration, ConfigError> {
		Ok(Duration::from_secs(self.parse(name, default.as_secs())?))
	}

	fn millis(&self, name: &str, default: Duration) -> Result<Duration, ConfigError> {
		let millis = u64::try_from(default.as_millis()).unwrap_or(u64::MAX);
		Ok(Duration::from_millis(self.parse(name, millis)?))
	}
}

/// A variable that is missing or malformed.
#[derive(Debug, thiserror::Error)]
#[error("{name} {problem}")]
pub struct ConfigError {
	name: String,
	problem: &'static str,
}

impl ConfigError {
	fn invalid(name: &str, problem: &'static str) -> Self {
		Self {
			name: name.to_owned(),
			problem,
		}
	}
}

#[cfg(test)]
mod tests {
	use std::collections::HashMap;

	use super::*;

	fn config(pairs: &[(&str, &str)]) -> Result<Config, ConfigError> {
		let vars: HashMap<String, String> = pairs
			.iter()
			.map(|(key, value)| ((*key).to_owned(), (*value).to_owned()))
			.collect();
		Config::from_lookup(|name| vars.get(name).cloned())
	}

	const SECRET: (&str, &str) = ("CANVIST_TOKEN_SECRET", "0123456789abcdef0123456789abcdef");

	#[test]
	fn the_token_secret_is_required_and_long() {
		assert!(config(&[]).is_err());
		assert!(config(&[("CANVIST_TOKEN_SECRET", "short")]).is_err());
		assert!(config(&[SECRET]).is_ok());
	}

	#[test]
	fn live_stripe_keys_are_refused() {
		assert!(config(&[SECRET, ("STRIPE_SECRET_KEY", "sk_live_abc")]).is_err());
		assert!(config(&[SECRET, ("STRIPE_SECRET_KEY", "rk_live_abc")]).is_err());
		let test = config(&[SECRET, ("STRIPE_SECRET_KEY", "sk_test_abc")]).expect("valid");
		let stripe = test.stripe.expect("Stripe is configured");
		assert_eq!(stripe.meter_event, "canvist_active_document");
		assert_eq!(stripe.conversion_meter_event, "canvist_conversion");
		assert!(
			!stripe.automatic_tax,
			"tax stays off until it is switched on"
		);
	}

	#[test]
	fn admin_emails_need_a_dashboard_url_and_a_mailer() {
		let emails = (
			"CANVIST_ADMIN_EMAILS",
			" Owner@Example.com ,admin@example.com",
		);
		assert!(config(&[SECRET, emails]).is_err(), "the URL is required");
		assert!(
			config(&[
				SECRET,
				emails,
				("CANVIST_ADMIN_URL", "https://example.com/admin")
			])
			.is_err(),
			"production needs a mailer"
		);

		let local = config(&[
			SECRET,
			emails,
			("CANVIST_ADMIN_URL", "http://localhost:4321/admin"),
		])
		.expect("valid");
		assert!(matches!(local.admin.delivery, Delivery::Log));
		assert!(local.admin.is_admin("owner@example.com"));
		assert!(local.admin.is_admin(" ADMIN@example.com"));
		assert!(!local.admin.is_admin("someone@example.com"));
		assert_eq!(local.admin.origin(), Some("http://localhost:4321"));

		let mailed = config(&[
			SECRET,
			emails,
			("CANVIST_ADMIN_URL", "https://example.com/admin"),
			("RESEND_API_KEY", "re_test"),
			("CANVIST_MAIL_FROM", "canvist <admin@example.com>"),
		])
		.expect("valid");
		assert!(matches!(mailed.admin.delivery, Delivery::Resend(_)));
		assert_eq!(mailed.admin.origin(), Some("https://example.com"));
	}

	#[test]
	fn lookalike_hosts_are_not_local() {
		assert!(is_local("http://localhost"));
		assert!(is_local("http://127.0.0.1:8080/admin"));
		assert!(!is_local("http://localhost.example.com/admin"));
		assert!(!is_local("https://localhost/admin"));
	}

	#[test]
	fn railway_port_is_honoured() {
		let config = config(&[SECRET, ("PORT", "4321")]).expect("valid");
		assert_eq!(config.bind.port(), 4321);
		assert_eq!(config.role, Role::All);
	}
}
