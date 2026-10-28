//! Client tokens: short-lived HS256 JWTs that an app's server mints.
//!
//! The relay holds no accounts. An app's server, which already knows who its
//! user is, mints a token with [`mint_token`] and the shared secret, and the
//! client presents it in `Authorization: Bearer` or, from a browser, the
//! `token` query parameter. A token names its subject for logs, and its
//! `scope` must be `dictation`, so a token minted for another canvist
//! service cannot open a dictation session. The shape and expiry leeway
//! match canvist Cloud's client tokens.

use std::time::Duration;
use std::time::SystemTime;
use std::time::UNIX_EPOCH;

use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use hmac::Hmac;
use hmac::Mac;
use serde::Deserialize;
use serde::Serialize;
use sha2::Sha256;

/// The only scope that opens a dictation session.
pub const SCOPE: &str = "dictation";
/// The shortest token secret: 256 bits, HS256's key size.
pub const MIN_SECRET_BYTES: usize = 32;
/// Clock skew tolerated when checking `exp`, in seconds.
pub const LEEWAY_SECONDS: u64 = 5;

const HEADER: &str = r#"{"alg":"HS256","typ":"JWT"}"#;

/// The claims inside a client token.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Claims {
	/// The app's id for the user, for logs.
	pub sub: String,
	/// What the token allows; it must be [`SCOPE`].
	pub scope: String,
	/// Issued at, in Unix seconds.
	pub iat: u64,
	/// Expires at, in Unix seconds.
	pub exp: u64,
}

impl Claims {
	/// Dictation claims for `subject`, issued at `now` and valid for `ttl`.
	pub fn new(subject: impl Into<String>, now: u64, ttl: Duration) -> Self {
		Self {
			sub: subject.into(),
			scope: SCOPE.to_owned(),
			iat: now,
			exp: now.saturating_add(ttl.as_secs()),
		}
	}
}

/// Why a token was refused.
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum AuthError {
	/// No token was sent.
	#[error("missing token")]
	Missing,
	/// The token is malformed or forged.
	#[error("invalid token")]
	Invalid,
	/// The token expired.
	#[error("token expired")]
	Expired,
	/// The token is genuine but not for dictation.
	#[error("this token is not for dictation")]
	WrongScope,
}

/// A token secret shorter than [`MIN_SECRET_BYTES`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
#[error("the token secret must be at least {MIN_SECRET_BYTES} bytes")]
pub struct WeakSecret;

/// Signs and verifies client tokens.
#[derive(Clone)]
pub struct TokenSigner {
	mac: Hmac<Sha256>,
}

impl std::fmt::Debug for TokenSigner {
	fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		formatter.write_str("TokenSigner(..)")
	}
}

impl TokenSigner {
	/// A signer keyed by `secret`, which must be at least
	/// [`MIN_SECRET_BYTES`] long.
	pub fn new(secret: &str) -> Result<Self, WeakSecret> {
		if secret.len() < MIN_SECRET_BYTES {
			return Err(WeakSecret);
		}

		let mac = Hmac::<Sha256>::new_from_slice(secret.as_bytes()).map_err(|_| WeakSecret)?;

		Ok(Self { mac })
	}

	/// Sign `claims` as a compact JWT.
	pub fn sign(&self, claims: &Claims) -> String {
		let payload = serde_json::json!({
			"sub": claims.sub,
			"scope": claims.scope,
			"iat": claims.iat,
			"exp": claims.exp,
		});
		let signing_input = format!(
			"{}.{}",
			URL_SAFE_NO_PAD.encode(HEADER),
			URL_SAFE_NO_PAD.encode(payload.to_string())
		);
		let signature = URL_SAFE_NO_PAD.encode(self.mac(&signing_input).finalize().into_bytes());

		format!("{signing_input}.{signature}")
	}

	/// Verify `token` at `now` (Unix seconds) and return its claims.
	pub fn verify(&self, token: &str, now: u64) -> Result<Claims, AuthError> {
		let (signing_input, signature) = token.rsplit_once('.').ok_or(AuthError::Invalid)?;
		let (header, payload) = signing_input.split_once('.').ok_or(AuthError::Invalid)?;
		let signature = URL_SAFE_NO_PAD
			.decode(signature)
			.map_err(|_| AuthError::Invalid)?;

		// Security: the signature is checked before anything in the token is
		// parsed, so only bytes this secret signed reach the JSON parser.
		self.mac(signing_input)
			.verify_slice(&signature)
			.map_err(|_| AuthError::Invalid)?;

		if !is_hs256(header) {
			return Err(AuthError::Invalid);
		}

		let payload = URL_SAFE_NO_PAD
			.decode(payload)
			.map_err(|_| AuthError::Invalid)?;
		let claims: Claims = serde_json::from_slice(&payload).map_err(|_| AuthError::Invalid)?;

		if claims.exp.saturating_add(LEEWAY_SECONDS) <= now {
			return Err(AuthError::Expired);
		}

		if claims.scope != SCOPE {
			return Err(AuthError::WrongScope);
		}

		Ok(claims)
	}

	fn mac(&self, input: &str) -> Hmac<Sha256> {
		let mut mac = self.mac.clone();
		mac.update(input.as_bytes());
		mac
	}
}

/// Mint a dictation token for `subject`, valid for `ttl` from now, signed
/// with `secret`. App servers call this, or sign the same claims with any
/// HS256 JWT library.
///
/// ```
/// use std::time::Duration;
///
/// use canvist_dictation_relay::auth::mint_token;
///
/// let token = mint_token(
/// 	"0123456789abcdef0123456789abcdef",
/// 	"user-1",
/// 	Duration::from_mins(5),
/// )?;
/// assert_eq!(token.split('.').count(), 3);
/// # Ok::<(), canvist_dictation_relay::auth::WeakSecret>(())
/// ```
pub fn mint_token(secret: &str, subject: &str, ttl: Duration) -> Result<String, WeakSecret> {
	let signer = TokenSigner::new(secret)?;
	Ok(signer.sign(&Claims::new(subject, unix_now(), ttl)))
}

/// Whether a JWT header names HS256. Pinning `alg` means a key can never be
/// read as some other algorithm's.
fn is_hs256(header: &str) -> bool {
	#[derive(Deserialize)]
	struct Header {
		alg: String,
	}

	URL_SAFE_NO_PAD
		.decode(header)
		.ok()
		.and_then(|bytes| serde_json::from_slice::<Header>(&bytes).ok())
		.is_some_and(|header| header.alg == "HS256")
}

/// The current time in Unix seconds.
pub fn unix_now() -> u64 {
	SystemTime::now()
		.duration_since(UNIX_EPOCH)
		.map_or(0, |elapsed| elapsed.as_secs())
}

/// The bearer credential in an `Authorization` header value.
pub fn bearer(header: &str) -> Option<&str> {
	let (scheme, credential) = header.split_once(' ')?;
	scheme
		.eq_ignore_ascii_case("bearer")
		.then(|| credential.trim())
		.filter(|credential| !credential.is_empty())
}

#[cfg(test)]
mod tests {
	use super::*;

	const SECRET: &str = "0123456789abcdef0123456789abcdef";
	/// When test tokens expire, in Unix seconds.
	const EXPIRY: u64 = 200;
	/// A moment while test tokens are valid.
	const VALID_AT: u64 = 150;

	fn signer() -> TokenSigner {
		TokenSigner::new(SECRET).expect("long enough")
	}

	fn claims(exp: u64) -> Claims {
		Claims {
			sub: "user-1".to_owned(),
			scope: SCOPE.to_owned(),
			iat: 100,
			exp,
		}
	}

	#[test]
	fn tokens_round_trip() {
		let token = signer().sign(&claims(EXPIRY));
		assert_eq!(signer().verify(&token, VALID_AT), Ok(claims(EXPIRY)));
	}

	#[test]
	fn expiry_allows_the_leeway_and_no_more() {
		let token = signer().sign(&claims(EXPIRY));

		assert!(signer().verify(&token, EXPIRY + LEEWAY_SECONDS - 1).is_ok());
		assert_eq!(
			signer().verify(&token, EXPIRY + LEEWAY_SECONDS),
			Err(AuthError::Expired)
		);
	}

	#[test]
	fn other_scopes_are_refused() {
		let token = signer().sign(&Claims {
			scope: "write".to_owned(),
			..claims(EXPIRY)
		});

		assert_eq!(
			signer().verify(&token, VALID_AT),
			Err(AuthError::WrongScope)
		);
	}

	#[test]
	fn forged_and_tampered_tokens_are_refused() {
		let other = TokenSigner::new("another secret that is long enough!").expect("long enough");
		assert_eq!(
			signer().verify(&other.sign(&claims(EXPIRY)), VALID_AT),
			Err(AuthError::Invalid)
		);

		let genuine = signer().sign(&claims(EXPIRY));
		let (input, signature) = genuine.rsplit_once('.').expect("three parts");
		let (header, _) = input.split_once('.').expect("three parts");
		let forged = URL_SAFE_NO_PAD
			.encode(r#"{"sub":"admin","scope":"dictation","iat":1,"exp":9999999999}"#);

		assert_eq!(
			signer().verify(&format!("{header}.{forged}.{signature}"), VALID_AT),
			Err(AuthError::Invalid)
		);
		assert_eq!(
			signer().verify("not a token", VALID_AT),
			Err(AuthError::Invalid)
		);
	}

	#[test]
	fn the_none_algorithm_is_refused() {
		let header = URL_SAFE_NO_PAD.encode(r#"{"alg":"none"}"#);
		let payload =
			URL_SAFE_NO_PAD.encode(r#"{"sub":"a","scope":"dictation","iat":1,"exp":200}"#);

		assert_eq!(
			signer().verify(&format!("{header}.{payload}."), VALID_AT),
			Err(AuthError::Invalid)
		);
	}

	#[test]
	fn short_secrets_are_refused() {
		assert_eq!(TokenSigner::new("short").err(), Some(WeakSecret));
		assert!(mint_token("short", "user-1", Duration::from_mins(1)).is_err());
	}

	#[test]
	fn minted_tokens_verify_now() {
		let token = mint_token(SECRET, "user-1", Duration::from_mins(5)).expect("mints");
		let claims = signer().verify(&token, unix_now()).expect("valid");

		assert_eq!(claims.sub, "user-1");
		assert_eq!(claims.exp - claims.iat, 300);
	}

	#[test]
	fn bearer_headers_parse() {
		assert_eq!(bearer("Bearer abc"), Some("abc"));
		assert_eq!(bearer("bearer  tok "), Some("tok"));
		assert_eq!(bearer("Basic abc"), None);
		assert_eq!(bearer("Bearer "), None);
	}
}
