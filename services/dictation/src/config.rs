//! Configuration, read once from the environment at startup.
//!
//! The token secret and the speech provider must be set; everything else
//! has a default. A provider or polish model that needs an API key refuses
//! to start without one, so a misconfigured relay fails at deploy time
//! rather than on the first session. The readme lists every variable.

use std::net::IpAddr;
use std::net::Ipv6Addr;
use std::net::SocketAddr;
use std::str::FromStr;
use std::time::Duration;

use crate::auth::MIN_SECRET_BYTES;

/// The port when `PORT` is unset.
const DEFAULT_PORT: u16 = 8080;
const SEGMENT_GRACE_MILLIS: u64 = 600;
const POLISH_TIMEOUT_MILLIS: u64 = 3_000;
const MAX_SESSION_SECONDS: u64 = 10 * 60;
const IDLE_SECONDS: u64 = 30;
const FINISH_TIMEOUT_MILLIS: u64 = 10_000;
/// Two seconds of 16 kHz PCM16, well above the 20 to 100 ms clients send.
const MAX_FRAME_BYTES: usize = 64 * 1024;

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

/// Which speech-to-text API the relay drives.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProviderKind {
	/// OpenAI's GA Realtime transcription API, or a server that copies it.
	OpenAi,
	/// Deepgram's streaming `listen` API.
	Deepgram,
	/// vLLM's `/v1/realtime` dialect, for local models such as Voxtral
	/// Realtime and Qwen3-ASR.
	Vllm,
}

impl ProviderKind {
	fn default_url(self) -> &'static str {
		match self {
			Self::OpenAi => "wss://api.openai.com",
			Self::Deepgram => "wss://api.deepgram.com",
			Self::Vllm => "ws://localhost:8000",
		}
	}

	fn default_model(self) -> &'static str {
		match self {
			Self::OpenAi => "gpt-live-transcribe",
			Self::Deepgram => "nova-3",
			Self::Vllm => "mistralai/Voxtral-Mini-4B-Realtime-2602",
		}
	}

	/// Whether the hosted API refuses requests without a key. A local vLLM
	/// only needs one when it was started with `--api-key`.
	fn needs_key(self) -> bool {
		!matches!(self, Self::Vllm)
	}
}

impl FromStr for ProviderKind {
	type Err = ConfigError;

	fn from_str(value: &str) -> Result<Self, Self::Err> {
		match value {
			"openai" => Ok(Self::OpenAi),
			"deepgram" => Ok(Self::Deepgram),
			"vllm" => Ok(Self::Vllm),
			_ => {
				Err(ConfigError::invalid(
					"CANVIST_DICTATION_PROVIDER",
					"expected openai, deepgram, or vllm",
				))
			}
		}
	}
}

/// The speech provider and how to reach it.
#[derive(Clone, Debug)]
pub struct ProviderConfig {
	/// Which API it speaks.
	pub kind: ProviderKind,
	/// The `ws://` or `wss://` origin, without a path or trailing slash.
	pub url: String,
	/// The transcription model.
	pub model: String,
	/// The API key, when the provider takes one.
	pub key: Option<Secret>,
	/// How long a vLLM segment stays open after a pause, for trailing
	/// deltas.
	pub segment_grace: Duration,
}

impl ProviderConfig {
	/// The provider `kind` at its default URL and model, with `key`.
	pub fn new(kind: ProviderKind, key: Option<Secret>) -> Self {
		Self {
			kind,
			url: kind.default_url().to_owned(),
			model: kind.default_model().to_owned(),
			key,
			segment_grace: Duration::from_millis(SEGMENT_GRACE_MILLIS),
		}
	}
}

/// Which language model API runs the polish pass.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PolishKind {
	/// Anthropic's Messages API.
	Anthropic,
	/// Any Chat Completions-compatible server: OpenAI, Ollama, vLLM, or
	/// llama.cpp.
	OpenAi,
}

/// The polish pass and how to reach its model.
#[derive(Clone, Debug)]
pub struct PolishConfig {
	/// Which API it speaks.
	pub kind: PolishKind,
	/// The API base URL: `https://api.anthropic.com` for Anthropic, or a
	/// Chat Completions base ending in `/v1`.
	pub url: String,
	/// The model.
	pub model: String,
	/// The API key, when the server takes one.
	pub key: Option<Secret>,
	/// The longest one segment's polish may take before the raw transcript
	/// is used.
	pub timeout: Duration,
}

impl PolishConfig {
	/// Anthropic's Messages API with its default model.
	pub fn anthropic(key: Secret) -> Self {
		Self {
			kind: PolishKind::Anthropic,
			url: "https://api.anthropic.com".to_owned(),
			model: "claude-haiku-5-5".to_owned(),
			key: Some(key),
			timeout: Duration::from_millis(POLISH_TIMEOUT_MILLIS),
		}
	}

	/// A Chat Completions-compatible server at `url` running `model`.
	pub fn openai(url: impl Into<String>, model: impl Into<String>, key: Option<Secret>) -> Self {
		Self {
			kind: PolishKind::OpenAi,
			url: url.into(),
			model: model.into(),
			key,
			timeout: Duration::from_millis(POLISH_TIMEOUT_MILLIS),
		}
	}
}

/// Limits on each session.
#[derive(Clone, Debug)]
pub struct Limits {
	/// The longest a session may run.
	pub max_session: Duration,
	/// How long a session may go without audio, or without its `start`.
	pub idle: Duration,
	/// The largest message a client may send, in bytes.
	pub max_frame_bytes: usize,
	/// How long `stop` waits for the provider to finish open segments.
	pub finish_timeout: Duration,
}

impl Default for Limits {
	fn default() -> Self {
		Self {
			max_session: Duration::from_secs(MAX_SESSION_SECONDS),
			idle: Duration::from_secs(IDLE_SECONDS),
			max_frame_bytes: MAX_FRAME_BYTES,
			finish_timeout: Duration::from_millis(FINISH_TIMEOUT_MILLIS),
		}
	}
}

/// Everything the relay needs.
#[derive(Clone, Debug)]
pub struct Config {
	/// Where to listen.
	pub bind: SocketAddr,
	/// The HMAC key that signs client tokens. At least 32 bytes.
	pub token_secret: Secret,
	/// The speech provider.
	pub provider: ProviderConfig,
	/// The polish pass, off when `None`.
	pub polish: Option<PolishConfig>,
	/// Per-session limits.
	pub limits: Limits,
}

impl Config {
	/// A configuration for tests and embedding: `provider`, no polish, and
	/// an ephemeral port on localhost.
	pub fn local(token_secret: impl Into<String>, provider: ProviderConfig) -> Self {
		Self {
			bind: SocketAddr::new(IpAddr::V6(Ipv6Addr::LOCALHOST), 0),
			token_secret: Secret::new(token_secret),
			provider,
			polish: None,
			limits: Limits::default(),
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
		let token_secret = env.require("CANVIST_DICTATION_TOKEN_SECRET")?;

		if token_secret.len() < MIN_SECRET_BYTES {
			return Err(ConfigError::invalid(
				"CANVIST_DICTATION_TOKEN_SECRET",
				"must be at least 32 bytes",
			));
		}

		Ok(Self {
			bind: SocketAddr::new(
				env.parse("HOST", IpAddr::V6(Ipv6Addr::UNSPECIFIED))?,
				env.parse("PORT", DEFAULT_PORT)?,
			),
			token_secret: Secret::new(token_secret),
			provider: provider_config(&env)?,
			polish: polish_config(&env)?,
			limits: limits(&env)?,
		})
	}
}

fn provider_config(env: &Env<'_>) -> Result<ProviderConfig, ConfigError> {
	let kind: ProviderKind = env.require("CANVIST_DICTATION_PROVIDER")?.parse()?;
	let key = env.get("CANVIST_DICTATION_PROVIDER_KEY").map(Secret::new);

	if kind.needs_key() && key.is_none() {
		return Err(ConfigError::invalid(
			"CANVIST_DICTATION_PROVIDER_KEY",
			"is required for this provider",
		));
	}

	let defaults = ProviderConfig::new(kind, key);

	Ok(ProviderConfig {
		url: env.url(
			"CANVIST_DICTATION_PROVIDER_URL",
			&defaults.url,
			&["ws://", "wss://"],
		)?,
		model: env
			.get("CANVIST_DICTATION_PROVIDER_MODEL")
			.unwrap_or(defaults.model),
		segment_grace: env.millis("CANVIST_DICTATION_SEGMENT_GRACE_MS", defaults.segment_grace)?,
		..defaults
	})
}

fn polish_config(env: &Env<'_>) -> Result<Option<PolishConfig>, ConfigError> {
	let key = env.get("CANVIST_DICTATION_POLISH_KEY").map(Secret::new);
	let model = env.get("CANVIST_DICTATION_POLISH_MODEL");

	let defaults = match env.get("CANVIST_DICTATION_POLISH").as_deref() {
		None | Some("off") => return Ok(None),
		Some("anthropic") => {
			let key = key.ok_or_else(|| {
				ConfigError::invalid("CANVIST_DICTATION_POLISH_KEY", "is required for anthropic")
			})?;
			PolishConfig::anthropic(key)
		}
		Some("openai") => {
			let model = model.clone().ok_or_else(|| {
				ConfigError::invalid("CANVIST_DICTATION_POLISH_MODEL", "is required for openai")
			})?;
			PolishConfig::openai("https://api.openai.com/v1", model, key)
		}
		Some(_) => {
			return Err(ConfigError::invalid(
				"CANVIST_DICTATION_POLISH",
				"expected off, anthropic, or openai",
			));
		}
	};

	Ok(Some(PolishConfig {
		url: env.url(
			"CANVIST_DICTATION_POLISH_URL",
			&defaults.url,
			&["http://", "https://"],
		)?,
		model: model.unwrap_or_else(|| defaults.model.clone()),
		timeout: env.millis("CANVIST_DICTATION_POLISH_TIMEOUT_MS", defaults.timeout)?,
		..defaults
	}))
}

fn limits(env: &Env<'_>) -> Result<Limits, ConfigError> {
	let defaults = Limits::default();

	Ok(Limits {
		max_session: env.seconds(
			"CANVIST_DICTATION_MAX_SESSION_SECONDS",
			defaults.max_session,
		)?,
		idle: env.seconds("CANVIST_DICTATION_IDLE_SECONDS", defaults.idle)?,
		max_frame_bytes: env.parse(
			"CANVIST_DICTATION_MAX_FRAME_BYTES",
			defaults.max_frame_bytes,
		)?,
		finish_timeout: env.millis(
			"CANVIST_DICTATION_FINISH_TIMEOUT_MS",
			defaults.finish_timeout,
		)?,
	})
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

	/// A base URL with one of `schemes`, without its trailing slash.
	fn url(&self, name: &str, default: &str, schemes: &[&str]) -> Result<String, ConfigError> {
		let url = self.get(name).unwrap_or_else(|| default.to_owned());

		if !schemes.iter().any(|scheme| url.starts_with(scheme)) {
			return Err(ConfigError::invalid(name, "has the wrong scheme"));
		}

		Ok(url.trim_end_matches('/').to_owned())
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

	const SECRET: (&str, &str) = (
		"CANVIST_DICTATION_TOKEN_SECRET",
		"0123456789abcdef0123456789abcdef",
	);
	const VLLM: (&str, &str) = ("CANVIST_DICTATION_PROVIDER", "vllm");

	fn config(pairs: &[(&str, &str)]) -> Result<Config, ConfigError> {
		let vars: HashMap<String, String> = pairs
			.iter()
			.map(|(key, value)| ((*key).to_owned(), (*value).to_owned()))
			.collect();
		Config::from_lookup(|name| vars.get(name).cloned())
	}

	#[test]
	fn the_token_secret_and_provider_are_required() {
		assert!(config(&[VLLM]).is_err());
		assert!(config(&[("CANVIST_DICTATION_TOKEN_SECRET", "short"), VLLM]).is_err());
		assert!(config(&[SECRET]).is_err());
		assert!(config(&[SECRET, ("CANVIST_DICTATION_PROVIDER", "whisper")]).is_err());
		assert!(config(&[SECRET, VLLM]).is_ok());
	}

	#[test]
	fn hosted_providers_need_a_key() {
		for provider in ["openai", "deepgram"] {
			let kind = ("CANVIST_DICTATION_PROVIDER", provider);
			let error = config(&[SECRET, kind]).expect_err("no key").to_string();

			assert!(error.contains("CANVIST_DICTATION_PROVIDER_KEY"), "{error}");
			assert!(config(&[SECRET, kind, ("CANVIST_DICTATION_PROVIDER_KEY", "k")]).is_ok());
		}
	}

	#[test]
	fn providers_have_defaults_that_can_be_overridden() {
		let defaults = config(&[SECRET, VLLM]).expect("valid");
		assert_eq!(defaults.provider.url, "ws://localhost:8000");
		assert_eq!(
			defaults.provider.model,
			"mistralai/Voxtral-Mini-4B-Realtime-2602"
		);
		assert_eq!(defaults.provider.segment_grace, Duration::from_millis(600));
		assert!(defaults.polish.is_none());

		let custom = config(&[
			SECRET,
			VLLM,
			("CANVIST_DICTATION_PROVIDER_URL", "wss://asr.internal/"),
			("CANVIST_DICTATION_PROVIDER_MODEL", "Qwen/Qwen3-ASR"),
		])
		.expect("valid");
		assert_eq!(custom.provider.url, "wss://asr.internal");
		assert_eq!(custom.provider.model, "Qwen/Qwen3-ASR");

		assert!(
			config(&[
				SECRET,
				VLLM,
				("CANVIST_DICTATION_PROVIDER_URL", "https://asr.internal")
			])
			.is_err()
		);
	}

	#[test]
	fn polish_needs_its_key_or_model() {
		let anthropic = ("CANVIST_DICTATION_POLISH", "anthropic");
		assert!(config(&[SECRET, VLLM, anthropic]).is_err());

		let polish = config(&[
			SECRET,
			VLLM,
			anthropic,
			("CANVIST_DICTATION_POLISH_KEY", "k"),
		])
		.expect("valid")
		.polish
		.expect("polish is on");
		assert_eq!(polish.model, "claude-haiku-5-5");
		assert_eq!(polish.timeout, Duration::from_secs(3));

		let openai = ("CANVIST_DICTATION_POLISH", "openai");
		assert!(config(&[SECRET, VLLM, openai]).is_err());

		let local = config(&[
			SECRET,
			VLLM,
			openai,
			("CANVIST_DICTATION_POLISH_MODEL", "llama3.2"),
			("CANVIST_DICTATION_POLISH_URL", "http://localhost:11434/v1"),
		])
		.expect("valid")
		.polish
		.expect("polish is on");
		assert_eq!(local.kind, PolishKind::OpenAi);
		assert_eq!(local.url, "http://localhost:11434/v1");
		assert!(local.key.is_none());

		assert!(config(&[SECRET, VLLM, ("CANVIST_DICTATION_POLISH", "gpt")]).is_err());
	}

	#[test]
	fn limits_and_port_are_configurable() {
		let config = config(&[
			SECRET,
			VLLM,
			("PORT", "4321"),
			("CANVIST_DICTATION_IDLE_SECONDS", "5"),
			("CANVIST_DICTATION_MAX_FRAME_BYTES", "1024"),
		])
		.expect("valid");

		assert_eq!(config.bind.port(), 4321);
		assert_eq!(config.limits.idle, Duration::from_secs(5));
		assert_eq!(config.limits.max_frame_bytes, 1024);
		assert_eq!(config.limits.max_session, Duration::from_mins(10));
	}

	#[test]
	fn secrets_never_print() {
		assert_eq!(format!("{:?}", Secret::new("sk-live")), "Secret(..)");
	}
}
