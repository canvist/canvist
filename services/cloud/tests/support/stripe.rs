//! A stand-in for the Stripe API: the canvist catalog, subscriptions and
//! Checkout Sessions the test controls, subscriptions the server creates,
//! and a record of every request.

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::Mutex;
use std::time::Duration;

use axum::Json;
use axum::Router;
use axum::extract::Path;
use axum::extract::RawQuery;
use axum::extract::State;
use axum::http::HeaderMap;
use axum::http::StatusCode;
use axum::routing::get;
use axum::routing::post;
use canvist_cloud::config::Secret;
use canvist_cloud::config::StripeConfig;
use serde_json::Value;
use serde_json::json;
use tokio::sync::mpsc;

/// A request the fake received.
#[derive(Clone, Debug)]
pub struct Recorded {
	pub path: String,
	pub headers: HeaderMap,
	pub form: Vec<(String, String)>,
}

impl Recorded {
	/// The first value of form field `name`.
	pub fn field(&self, name: &str) -> Option<&str> {
		self.form
			.iter()
			.find(|(key, _)| key == name)
			.map(|(_, value)| value.as_str())
	}
}

#[derive(Default)]
struct Inner {
	subscriptions: HashMap<String, Value>,
	checkout_sessions: HashMap<String, Value>,
	/// Responses to creations, by idempotency key.
	idempotent: HashMap<String, Value>,
	failing_meter_events: usize,
	failing_subscriptions: usize,
	posts: Vec<Recorded>,
	/// Coupons and promotion codes, oldest first.
	coupons: Vec<Value>,
	promotion_codes: Vec<Value>,
	customers: usize,
}

#[derive(Clone)]
struct Shared {
	inner: Arc<Mutex<Inner>>,
	meter_events: mpsc::UnboundedSender<Recorded>,
}

/// A running fake.
pub struct FakeStripe {
	pub url: String,
	shared: Shared,
	meter_events: mpsc::UnboundedReceiver<Recorded>,
}

/// The lookup keys of the catalog's prices, and whether each is metered.
const PRICES: [(&str, bool); 8] = [
	("canvist_builder_monthly", false),
	("canvist_builder_annual", false),
	("canvist_builder_mad_overage", true),
	("canvist_builder_conversion_overage", true),
	("canvist_scale_monthly", false),
	("canvist_scale_annual", false),
	("canvist_scale_mad_overage", true),
	("canvist_scale_conversion_overage", true),
];

impl FakeStripe {
	pub async fn start() -> Self {
		let (sender, meter_events) = mpsc::unbounded_channel();
		let shared = Shared {
			inner: Arc::default(),
			meter_events: sender,
		};
		let app = Router::new()
			.route("/v1/billing/meter_events", post(meter_event))
			.route("/v1/prices", get(prices))
			.route("/v1/checkout/sessions", post(checkout_session))
			.route("/v1/checkout/sessions/{id}", get(retrieve_checkout_session))
			.route(
				"/v1/subscriptions",
				get(list_subscriptions).post(create_subscription),
			)
			.route(
				"/v1/subscriptions/{id}",
				get(subscription).post(update_subscription),
			)
			.route(
				"/v1/billing_portal/configurations",
				get(portal_configurations),
			)
			.route("/v1/billing_portal/sessions", post(portal_session))
			.route("/v1/coupons", get(list_coupons).post(create_coupon))
			.route("/v1/coupons/{id}", axum::routing::delete(delete_coupon))
			.route(
				"/v1/promotion_codes",
				get(list_promotion_codes).post(create_promotion_code),
			)
			.route("/v1/promotion_codes/{id}", post(update_promotion_code))
			.route("/v1/customers", post(create_customer))
			.with_state(shared.clone());
		let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
			.await
			.expect("binds");
		let url = format!("http://{}", listener.local_addr().expect("addr"));
		tokio::spawn(async move { axum::serve(listener, app).await });

		Self {
			url,
			shared,
			meter_events,
		}
	}

	/// Billing configuration pointing at this fake, reporting quickly.
	pub fn config(&self) -> StripeConfig {
		StripeConfig {
			secret_key: Secret::new("sk_test_canvist"),
			meter_event: "canvist_active_document".to_owned(),
			conversion_meter_event: "canvist_conversion".to_owned(),
			api_base: self.url.clone(),
			report_interval: Duration::from_millis(50),
			automatic_tax: false,
		}
	}

	/// Store a monthly subscription for `project` on `plan` with `status`.
	pub fn put_subscription(&self, id: &str, project: &str, plan: &str, status: &str) {
		self.put_subscription_billed(id, project, plan, "monthly", status);
	}

	/// Store a subscription for `project` on `plan` with `status`, whose base
	/// price is `canvist_<plan>_<base>`.
	pub fn put_subscription_billed(
		&self,
		id: &str,
		project: &str,
		plan: &str,
		base: &str,
		status: &str,
	) {
		let items: Vec<Value> = [base, "mad_overage", "conversion_overage"]
			.iter()
			.enumerate()
			.map(|(index, suffix)| {
				price_item(
					&format!("si_{id}_{index}"),
					&format!("canvist_{plan}_{suffix}"),
				)
			})
			.collect();
		let subscription = json!({
			"id": id,
			"object": "subscription",
			"customer": "cus_test",
			"status": status,
			"metadata": { "canvist_project": project },
			"items": { "data": items },
		});
		self.lock()
			.subscriptions
			.insert(id.to_owned(), subscription);
	}

	/// Store a setup-mode Checkout Session for an annual `plan`, with
	/// `status`, that saved `pm_card_visa` for `cus_test`.
	pub fn put_annual_checkout_session(&self, id: &str, project: &str, plan: &str, status: &str) {
		let session = json!({
			"id": id,
			"object": "checkout.session",
			"mode": "setup",
			"status": status,
			"customer": "cus_test",
			"subscription": null,
			"metadata": { "canvist_project": project, "canvist_plan": plan },
			"setup_intent": {
				"id": format!("seti_{id}"),
				"object": "setup_intent",
				"payment_method": "pm_card_visa",
			},
		});
		self.lock().checkout_sessions.insert(id.to_owned(), session);
	}

	/// The subscription with `id`, as the fake holds it now.
	pub fn subscription(&self, id: &str) -> Option<Value> {
		self.lock().subscriptions.get(id).cloned()
	}

	/// Answer the next `count` meter events with a server error.
	pub fn fail_meter_events(&self, count: usize) {
		self.lock().failing_meter_events = count;
	}

	/// Answer the next `count` subscription updates with a server error.
	pub fn fail_subscriptions(&self, count: usize) {
		self.lock().failing_subscriptions = count;
	}

	/// Store an active promotion code for `coupon` directly, as one made in
	/// the Stripe dashboard would be.
	pub fn put_promotion_code(&self, id: &str, code: &str, coupon: &str) {
		self.lock().promotion_codes.push(json!({
			"id": id,
			"object": "promotion_code",
			"code": code,
			"active": true,
			"promotion": { "type": "coupon", "coupon": coupon },
			"customer": null,
			"times_redeemed": 0,
			"metadata": {},
		}));
	}

	/// The next meter event Stripe accepted, waiting up to five seconds.
	pub async fn next_meter_event(&mut self) -> Recorded {
		tokio::time::timeout(Duration::from_secs(5), self.meter_events.recv())
			.await
			.expect("a meter event arrives")
			.expect("the fake is running")
	}

	/// Whether no further meter event arrives within `wait`.
	pub async fn no_meter_event_within(&mut self, wait: Duration) -> bool {
		tokio::time::timeout(wait, self.meter_events.recv())
			.await
			.is_err()
	}

	/// The coupons the fake holds, oldest first.
	pub fn coupons(&self) -> Vec<Value> {
		self.lock().coupons.clone()
	}

	/// The promotion codes the fake holds, oldest first.
	pub fn promotion_codes(&self) -> Vec<Value> {
		self.lock().promotion_codes.clone()
	}

	/// Every POST to `path`, oldest first.
	pub fn posts(&self, path: &str) -> Vec<Recorded> {
		self.lock()
			.posts
			.iter()
			.filter(|recorded| recorded.path == path)
			.cloned()
			.collect()
	}

	fn lock(&self) -> std::sync::MutexGuard<'_, Inner> {
		self.shared.inner.lock().expect("not poisoned")
	}
}

fn price_item(id: &str, lookup_key: &str) -> Value {
	let metered = PRICES
		.iter()
		.any(|(key, metered)| *key == lookup_key && *metered);
	json!({
		"id": id,
		"price": price(lookup_key, metered),
	})
}

fn price(lookup_key: &str, metered: bool) -> Value {
	let interval = if lookup_key.ends_with("_annual") {
		"year"
	} else {
		"month"
	};
	json!({
		"id": format!("price_{lookup_key}"),
		"lookup_key": lookup_key,
		"currency": "usd",
		"recurring": {
			"interval": interval,
			"usage_type": if metered { "metered" } else { "licensed" },
		},
	})
}

fn record(shared: &Shared, path: &str, headers: HeaderMap, body: &str) -> Recorded {
	let recorded = Recorded {
		path: path.to_owned(),
		headers,
		form: decode_form(body),
	};
	shared
		.inner
		.lock()
		.expect("not poisoned")
		.posts
		.push(recorded.clone());
	recorded
}

async fn meter_event(
	State(shared): State<Shared>,
	headers: HeaderMap,
	body: String,
) -> (StatusCode, Json<Value>) {
	let recorded = record(&shared, "/v1/billing/meter_events", headers, &body);
	{
		let mut inner = shared.inner.lock().expect("not poisoned");

		if inner.failing_meter_events > 0 {
			inner.failing_meter_events -= 1;
			return (
				StatusCode::INTERNAL_SERVER_ERROR,
				Json(json!({ "error": { "message": "try again" } })),
			);
		}
	}
	let _ = shared.meter_events.send(recorded);
	(
		StatusCode::OK,
		Json(json!({ "object": "billing.meter_event" })),
	)
}

async fn prices(RawQuery(query): RawQuery) -> Json<Value> {
	let wanted: Vec<String> = decode_form(&query.unwrap_or_default())
		.into_iter()
		.filter(|(key, _)| key == "lookup_keys[]")
		.map(|(_, value)| value)
		.collect();
	let data: Vec<Value> = PRICES
		.iter()
		.filter(|(key, _)| wanted.iter().any(|wanted| wanted == key))
		.map(|(key, metered)| price(key, *metered))
		.collect();
	Json(json!({ "object": "list", "data": data }))
}

async fn checkout_session(
	State(shared): State<Shared>,
	headers: HeaderMap,
	body: String,
) -> Json<Value> {
	record(&shared, "/v1/checkout/sessions", headers, &body);
	Json(json!({ "id": "cs_test_1", "url": "https://checkout.stripe.test/cs_test_1" }))
}

/// A stored Checkout Session, with its setup intent as an id unless the
/// query expands it, as Stripe does.
async fn retrieve_checkout_session(
	State(shared): State<Shared>,
	Path(id): Path<String>,
	RawQuery(query): RawQuery,
) -> Result<Json<Value>, StatusCode> {
	let mut session = shared
		.inner
		.lock()
		.expect("not poisoned")
		.checkout_sessions
		.get(&id)
		.cloned()
		.ok_or(StatusCode::NOT_FOUND)?;
	let expanded = decode_form(&query.unwrap_or_default())
		.iter()
		.any(|(key, value)| key == "expand[]" && value == "setup_intent");

	if !expanded {
		session["setup_intent"] = session["setup_intent"]["id"].clone();
	}

	Ok(Json(session))
}

/// The subscriptions of `customer`, of any status.
async fn list_subscriptions(
	State(shared): State<Shared>,
	RawQuery(query): RawQuery,
) -> Json<Value> {
	let query = decode_form(&query.unwrap_or_default());
	let customer = query
		.iter()
		.find(|(key, _)| key == "customer")
		.map(|(_, value)| value.as_str());
	let data: Vec<Value> = shared
		.inner
		.lock()
		.expect("not poisoned")
		.subscriptions
		.values()
		.filter(|subscription| customer.is_none_or(|customer| subscription["customer"] == customer))
		.cloned()
		.collect();
	Json(json!({ "object": "list", "data": data }))
}

/// Create an active subscription from `items[i][price]` and `metadata[…]`.
/// A repeated idempotency key returns the first response, as Stripe does.
async fn create_subscription(
	State(shared): State<Shared>,
	headers: HeaderMap,
	body: String,
) -> Result<Json<Value>, StatusCode> {
	let recorded = record(&shared, "/v1/subscriptions", headers, &body);
	let key = recorded
		.headers
		.get("idempotency-key")
		.and_then(|key| key.to_str().ok())
		.ok_or(StatusCode::BAD_REQUEST)?
		.to_owned();
	let mut inner = shared.inner.lock().expect("not poisoned");

	if let Some(response) = inner.idempotent.get(&key) {
		return Ok(Json(response.clone()));
	}

	let id = format!("sub_created_{}", inner.idempotent.len() + 1);
	let mut items = Vec::new();

	for index in 0.. {
		let Some(price) = recorded.field(&format!("items[{index}][price]")) else {
			break;
		};
		let lookup_key = price
			.strip_prefix("price_")
			.ok_or(StatusCode::BAD_REQUEST)?;
		items.push(price_item(&format!("si_{id}_{index}"), lookup_key));
	}

	let metadata: serde_json::Map<String, Value> = recorded
		.form
		.iter()
		.filter_map(|(field, value)| {
			let name = field.strip_prefix("metadata[")?.strip_suffix(']')?;
			Some((name.to_owned(), Value::from(value.as_str())))
		})
		.collect();
	let subscription = json!({
		"id": id,
		"object": "subscription",
		"customer": recorded.field("customer").ok_or(StatusCode::BAD_REQUEST)?,
		"status": "active",
		"metadata": metadata,
		"items": { "data": items },
	});
	inner.subscriptions.insert(id, subscription.clone());
	inner.idempotent.insert(key, subscription.clone());

	Ok(Json(subscription))
}

async fn subscription(
	State(shared): State<Shared>,
	Path(id): Path<String>,
) -> Result<Json<Value>, StatusCode> {
	shared
		.inner
		.lock()
		.expect("not poisoned")
		.subscriptions
		.get(&id)
		.cloned()
		.map(Json)
		.ok_or(StatusCode::NOT_FOUND)
}

/// Swap item prices as `items[i][id]` and `items[i][price]` ask.
async fn update_subscription(
	State(shared): State<Shared>,
	Path(id): Path<String>,
	headers: HeaderMap,
	body: String,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
	let recorded = record(&shared, &format!("/v1/subscriptions/{id}"), headers, &body);
	let mut inner = shared.inner.lock().expect("not poisoned");

	if inner.failing_subscriptions > 0 {
		inner.failing_subscriptions -= 1;
		return Err((
			StatusCode::INTERNAL_SERVER_ERROR,
			Json(json!({ "error": { "message": "try again" } })),
		));
	}

	let subscription = inner
		.subscriptions
		.get_mut(&id)
		.ok_or((StatusCode::NOT_FOUND, Json(json!({}))))?;

	for index in 0.. {
		let (Some(item), Some(price)) = (
			recorded.field(&format!("items[{index}][id]")),
			recorded.field(&format!("items[{index}][price]")),
		) else {
			break;
		};
		let lookup_key = price
			.strip_prefix("price_")
			.ok_or_else(|| stripe_error("expected a price"))?;
		let items = subscription["items"]["data"]
			.as_array_mut()
			.ok_or_else(|| stripe_error("no items"))?;
		let entry = items
			.iter_mut()
			.find(|entry| entry["id"] == item)
			.ok_or_else(|| stripe_error("no such item"))?;
		*entry = price_item(item, lookup_key);
	}

	Ok(Json(subscription.clone()))
}

async fn portal_configurations() -> Json<Value> {
	Json(json!({
		"object": "list",
		"data": [
			{ "id": "bpc_other", "metadata": {} },
			{ "id": "bpc_canvist", "metadata": { "canvist_key": "portal" } },
		],
	}))
}

async fn portal_session(
	State(shared): State<Shared>,
	headers: HeaderMap,
	body: String,
) -> Json<Value> {
	record(&shared, "/v1/billing_portal/sessions", headers, &body);
	Json(json!({ "url": "https://billing.stripe.test/session" }))
}

/// `metadata[key]` fields as an object.
fn metadata(recorded: &Recorded) -> serde_json::Map<String, Value> {
	recorded
		.form
		.iter()
		.filter_map(|(field, value)| {
			let name = field.strip_prefix("metadata[")?.strip_suffix(']')?;
			Some((name.to_owned(), Value::from(value.as_str())))
		})
		.collect()
}

fn number(recorded: &Recorded, field: &str) -> Value {
	recorded
		.field(field)
		.and_then(|value| value.parse::<f64>().ok())
		.map_or(Value::Null, Value::from)
}

fn stripe_error(message: &str) -> (StatusCode, Json<Value>) {
	(
		StatusCode::BAD_REQUEST,
		Json(json!({ "error": { "message": message } })),
	)
}

async fn create_coupon(
	State(shared): State<Shared>,
	headers: HeaderMap,
	body: String,
) -> Json<Value> {
	let recorded = record(&shared, "/v1/coupons", headers, &body);
	let mut inner = shared.inner.lock().expect("not poisoned");
	let coupon = json!({
		"id": format!("co_{}", inner.coupons.len() + 1),
		"object": "coupon",
		"name": recorded.field("name"),
		"percent_off": number(&recorded, "percent_off"),
		"amount_off": recorded.field("amount_off").and_then(|value| value.parse::<u64>().ok()),
		"currency": recorded.field("currency"),
		"duration": recorded.field("duration"),
		"duration_in_months": recorded.field("duration_in_months").and_then(|value| value.parse::<u64>().ok()),
		"metadata": metadata(&recorded),
		"valid": true,
		"times_redeemed": 0,
		"created": 1_767_225_600,
	});
	inner.coupons.push(coupon.clone());
	Json(coupon)
}

async fn list_coupons(State(shared): State<Shared>) -> Json<Value> {
	let inner = shared.inner.lock().expect("not poisoned");
	let data: Vec<Value> = inner.coupons.iter().rev().cloned().collect();
	Json(json!({ "object": "list", "data": data }))
}

async fn delete_coupon(
	State(shared): State<Shared>,
	Path(id): Path<String>,
	headers: HeaderMap,
) -> Result<Json<Value>, StatusCode> {
	record(&shared, &format!("DELETE /v1/coupons/{id}"), headers, "");
	let mut inner = shared.inner.lock().expect("not poisoned");
	let before = inner.coupons.len();
	inner.coupons.retain(|coupon| coupon["id"] != id.as_str());

	if inner.coupons.len() == before {
		return Err(StatusCode::NOT_FOUND);
	}

	Ok(Json(json!({ "id": id, "deleted": true })))
}

/// Create a code for `promotion[coupon]`; a repeated active code is refused,
/// as Stripe does.
async fn create_promotion_code(
	State(shared): State<Shared>,
	headers: HeaderMap,
	body: String,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
	let recorded = record(&shared, "/v1/promotion_codes", headers, &body);
	let mut inner = shared.inner.lock().expect("not poisoned");
	let coupon = recorded.field("promotion[coupon]").unwrap_or_default();

	if !inner.coupons.iter().any(|known| known["id"] == coupon) {
		return Err(stripe_error("No such coupon"));
	}

	let code = recorded.field("code").map_or_else(
		|| format!("CODE{}", inner.promotion_codes.len() + 1),
		str::to_owned,
	);

	if inner
		.promotion_codes
		.iter()
		.any(|known| known["code"] == code.as_str() && known["active"] == true)
	{
		return Err(stripe_error(
			"An active promotion code with `code` already exists.",
		));
	}

	let promotion_code = json!({
		"id": format!("promo_{}", inner.promotion_codes.len() + 1),
		"object": "promotion_code",
		"code": code,
		"active": true,
		"promotion": { "type": "coupon", "coupon": coupon },
		"customer": recorded.field("customer"),
		"max_redemptions": recorded.field("max_redemptions").and_then(|value| value.parse::<u64>().ok()),
		"expires_at": recorded.field("expires_at").and_then(|value| value.parse::<u64>().ok()),
		"times_redeemed": 0,
		"metadata": metadata(&recorded),
	});
	inner.promotion_codes.push(promotion_code.clone());
	Ok(Json(promotion_code))
}

async fn list_promotion_codes(
	State(shared): State<Shared>,
	RawQuery(query): RawQuery,
) -> Json<Value> {
	let query = decode_form(&query.unwrap_or_default());
	let field = |name: &str| {
		query
			.iter()
			.find(|(key, _)| key == name)
			.map(|(_, value)| value.as_str())
	};
	let coupon = field("coupon");
	let active_only = field("active") == Some("true");
	let limit = field("limit")
		.and_then(|limit| limit.parse::<usize>().ok())
		.unwrap_or(10);
	let mut codes: Vec<Value> = shared
		.inner
		.lock()
		.expect("not poisoned")
		.promotion_codes
		.iter()
		.rev()
		.cloned()
		.collect();

	// Stripe's cursor is a position in the unfiltered list, so it is applied
	// before the filters are, as deactivating a page does not move it.
	if let Some(after) = field("starting_after")
		&& let Some(position) = codes.iter().position(|code| code["id"] == after)
	{
		codes.drain(..=position);
	}

	codes.retain(|code| {
		(!active_only || code["active"] == true)
			&& coupon.is_none_or(|coupon| code["promotion"]["coupon"] == coupon)
	});
	let has_more = codes.len() > limit;
	codes.truncate(limit);

	Json(json!({ "object": "list", "data": codes, "has_more": has_more }))
}

async fn update_promotion_code(
	State(shared): State<Shared>,
	Path(id): Path<String>,
	headers: HeaderMap,
	body: String,
) -> Result<Json<Value>, StatusCode> {
	let recorded = record(
		&shared,
		&format!("/v1/promotion_codes/{id}"),
		headers,
		&body,
	);
	let mut inner = shared.inner.lock().expect("not poisoned");
	let code = inner
		.promotion_codes
		.iter_mut()
		.find(|code| code["id"] == id.as_str())
		.ok_or(StatusCode::NOT_FOUND)?;

	if let Some(active) = recorded.field("active") {
		code["active"] = Value::from(active == "true");
	}

	Ok(Json(code.clone()))
}

/// Create a customer; a repeated idempotency key returns the first one.
async fn create_customer(
	State(shared): State<Shared>,
	headers: HeaderMap,
	body: String,
) -> Result<Json<Value>, StatusCode> {
	let recorded = record(&shared, "/v1/customers", headers, &body);
	let key = recorded
		.headers
		.get("idempotency-key")
		.and_then(|key| key.to_str().ok())
		.ok_or(StatusCode::BAD_REQUEST)?
		.to_owned();
	let mut inner = shared.inner.lock().expect("not poisoned");

	if let Some(response) = inner.idempotent.get(&key) {
		return Ok(Json(response.clone()));
	}

	inner.customers += 1;
	let customer = json!({
		"id": format!("cus_created_{}", inner.customers),
		"object": "customer",
		"metadata": metadata(&recorded),
	});
	inner.idempotent.insert(key, customer.clone());
	Ok(Json(customer))
}

/// Decode `a=b&c=d` with percent escapes and `+` for spaces.
pub fn decode_form(text: &str) -> Vec<(String, String)> {
	text.split('&')
		.filter_map(|pair| pair.split_once('='))
		.map(|(key, value)| (decode(key), decode(value)))
		.collect()
}

fn decode(text: &str) -> String {
	let bytes = text.as_bytes();
	let mut out = Vec::new();
	let mut index = 0;

	while index < bytes.len() {
		match bytes[index] {
			b'%' => {
				let hex = std::str::from_utf8(&bytes[index + 1..index + 3]).expect("ascii");
				out.push(u8::from_str_radix(hex, 16).expect("hex"));
				index += 3;
			}
			b'+' => {
				out.push(b' ');
				index += 1;
			}
			byte => {
				out.push(byte);
				index += 1;
			}
		}
	}

	String::from_utf8(out).expect("utf-8")
}
