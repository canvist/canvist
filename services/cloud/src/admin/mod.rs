//! The admin dashboard's API: who may use it, and what it does.
//!
//! Two kinds of caller reach `/v1/admin`:
//!
//! - **Scripts** send `Authorization: Bearer $CANVIST_ADMIN_TOKEN`.
//! - **People** sign in by email. Only addresses in `CANVIST_ADMIN_EMAILS`
//!   are sent a link; it points at the dashboard (`CANVIST_ADMIN_URL`) with
//!   a 15-minute token in the fragment, so it never reaches a server log,
//!   and the dashboard trades it for a week-long session cookie.
//!
//! Both tokens are signed with a key derived from `CANVIST_TOKEN_SECRET` for
//! this purpose alone, so no client token can pass as one. Nothing is
//! stored: removing an address from `CANVIST_ADMIN_EMAILS` locks it out at
//! its next request, and rotating the token secret signs everyone out.
//!
//! The cookie is `HttpOnly`, `SameSite=Strict`, scoped to `/v1/admin`, and
//! `Secure` on HTTPS. A cookie-authenticated request that names another
//! origin is refused, so another site can never act through it.

mod coupons;
mod mail;
mod projects;

use std::collections::HashMap;
use std::fmt;
use std::sync::Mutex;
use std::time::Duration;
use std::time::Instant;

use axum::Json;
use axum::Router;
use axum::extract::Query;
use axum::extract::State;
use axum::http::HeaderMap;
use axum::http::HeaderValue;
use axum::http::StatusCode;
use axum::http::header::AUTHORIZATION;
use axum::http::header::COOKIE;
use axum::http::header::ORIGIN;
use axum::http::header::SET_COOKIE;
use axum::response::IntoResponse;
use axum::response::Response;
use axum::routing::delete;
use axum::routing::get;
use axum::routing::post;
use axum::routing::put;
pub use mail::Mailer;
use serde::Deserialize;
use serde::Serialize;
use serde_json::Value as JsonValue;
use serde_json::json;

use crate::analytics;
use crate::analytics::Report;
use crate::analytics::report::Period;
use crate::app::SharedApp;
use crate::auth;
use crate::auth::AuthError;
use crate::auth::Expiring;
use crate::auth::TokenSigner;
use crate::config::AdminConfig;
use crate::config::Config;
use crate::config::Secret;
use crate::error::ApiError;

/// The session cookie's name.
const COOKIE_NAME: &str = "canvist_admin";
/// How long a sign-in link works.
const SIGN_IN_SECONDS: u64 = 15 * 60;
/// How long a session lasts.
const SESSION_SECONDS: u64 = 7 * 24 * 60 * 60;
/// The shortest gap between two links to one address, so the endpoint
/// cannot flood an admin's inbox.
const LINK_COOLDOWN: Duration = Duration::from_mins(1);
/// The longest analytics period, in days.
const MAX_REPORT_DAYS: u32 = 90;

/// Who is acting as an admin.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Admin {
	/// A script holding `CANVIST_ADMIN_TOKEN`.
	Token,
	/// A person signed in by email.
	Person(String),
}

impl fmt::Display for Admin {
	fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
		match self {
			Self::Token => formatter.write_str("admin token"),
			Self::Person(email) => formatter.write_str(email),
		}
	}
}

/// What a sign-in link or session token says.
#[derive(Debug, Serialize, Deserialize)]
struct AdminClaims {
	/// The admin's email address.
	sub: String,
	purpose: Purpose,
	iat: u64,
	exp: u64,
}

impl Expiring for AdminClaims {
	fn expires_at(&self) -> u64 {
		self.exp
	}
}

/// Which of the two tokens a token is, so neither passes as the other.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum Purpose {
	SignIn,
	Session,
}

/// Admin authentication, shared by every handler.
#[derive(Debug)]
pub struct Admins {
	config: AdminConfig,
	token: Option<Secret>,
	signer: TokenSigner,
	mailer: Mailer,
	/// When each address was last sent a link.
	sent: Mutex<HashMap<String, Instant>>,
}

impl Admins {
	/// Admin authentication as `config` sets it up.
	pub fn new(config: &Config) -> Self {
		Self {
			signer: TokenSigner::derived(config.token_secret.expose(), "canvist admin"),
			mailer: Mailer::new(config.admin.delivery.clone()),
			config: config.admin.clone(),
			token: config.admin_token.clone(),
			sent: Mutex::default(),
		}
	}

	/// The admin behind a request: a bearer token, or a session cookie.
	/// Admin routes answer `404` when neither is configured.
	pub fn authorize(&self, headers: &HeaderMap) -> Result<Admin, ApiError> {
		if self.token.is_none() && self.config.emails.is_empty() {
			return Err(ApiError::not_found("admin routes are disabled"));
		}

		let bearer = headers
			.get(AUTHORIZATION)
			.and_then(|value| value.to_str().ok())
			.and_then(auth::bearer);

		if let Some(given) = bearer {
			return match &self.token {
				Some(expected) if auth::secrets_match(given, expected.expose()) => Ok(Admin::Token),
				_ => Err(AuthError::Invalid.into()),
			};
		}

		let session = cookie(headers, COOKIE_NAME).ok_or(AuthError::Missing)?;
		let email = self.verify(session, Purpose::Session)?;

		if self.is_foreign(headers) {
			return Err(AuthError::Forbidden.into());
		}

		Ok(Admin::Person(email))
	}

	/// The admin a token of `purpose` names, if they are still an admin.
	fn verify(&self, token: &str, purpose: Purpose) -> Result<String, AuthError> {
		let claims: AdminClaims = self.signer.verify_as(token, auth::unix_now())?;

		if claims.purpose != purpose || !self.config.is_admin(&claims.sub) {
			return Err(AuthError::Invalid);
		}

		Ok(claims.sub)
	}

	/// A token naming `email` for `purpose`, valid for `seconds`.
	fn mint(&self, email: &str, purpose: Purpose, seconds: u64) -> String {
		let now = auth::unix_now();
		self.signer.sign(&AdminClaims {
			sub: email.to_owned(),
			purpose,
			iat: now,
			exp: now + seconds,
		})
	}

	/// Whether the request says it comes from a page on another origin.
	fn is_foreign(&self, headers: &HeaderMap) -> bool {
		let Some(origin) = headers.get(ORIGIN).and_then(|value| value.to_str().ok()) else {
			return false;
		};
		self.config.origin() != Some(origin)
	}

	/// Whether a link may go to `email` now, recording that one does.
	fn may_send(&self, email: &str) -> bool {
		let mut sent = self
			.sent
			.lock()
			.unwrap_or_else(std::sync::PoisonError::into_inner);
		let now = Instant::now();

		if sent
			.get(email)
			.is_some_and(|last| now.duration_since(*last) < LINK_COOLDOWN)
		{
			return false;
		}

		sent.insert(email.to_owned(), now);
		true
	}

	/// The `Set-Cookie` value holding `token`, or clearing the cookie.
	fn cookie(&self, token: Option<&str>) -> HeaderValue {
		let secure = self
			.config
			.url
			.as_deref()
			.is_some_and(|url| url.starts_with("https://"));
		let (value, age) = token.map_or(("", 0), |token| (token, SESSION_SECONDS));
		let cookie = format!(
			"{COOKIE_NAME}={value}; Path=/v1/admin; Max-Age={age}; HttpOnly; SameSite=Strict{}",
			if secure { "; Secure" } else { "" }
		);
		HeaderValue::from_str(&cookie).expect("tokens are base64url and dots")
	}
}

/// The value of cookie `name`.
fn cookie<'a>(headers: &'a HeaderMap, name: &str) -> Option<&'a str> {
	headers
		.get_all(COOKIE)
		.iter()
		.filter_map(|value| value.to_str().ok())
		.flat_map(|value| value.split(';'))
		.find_map(|pair| {
			let (key, value) = pair.trim().split_once('=')?;
			(key == name && !value.is_empty()).then_some(value)
		})
}

/// The dashboard's routes.
pub fn routes() -> Router<SharedApp> {
	Router::new()
		.route("/v1/admin/sign-in", post(sign_in))
		.route(
			"/v1/admin/session",
			get(current).post(start_session).delete(end_session),
		)
		.route(
			"/v1/admin/projects",
			get(projects::list_projects).post(projects::create_project),
		)
		.route(
			"/v1/admin/projects/{project}/keys",
			post(projects::create_key),
		)
		.route(
			"/v1/admin/projects/{project}/terms",
			put(projects::set_terms),
		)
		.route("/v1/admin/stats", get(projects::stats))
		.route("/v1/admin/analytics", get(report))
		.route(
			"/v1/admin/coupons",
			get(coupons::list_coupons).post(coupons::create_coupon),
		)
		.route("/v1/admin/coupons/{id}", delete(coupons::remove_coupon))
}

#[derive(Debug, Deserialize)]
struct SignInRequest {
	email: String,
}

/// `POST /v1/admin/sign-in`: email a sign-in link to `email` if it belongs
/// to an admin. The answer is `202` either way, and the link goes out in
/// the background, so the response says nothing about who is an admin.
async fn sign_in(
	State(app): State<SharedApp>,
	Json(request): Json<SignInRequest>,
) -> Result<StatusCode, ApiError> {
	let admins = &app.admins;
	let Some(url) = admins.config.url.as_deref() else {
		return Err(ApiError::not_found("email sign-in is not configured"));
	};
	let email = request.email.trim().to_lowercase();

	if admins.config.is_admin(&email) && admins.may_send(&email) {
		let link = format!(
			"{url}#token={}",
			admins.mint(&email, Purpose::SignIn, SIGN_IN_SECONDS)
		);
		let mailer = admins.mailer.clone();
		tokio::spawn(async move {
			if let Err(error) = mailer.send_sign_in(&email, &link).await {
				tracing::error!(%error, "could not send an admin sign-in link");
			}
		});
	}

	Ok(StatusCode::ACCEPTED)
}

#[derive(Debug, Deserialize)]
struct SessionRequest {
	token: String,
}

/// `POST /v1/admin/session`: trade a sign-in link's token for a session
/// cookie.
async fn start_session(
	State(app): State<SharedApp>,
	Json(request): Json<SessionRequest>,
) -> Result<Response, ApiError> {
	let email = app.admins.verify(&request.token, Purpose::SignIn)?;
	let session = app.admins.mint(&email, Purpose::Session, SESSION_SECONDS);
	tracing::info!(admin = %email, "admin signed in");

	Ok((
		[(SET_COOKIE, app.admins.cookie(Some(&session)))],
		Json(json!({ "admin": email })),
	)
		.into_response())
}

/// `GET /v1/admin/session`: who is signed in.
async fn current(
	State(app): State<SharedApp>,
	headers: HeaderMap,
) -> Result<Json<JsonValue>, ApiError> {
	let admin = app.admins.authorize(&headers)?;
	Ok(Json(json!({ "admin": admin.to_string() })))
}

/// `DELETE /v1/admin/session`: sign out. A request that says it comes from
/// another site is refused, so no page can sign an admin out.
async fn end_session(
	State(app): State<SharedApp>,
	headers: HeaderMap,
) -> Result<Response, ApiError> {
	if app.admins.is_foreign(&headers) {
		return Err(AuthError::Forbidden.into());
	}

	Ok((
		StatusCode::NO_CONTENT,
		[(SET_COOKIE, app.admins.cookie(None))],
	)
		.into_response())
}

#[derive(Debug, Deserialize)]
struct ReportQuery {
	#[serde(default = "default_days")]
	days: u32,
}

fn default_days() -> u32 {
	30
}

/// `GET /v1/admin/analytics?days=30`: the last `days` UTC days (at most
/// 90) of site and product analytics, against the days before.
async fn report(
	State(app): State<SharedApp>,
	Query(query): Query<ReportQuery>,
	headers: HeaderMap,
) -> Result<Json<Report>, ApiError> {
	app.admins.authorize(&headers)?;

	if !(1..=MAX_REPORT_DAYS).contains(&query.days) {
		return Err(ApiError::bad_request("days must be between 1 and 90"));
	}

	let period = Period::last_days(query.days, analytics::unix_millis());
	let scanned = Period {
		from: period.before().from,
		to: period.to,
	};
	let events = app
		.store
		.events_in(scanned, analytics::SCAN_LIMIT + 1)
		.await?;
	let truncated = events.len() > analytics::SCAN_LIMIT;

	Ok(Json(Report::build(&events, period, truncated)))
}

#[cfg(test)]
mod tests {
	use super::*;

	fn admins() -> Admins {
		let mut config = Config::local("0123456789abcdef0123456789abcdef");
		config.admin.emails = vec!["owner@example.com".to_owned()];
		config.admin.url = Some("https://example.com/admin".to_owned());
		Admins::new(&config)
	}

	fn headers(pairs: &[(&'static str, &str)]) -> HeaderMap {
		pairs
			.iter()
			.map(|(name, value)| {
				(
					axum::http::HeaderName::from_static(name),
					HeaderValue::from_str(value).expect("valid"),
				)
			})
			.collect()
	}

	#[test]
	fn sessions_authorize_their_admin() {
		let admins = admins();
		let session = admins.mint("owner@example.com", Purpose::Session, 60);
		let cookie = format!("theme=dark; canvist_admin={session}");

		assert_eq!(
			admins
				.authorize(&headers(&[("cookie", &cookie)]))
				.expect("authorized"),
			Admin::Person("owner@example.com".to_owned())
		);
		assert!(
			admins
				.authorize(&headers(&[
					("cookie", &cookie),
					("origin", "https://example.com")
				]))
				.is_ok()
		);
	}

	#[test]
	fn sign_in_links_are_not_sessions_and_other_origins_are_refused() {
		let admins = admins();
		let link = admins.mint("owner@example.com", Purpose::SignIn, 60);
		let refused = admins.authorize(&headers(&[("cookie", &format!("canvist_admin={link}"))]));
		assert_eq!(
			refused.expect_err("refused").status(),
			StatusCode::UNAUTHORIZED
		);

		let session = admins.mint("owner@example.com", Purpose::Session, 60);
		let foreign = admins.authorize(&headers(&[
			("cookie", &format!("canvist_admin={session}")),
			("origin", "https://evil.example"),
		]));
		assert_eq!(
			foreign.expect_err("refused").status(),
			StatusCode::FORBIDDEN
		);
	}

	#[test]
	fn removed_admins_lose_their_sessions() {
		let admins = admins();
		let session = admins.mint("former@example.com", Purpose::Session, 60);
		assert!(
			admins
				.authorize(&headers(&[("cookie", &format!("canvist_admin={session}"))]))
				.is_err()
		);
	}

	#[test]
	fn links_to_one_address_are_spaced_out() {
		let admins = admins();
		assert!(admins.may_send("owner@example.com"));
		assert!(!admins.may_send("owner@example.com"));
	}

	#[test]
	fn cookies_are_secure_on_https() {
		let cookie = admins().cookie(Some("token"));
		let cookie = cookie.to_str().expect("ascii");
		assert!(cookie.starts_with("canvist_admin=token; Path=/v1/admin; Max-Age=604800"));
		assert!(cookie.ends_with("HttpOnly; SameSite=Strict; Secure"));
	}
}
