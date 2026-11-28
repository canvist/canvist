//! The admin dashboard's API: signing in by email, projects, Stripe
//! coupons, and first-party analytics. Mail and Stripe are the fakes in
//! `support/`.

mod support;

use std::time::Duration;

use canvist_cloud::Config;
use canvist_cloud::metering::Plan;
use canvist_cloud::metering::Subscription;
use canvist_cloud::metering::SubscriptionStatus;
use canvist_cloud::metering::Terms;
use reqwest::StatusCode;
use reqwest::header::COOKIE;
use reqwest::header::ORIGIN;
use reqwest::header::SET_COOKIE;
use reqwest::header::USER_AGENT;
use serde_json::Value as Json;
use serde_json::json;
use support::Fixture;
use support::mail::FakeMail;
use support::stripe::FakeStripe;

const OWNER: &str = "owner@canvist.test";
const DASHBOARD: &str = "https://canvist.test/admin";
const BROWSER: &str = concat!(
	"Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 ",
	"(KHTML, like Gecko) Chrome/130.0.0.0 Safari/537.36",
);

fn admin_config(mail: &FakeMail) -> Config {
	let mut config = support::config();
	config.admin.emails = vec![OWNER.to_owned(), "admin@canvist.test".to_owned()];
	config.admin.url = Some(DASHBOARD.to_owned());
	config.admin.delivery = mail.delivery();
	config
}

/// The token in the sign-in link of `message`.
fn link_token(message: &Json) -> String {
	let text = message["text"].as_str().expect("a text body");
	let link = text
		.lines()
		.find(|line| line.starts_with(DASHBOARD))
		.expect("the link");
	link.split_once("#token=")
		.expect("a token in the fragment")
		.1
		.to_owned()
}

/// Sign `OWNER` in through their email, returning the session cookie.
async fn sign_in(fixture: &Fixture, mail: &mut FakeMail) -> String {
	let client = reqwest::Client::new();
	let response = client
		.post(fixture.http("/v1/admin/sign-in"))
		.json(&json!({ "email": " Owner@Canvist.test " }))
		.send()
		.await
		.expect("responds");
	assert_eq!(response.status(), StatusCode::ACCEPTED);

	let message = mail.next().await;
	assert_eq!(message["to"], json!([OWNER]));
	let response = client
		.post(fixture.http("/v1/admin/session"))
		.json(&json!({ "token": link_token(&message) }))
		.send()
		.await
		.expect("responds");
	assert_eq!(response.status(), StatusCode::OK);
	let cookie = response.headers()[SET_COOKIE]
		.to_str()
		.expect("ascii")
		.to_owned();
	assert!(
		cookie.contains("HttpOnly; SameSite=Strict; Secure"),
		"{cookie}"
	);
	assert_eq!(
		response.json::<Json>().await.expect("json"),
		json!({ "admin": OWNER })
	);

	cookie.split(';').next().expect("name=value").to_owned()
}

#[tokio::test]
async fn admins_sign_in_by_email_and_others_get_nothing() {
	let mut mail = FakeMail::start().await;
	let fixture = Fixture::start_with(admin_config(&mail), support::store().await).await;
	let client = reqwest::Client::new();

	let stranger = client
		.post(fixture.http("/v1/admin/sign-in"))
		.json(&json!({ "email": "someone@example.com" }))
		.send()
		.await
		.expect("responds");
	assert_eq!(
		stranger.status(),
		StatusCode::ACCEPTED,
		"the answer is the same"
	);
	assert!(mail.nothing_within(Duration::from_millis(300)).await);

	let cookie = sign_in(&fixture, &mut mail).await;
	let session = |request: reqwest::RequestBuilder| request.header(COOKIE, &cookie);

	let current = session(client.get(fixture.http("/v1/admin/session")))
		.send()
		.await
		.expect("responds");
	assert_eq!(
		current.json::<Json>().await.expect("json"),
		json!({ "admin": OWNER })
	);

	let same_origin = session(client.get(fixture.http("/v1/admin/projects")))
		.header(ORIGIN, "https://canvist.test")
		.send()
		.await
		.expect("responds");
	assert_eq!(same_origin.status(), StatusCode::OK);

	let foreign = session(client.get(fixture.http("/v1/admin/projects")))
		.header(ORIGIN, "https://evil.test")
		.send()
		.await
		.expect("responds");
	assert_eq!(foreign.status(), StatusCode::FORBIDDEN);

	let signed_out = client
		.get(fixture.http("/v1/admin/session"))
		.send()
		.await
		.expect("responds");
	assert_eq!(signed_out.status(), StatusCode::UNAUTHORIZED);

	let ended = session(client.delete(fixture.http("/v1/admin/session")))
		.send()
		.await
		.expect("responds");
	assert_eq!(ended.status(), StatusCode::NO_CONTENT);
	assert!(
		ended.headers()[SET_COOKIE]
			.to_str()
			.expect("ascii")
			.starts_with("canvist_admin=; Path=/v1/admin; Max-Age=0")
	);

	let foreign_sign_out = session(client.delete(fixture.http("/v1/admin/session")))
		.header(ORIGIN, "https://evil.test")
		.send()
		.await
		.expect("responds");
	assert_eq!(
		foreign_sign_out.status(),
		StatusCode::FORBIDDEN,
		"another site cannot sign an admin out"
	);
	assert!(foreign_sign_out.headers().get(SET_COOKIE).is_none());
}

#[tokio::test]
async fn sign_in_tokens_are_not_sessions_and_links_are_spaced_out() {
	let mut mail = FakeMail::start().await;
	let fixture = Fixture::start_with(admin_config(&mail), support::store().await).await;
	let client = reqwest::Client::new();
	let request = || {
		client
			.post(fixture.http("/v1/admin/sign-in"))
			.json(&json!({ "email": OWNER }))
			.send()
	};

	request().await.expect("responds");
	let token = link_token(&mail.next().await);
	request().await.expect("responds");
	assert!(
		mail.nothing_within(Duration::from_millis(300)).await,
		"a second link waits a minute"
	);

	let as_cookie = client
		.get(fixture.http("/v1/admin/session"))
		.header(COOKIE, format!("canvist_admin={token}"))
		.send()
		.await
		.expect("responds");
	assert_eq!(as_cookie.status(), StatusCode::UNAUTHORIZED);

	let forged = client
		.post(fixture.http("/v1/admin/session"))
		.json(&json!({ "token": fixture.token("notes", canvist_cloud::auth::Scope::Write) }))
		.send()
		.await
		.expect("responds");
	assert_eq!(
		forged.status(),
		StatusCode::UNAUTHORIZED,
		"a client token is not a link"
	);
}

#[tokio::test]
async fn admin_routes_are_off_unless_configured() {
	let fixture = Fixture::start_with(Config::local(support::SECRET), support::store().await).await;
	let response = reqwest::get(fixture.http("/v1/admin/session"))
		.await
		.expect("responds");
	assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn projects_are_listed_newest_first() {
	let fixture = Fixture::start().await;
	let created = fixture
		.admin()
		.post("/v1/admin/projects")
		.json(&json!({ "name": "newer", "environments": ["production"] }))
		.send()
		.await
		.expect("responds");
	assert_eq!(created.status(), StatusCode::CREATED);

	let listed: Json = fixture
		.admin()
		.get("/v1/admin/projects")
		.send()
		.await
		.expect("responds")
		.json()
		.await
		.expect("json");
	let projects = listed["projects"].as_array().expect("projects");
	let newer = projects
		.iter()
		.position(|project| project["name"] == "newer")
		.expect("listed");
	let fixture_project = projects
		.iter()
		.position(|project| project["id"] == fixture.project.as_str())
		.expect("listed");
	assert!(newer < fixture_project);
	assert_eq!(projects[fixture_project]["plan"], "hobby");
	assert_eq!(projects[fixture_project]["standing"], "ok");
}

/// A server with Stripe, over its own database.
async fn with_stripe(stripe: &FakeStripe) -> Fixture {
	let mut config = support::config();
	config.stripe = Some(stripe.config());
	Fixture::start_with(config, support::isolated_store().await).await
}

async fn create_coupon(fixture: &Fixture, body: Json) -> (StatusCode, Json) {
	let response = fixture
		.admin()
		.post("/v1/admin/coupons")
		.json(&body)
		.send()
		.await
		.expect("responds");
	let status = response.status();
	(status, response.json().await.expect("json"))
}

#[tokio::test]
async fn public_coupons_get_a_code_anyone_can_use() {
	let stripe = FakeStripe::start().await;
	let fixture = with_stripe(&stripe).await;

	let (status, body) = create_coupon(
		&fixture,
		json!({
			"name": "Launch", "percent_off": 20, "duration": "once",
			"visibility": "public", "code": "LAUNCH20", "max_redemptions": 50,
		}),
	)
	.await;
	assert_eq!(status, StatusCode::CREATED, "{body}");
	assert_eq!(body["coupon"]["visibility"], "public");
	assert_eq!(body["coupon"]["created_by"], "admin token");
	assert_eq!(body["coupon"]["codes"][0]["code"], "LAUNCH20");
	assert_eq!(body["applied_to_subscription"], false);

	let coupon = &stripe.coupons()[0];
	assert_eq!(coupon["percent_off"], 20.0);
	assert_eq!(coupon["metadata"]["canvist_visibility"], "public");
	let code = &stripe.promotion_codes()[0];
	assert_eq!(code["customer"], Json::Null);
	assert_eq!(code["max_redemptions"], 50);

	// The same code again: Stripe refuses it, and the orphaned coupon goes.
	let (status, body) = create_coupon(
		&fixture,
		json!({ "name": "Again", "percent_off": 10, "duration": "once",
			"visibility": "public", "code": "LAUNCH20" }),
	)
	.await;
	assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
	assert_eq!(stripe.coupons().len(), 1);

	let (status, body) = create_coupon(
		&fixture,
		json!({ "name": "Nothing off", "percent_off": 0, "duration": "once", "visibility": "public" }),
	)
	.await;
	assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
}

#[tokio::test]
async fn private_coupons_bind_to_the_project_and_apply_to_its_subscription() {
	let stripe = FakeStripe::start().await;
	let fixture = with_stripe(&stripe).await;

	// A project that has never paid gets a customer, and the code is theirs.
	let (status, body) = create_coupon(
		&fixture,
		json!({
			"name": "Free forever", "percent_off": 100, "duration": "forever",
			"visibility": "private", "project": fixture.project,
		}),
	)
	.await;
	assert_eq!(status, StatusCode::CREATED, "{body}");
	assert_eq!(body["coupon"]["project"], fixture.project.as_str());
	assert_eq!(body["applied_to_subscription"], false);
	let terms = fixture.store.terms(&fixture.project).await.expect("terms");
	let customer = terms.stripe_customer.expect("a customer was made");
	assert_eq!(stripe.promotion_codes()[0]["customer"], customer.as_str());

	// A paying project has the code applied to its subscription.
	stripe.put_subscription("sub_paying", fixture.project.as_str(), "builder", "active");
	fixture
		.store
		.set_terms(
			&fixture.project,
			&Terms {
				stripe_customer: Some(customer.clone()),
				subscription: Some(Subscription {
					id: "sub_paying".to_owned(),
					status: SubscriptionStatus::Active,
				}),
				..Terms::of(Plan::Builder)
			},
		)
		.await
		.expect("sets terms");
	let (status, body) = create_coupon(
		&fixture,
		json!({
			"name": "Trial", "percent_off": 100, "duration": "repeating", "duration_in_months": 3,
			"visibility": "private", "project": fixture.project,
		}),
	)
	.await;
	assert_eq!(status, StatusCode::CREATED, "{body}");
	assert_eq!(body["applied_to_subscription"], true);
	let applied = stripe.posts("/v1/subscriptions/sub_paying");
	assert_eq!(
		applied[0].field("discounts[0][promotion_code]"),
		Some("promo_2")
	);
	assert_eq!(
		stripe.posts("/v1/customers").len(),
		1,
		"the customer is reused"
	);

	let (status, _) = create_coupon(
		&fixture,
		json!({ "name": "Nobody", "percent_off": 10, "duration": "once",
			"visibility": "private", "project": "prj_missing" }),
	)
	.await;
	assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn coupons_are_listed_with_their_codes_and_deleted() {
	let stripe = FakeStripe::start().await;
	let fixture = with_stripe(&stripe).await;
	create_coupon(
		&fixture,
		json!({ "name": "Ten off", "amount_off": 1000, "currency": "usd",
			"duration": "once", "visibility": "public", "code": "TENOFF" }),
	)
	.await;

	let listed: Json = fixture
		.admin()
		.get("/v1/admin/coupons")
		.send()
		.await
		.expect("responds")
		.json()
		.await
		.expect("json");
	let coupon = &listed["coupons"][0];
	assert_eq!(coupon["amount_off"], 1000);
	assert_eq!(coupon["codes"][0]["code"], "TENOFF");
	let id = coupon["id"].as_str().expect("an id");

	let deleted = fixture
		.admin()
		.delete(&format!("/v1/admin/coupons/{id}"))
		.send()
		.await
		.expect("responds");
	assert_eq!(deleted.status(), StatusCode::NO_CONTENT);
	assert!(stripe.coupons().is_empty());
	assert_eq!(stripe.promotion_codes()[0]["active"], false);

	let traversal = fixture
		.admin()
		.delete("/v1/admin/coupons/..%2Fcustomers")
		.send()
		.await
		.expect("responds");
	assert_eq!(traversal.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn deleting_a_coupon_switches_off_every_page_of_its_codes() {
	let stripe = FakeStripe::start().await;
	let fixture = with_stripe(&stripe).await;
	create_coupon(
		&fixture,
		json!({ "name": "Ten off", "amount_off": 1000, "currency": "usd",
			"duration": "once", "visibility": "public", "code": "TENOFF" }),
	)
	.await;
	// More codes than one page holds, made in the Stripe dashboard.
	for index in 0..120 {
		stripe.put_promotion_code(
			&format!("promo_extra_{index}"),
			&format!("EXTRA{index}"),
			"co_1",
		);
	}
	stripe.put_promotion_code("promo_other", "OTHER", "co_2");

	let deleted = fixture
		.admin()
		.delete("/v1/admin/coupons/co_1")
		.send()
		.await
		.expect("responds");
	assert_eq!(deleted.status(), StatusCode::NO_CONTENT);
	let active = |coupon: &str| {
		stripe
			.promotion_codes()
			.into_iter()
			.filter(|code| code["promotion"]["coupon"] == coupon && code["active"] == true)
			.count()
	};
	assert_eq!(active("co_1"), 0, "every page of codes is switched off");
	assert_eq!(active("co_2"), 1, "another coupon's code is untouched");
}

#[tokio::test]
async fn a_code_that_cannot_be_applied_leaves_no_coupon_or_code() {
	let stripe = FakeStripe::start().await;
	let fixture = with_stripe(&stripe).await;
	stripe.put_subscription("sub_paying", fixture.project.as_str(), "builder", "active");
	fixture
		.store
		.set_terms(
			&fixture.project,
			&Terms {
				stripe_customer: Some("cus_test".to_owned()),
				subscription: Some(Subscription {
					id: "sub_paying".to_owned(),
					status: SubscriptionStatus::Active,
				}),
				..Terms::of(Plan::Builder)
			},
		)
		.await
		.expect("sets terms");
	stripe.fail_subscriptions(1);

	let (status, body) = create_coupon(
		&fixture,
		json!({
			"name": "Doomed", "percent_off": 100, "duration": "forever",
			"visibility": "private", "project": fixture.project,
		}),
	)
	.await;
	assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE, "{body}");
	assert!(stripe.coupons().is_empty(), "the coupon is deleted again");
	assert!(
		stripe
			.promotion_codes()
			.iter()
			.all(|code| code["active"] == false),
		"its code is switched off"
	);
}

#[tokio::test]
async fn coupons_need_stripe() {
	let fixture = Fixture::start().await;
	let response = fixture
		.admin()
		.get("/v1/admin/coupons")
		.send()
		.await
		.expect("responds");
	assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

fn pageview(uuid: &str, path: &str, at: i64) -> Json {
	json!({
		"uuid": uuid, "name": "$pageview", "visitor": "visitor-1", "session": "session-1",
		"timestamp": at, "path": path, "source": "news.ycombinator.com",
	})
}

async fn send_batch(fixture: &Fixture, user_agent: &str, events: &[Json]) -> Json {
	let response = reqwest::Client::new()
		.post(fixture.http("/v1/events"))
		.header(USER_AGENT, user_agent)
		.json(&json!({ "sent_at": canvist_cloud::analytics::unix_millis(), "events": events }))
		.send()
		.await
		.expect("responds");
	assert_eq!(response.status(), StatusCode::ACCEPTED);
	response.json().await.expect("json")
}

/// A visit with two pages and a click, from a browser.
fn visit(now: i64) -> [Json; 3] {
	[
		pageview("e1", "/", now - 60_000),
		pageview("e2", "/pricing?ref=x", now - 30_000),
		json!({ "uuid": "e3", "name": "hero: start building", "visitor": "visitor-1",
			"session": "session-1", "timestamp": now - 20_000, "path": "/" }),
	]
}

async fn analytics(fixture: &Fixture, days: u32) -> reqwest::Response {
	fixture
		.admin()
		.get(&format!("/v1/admin/analytics?days={days}"))
		.send()
		.await
		.expect("responds")
}

#[tokio::test]
async fn analytics_are_stored_once_and_bots_are_dropped() {
	let fixture = Fixture::start_with(support::config(), support::isolated_store().await).await;
	let now = canvist_cloud::analytics::unix_millis();
	let batch = visit(now);

	assert_eq!(
		send_batch(&fixture, BROWSER, &batch).await,
		json!({ "accepted": 3, "dropped": 0 })
	);
	assert_eq!(
		send_batch(&fixture, BROWSER, &batch).await["accepted"],
		0,
		"a batch sent twice is stored once"
	);
	assert_eq!(
		send_batch(&fixture, "Googlebot/2.1", &[pageview("bot", "/", now)]).await,
		json!({ "accepted": 0 })
	);
	assert_eq!(
		analytics(&fixture, 365).await.status(),
		StatusCode::BAD_REQUEST
	);
}

#[tokio::test]
async fn analytics_report_the_site_and_the_server() {
	let fixture = Fixture::start_with(support::config(), support::isolated_store().await).await;
	send_batch(
		&fixture,
		BROWSER,
		&visit(canvist_cloud::analytics::unix_millis()),
	)
	.await;
	fixture
		.admin()
		.post("/v1/admin/projects")
		.json(&json!({ "name": "counted", "environments": [] }))
		.send()
		.await
		.expect("responds");

	let report: Json = analytics(&fixture, 7).await.json().await.expect("json");
	let keys = |list: &str| -> Vec<String> {
		report[list]
			.as_array()
			.expect("a list")
			.iter()
			.filter_map(|row| row["key"].as_str().map(str::to_owned))
			.collect()
	};

	assert_eq!(report["summary"]["visitors"], 1);
	assert_eq!(report["summary"]["pageviews"], 2);
	assert_eq!(report["summary"]["bounce_rate"], 0.0);
	assert_eq!(report["days"].as_array().expect("days").len(), 7);
	assert_eq!(keys("pages"), ["/", "/pricing"], "query strings are cut");
	assert_eq!(keys("sources"), ["news.ycombinator.com"]);
	assert_eq!(keys("devices"), ["desktop"]);
	assert_eq!(keys("browsers"), ["Chrome"]);
	assert_eq!(keys("site_events"), ["hero: start building"]);
	assert_eq!(keys("server_events"), ["project created"]);
	assert_eq!(report["truncated"], false);
}
