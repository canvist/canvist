//! Coupons: discounts the admin creates in Stripe, public or private.
//!
//! A coupon is Stripe's discount (a percentage or an amount, once, for some
//! months, or forever); customers redeem it through a promotion code. Each
//! coupon made here gets one code:
//!
//! - **Public:** a code anyone can type at Checkout, such as `LAUNCH50`.
//! - **Private:** a code only one project's Stripe customer can redeem. A
//!   project without a customer gets one. If the project already pays, the
//!   code is applied to its subscription at once. 100% off forever is how
//!   a project runs for free; 100% off for three months is a free trial
//!   that still collects a card.
//!
//! Stripe is the only record: coupons carry `canvist_visibility`,
//! `canvist_project` and `canvist_created_by` in their metadata. Lists show
//! the newest 100 coupons and codes.

use axum::Json;
use axum::extract::Path;
use axum::extract::State;
use axum::http::HeaderMap;
use axum::http::StatusCode;
use serde::Deserialize;
use serde::Serialize;
use serde_json::Value as JsonValue;
use serde_json::json;

use super::Admin;
use crate::app::App;
use crate::app::SharedApp;
use crate::error::ApiError;
use crate::metering::Terms;
use crate::metering::billing::BillingError;
use crate::metering::billing::form;
use crate::metering::billing::list;
use crate::metering::billing::text;
use crate::metering::stripe::StripeClient;
use crate::names::ProjectId;
use crate::store::Store;

/// Stripe's limit on a coupon's name.
const MAX_NAME: usize = 40;
/// The longest repeating discount offered, in months.
const MAX_MONTHS: u32 = 36;
const VISIBILITY: &str = "canvist_visibility";
const PROJECT: &str = "canvist_project";
const CREATED_BY: &str = "canvist_created_by";

/// A coupon to create.
#[derive(Debug, Deserialize)]
pub struct NewCoupon {
	/// Shown to customers on invoices and receipts; at most 40 characters.
	pub name: String,
	/// How much it takes off.
	#[serde(flatten)]
	pub discount: Discount,
	/// How long it lasts on a subscription.
	pub duration: Duration,
	/// Months a `repeating` coupon lasts.
	pub duration_in_months: Option<u32>,
	/// Who may redeem it.
	#[serde(flatten)]
	pub audience: Audience,
	/// Redemptions allowed in all.
	pub max_redemptions: Option<u32>,
	/// When the code stops working, in Unix seconds.
	pub expires_at: Option<u64>,
}

/// How much a coupon takes off.
#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub enum Discount {
	/// A share of each invoice, above 0 and at most 100.
	Percent {
		/// The percentage.
		percent_off: f64,
	},
	/// A fixed amount off each invoice, in the currency's smallest unit.
	Amount {
		/// Cents, for `usd`.
		amount_off: u64,
		/// A three-letter ISO currency code.
		currency: String,
	},
}

/// How long a coupon lasts on a subscription.
#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Duration {
	/// The first invoice.
	Once,
	/// `duration_in_months` months of invoices.
	Repeating,
	/// Every invoice.
	Forever,
}

impl Duration {
	fn as_str(self) -> &'static str {
		match self {
			Self::Once => "once",
			Self::Repeating => "repeating",
			Self::Forever => "forever",
		}
	}
}

/// Who may redeem a coupon.
#[derive(Debug, Deserialize)]
#[serde(tag = "visibility", rename_all = "lowercase")]
pub enum Audience {
	/// Anyone, with the code: one given, or one Stripe makes up.
	Public {
		/// Letters, digits and dashes.
		code: Option<String>,
	},
	/// Only this project.
	Private {
		/// The project.
		project: ProjectId,
	},
}

impl NewCoupon {
	/// Why the coupon cannot be made, if it cannot.
	fn problem(&self) -> Option<&'static str> {
		let name = self.name.trim();

		if name.is_empty() || name.chars().count() > MAX_NAME {
			return Some("name must be 1 to 40 characters");
		}

		match &self.discount {
			Discount::Percent { percent_off } if !(*percent_off > 0.0 && *percent_off <= 100.0) => {
				return Some("percent_off must be above 0 and at most 100");
			}
			Discount::Amount { amount_off: 0, .. } => {
				return Some("amount_off must be positive");
			}
			Discount::Amount { currency, .. }
				if currency.len() != 3 || !currency.chars().all(|c| c.is_ascii_lowercase()) =>
			{
				return Some("currency must be a lowercase ISO code such as usd");
			}
			_ => {}
		}

		match (self.duration, self.duration_in_months) {
			(Duration::Repeating, Some(months)) if (1..=MAX_MONTHS).contains(&months) => {}
			(Duration::Repeating, _) => {
				return Some("a repeating coupon needs duration_in_months from 1 to 36");
			}
			(_, Some(_)) => return Some("only a repeating coupon takes duration_in_months"),
			(_, None) => {}
		}

		if let Audience::Public { code: Some(code) } = &self.audience
			&& !is_code(code)
		{
			return Some("code must be 3 to 40 letters, digits or dashes");
		}

		(self.max_redemptions == Some(0)).then_some("max_redemptions must be positive")
	}
}

fn is_code(code: &str) -> bool {
	(3..=40).contains(&code.len())
		&& code
			.chars()
			.all(|character| character.is_ascii_alphanumeric() || character == '-')
}

/// A coupon as the dashboard shows it.
#[derive(Debug, Serialize)]
pub struct CouponView {
	id: String,
	name: Option<String>,
	percent_off: Option<f64>,
	amount_off: Option<u64>,
	currency: Option<String>,
	duration: Option<String>,
	duration_in_months: Option<u64>,
	/// Whether it can still be redeemed.
	valid: bool,
	times_redeemed: u64,
	created: u64,
	/// `public` or `private`.
	visibility: &'static str,
	/// The project a private coupon is for.
	project: Option<String>,
	created_by: Option<String>,
	codes: Vec<CodeView>,
}

/// A promotion code as the dashboard shows it.
#[derive(Debug, Serialize)]
pub struct CodeView {
	id: String,
	code: String,
	active: bool,
	customer: Option<String>,
	times_redeemed: u64,
	max_redemptions: Option<u64>,
	expires_at: Option<u64>,
}

impl CodeView {
	fn of(code: &JsonValue) -> Option<Self> {
		Some(Self {
			id: text(code, "id")?,
			code: text(code, "code")?,
			active: code
				.get("active")
				.and_then(JsonValue::as_bool)
				.unwrap_or(false),
			customer: text(code, "customer"),
			times_redeemed: number(code, "times_redeemed").unwrap_or(0),
			max_redemptions: number(code, "max_redemptions"),
			expires_at: number(code, "expires_at"),
		})
	}
}

impl CouponView {
	fn of(coupon: &JsonValue, codes: &[JsonValue]) -> Option<Self> {
		let id = text(coupon, "id")?;
		let metadata = |key| {
			coupon
				.get("metadata")
				.and_then(|metadata| text(metadata, key))
		};
		let codes: Vec<CodeView> = codes
			.iter()
			.filter(|code| coupon_of(code).as_deref() == Some(&id))
			.filter_map(CodeView::of)
			.collect();
		// Coupons made elsewhere have no visibility: a code bound to a
		// customer makes one private.
		let private = match metadata(VISIBILITY).as_deref() {
			Some(visibility) => visibility == "private",
			None => codes.iter().any(|code| code.customer.is_some()),
		};
		let visibility = if private { "private" } else { "public" };

		Some(Self {
			name: text(coupon, "name"),
			percent_off: coupon.get("percent_off").and_then(JsonValue::as_f64),
			amount_off: number(coupon, "amount_off"),
			currency: text(coupon, "currency"),
			duration: text(coupon, "duration"),
			duration_in_months: number(coupon, "duration_in_months"),
			valid: coupon
				.get("valid")
				.and_then(JsonValue::as_bool)
				.unwrap_or(false),
			times_redeemed: number(coupon, "times_redeemed").unwrap_or(0),
			created: number(coupon, "created").unwrap_or(0),
			visibility,
			project: metadata(PROJECT),
			created_by: metadata(CREATED_BY),
			codes,
			id,
		})
	}
}

fn number(value: &JsonValue, field: &str) -> Option<u64> {
	value.get(field)?.as_u64()
}

/// The coupon a promotion code redeems: `promotion.coupon` in this API
/// version, as an id or an expanded object.
fn coupon_of(code: &JsonValue) -> Option<String> {
	let coupon = code.get("promotion")?.get("coupon")?;
	coupon
		.as_str()
		.map(str::to_owned)
		.or_else(|| text(coupon, "id"))
}

/// The configured Stripe client, or `404` where billing is off.
fn stripe(app: &App) -> Result<&StripeClient, ApiError> {
	app.stripe
		.as_ref()
		.ok_or_else(|| BillingError::NotConfigured.into())
}

/// `GET /v1/admin/coupons`: the newest coupons, with their codes.
pub(super) async fn list_coupons(
	State(app): State<SharedApp>,
	headers: HeaderMap,
) -> Result<Json<JsonValue>, ApiError> {
	app.admins.authorize(&headers)?;
	let coupons = list_all(stripe(&app)?).await?;
	Ok(Json(json!({ "coupons": coupons })))
}

/// `POST /v1/admin/coupons`: create a coupon and its code.
pub(super) async fn create_coupon(
	State(app): State<SharedApp>,
	headers: HeaderMap,
	Json(request): Json<NewCoupon>,
) -> Result<(StatusCode, Json<JsonValue>), ApiError> {
	let admin = app.admins.authorize(&headers)?;

	if let Some(problem) = request.problem() {
		return Err(ApiError::bad_request(problem));
	}

	let created = create(stripe(&app)?, &app.store, &request, &admin).await?;
	tracing::info!(%admin, coupon = %created.coupon.id, visibility = created.coupon.visibility, "created a coupon");

	Ok((
		StatusCode::CREATED,
		Json(json!({
			"coupon": created.coupon,
			"applied_to_subscription": created.applied,
		})),
	))
}

/// `DELETE /v1/admin/coupons/{id}`: switch off its codes and delete it.
/// Subscriptions that already redeemed it keep the discount until it ends.
pub(super) async fn remove_coupon(
	State(app): State<SharedApp>,
	Path(id): Path<String>,
	headers: HeaderMap,
) -> Result<StatusCode, ApiError> {
	let admin = app.admins.authorize(&headers)?;

	// The id goes into a Stripe URL path.
	if id.is_empty()
		|| id.len() > 100
		|| !id
			.chars()
			.all(|character| character.is_ascii_alphanumeric() || matches!(character, '-' | '_'))
	{
		return Err(ApiError::not_found("no such coupon"));
	}

	remove(stripe(&app)?, &id).await?;
	tracing::info!(%admin, coupon = %id, "deleted a coupon");
	Ok(StatusCode::NO_CONTENT)
}

/// The newest coupons with their codes.
async fn list_all(stripe: &StripeClient) -> Result<Vec<CouponView>, BillingError> {
	let coupons = stripe.get("/v1/coupons", &[("limit", "100")]).await?;
	let codes = stripe
		.get("/v1/promotion_codes", &[("limit", "100")])
		.await?;

	Ok(list(&coupons)
		.iter()
		.filter_map(|coupon| CouponView::of(coupon, list(&codes)))
		.collect())
}

/// A new coupon, and whether it was applied to a subscription.
struct Created {
	coupon: CouponView,
	applied: bool,
}

/// Create the coupon, then its code. A private coupon's project gets a
/// customer first, and a paying project gets the code on its subscription.
async fn create(
	stripe: &StripeClient,
	store: &Store,
	request: &NewCoupon,
	admin: &Admin,
) -> Result<Created, BillingError> {
	let terms = match &request.audience {
		Audience::Public { .. } => None,
		Audience::Private { project } => Some(customer_terms(stripe, store, project).await?),
	};
	let customer = terms
		.as_ref()
		.and_then(|terms| terms.stripe_customer.as_deref());
	let coupon = stripe
		.post("/v1/coupons", &coupon_form(request, admin))
		.await?;
	let id = text(&coupon, "id").ok_or(BillingError::Malformed("a coupon without an id"))?;
	let code = create_code(stripe, &id, &code_form(request, &id, customer)).await?;
	let applied = match terms.as_ref() {
		Some(terms) => {
			match apply(stripe, terms, &code).await {
				Ok(applied) => applied,
				Err(error) => {
					// The code never reached the subscription: undo both, so a
					// retry is not refused for a duplicate code and leaves no
					// coupon nobody can redeem.
					discard(stripe, &id, &code).await;
					return Err(error);
				}
			}
		}
		None => false,
	};

	Ok(Created {
		coupon: CouponView::of(&coupon, &[code])
			.ok_or(BillingError::Malformed("a coupon without an id"))?,
		applied,
	})
}

/// Create coupon `id`'s code. Without its code the coupon is unreachable,
/// so a failure deletes the coupon again.
async fn create_code(
	stripe: &StripeClient,
	id: &str,
	fields: &[(String, String)],
) -> Result<JsonValue, BillingError> {
	let error = match stripe.post("/v1/promotion_codes", fields).await {
		Ok(code) => return Ok(code),
		Err(error) => error,
	};

	if let Err(cleanup) = stripe.delete(&format!("/v1/coupons/{id}")).await {
		tracing::warn!(coupon = %id, error = %cleanup, "could not delete an orphaned coupon");
	}

	Err(error.into())
}

/// Undo a coupon whose code could not be applied to a subscription: switch
/// the code off, then delete the coupon, so the caller's retry makes one
/// coupon rather than two. Cleanup failures are logged, not returned: the
/// error worth reporting is the one that stopped the creation.
async fn discard(stripe: &StripeClient, coupon: &str, code: &JsonValue) {
	if let Some(code) = text(code, "id")
		&& let Err(error) = deactivate(stripe, &code).await
	{
		tracing::warn!(%code, %error, "could not switch off an unapplied promotion code");
	}

	if let Err(error) = stripe.delete(&format!("/v1/coupons/{coupon}")).await {
		tracing::warn!(coupon = %coupon, %error, "could not delete an unapplied coupon");
	}
}

/// Switch off promotion code `id`, so it can no longer be redeemed.
async fn deactivate(stripe: &StripeClient, id: &str) -> Result<(), BillingError> {
	stripe
		.post(
			&format!("/v1/promotion_codes/{id}"),
			&form(&[("active", "false")]),
		)
		.await?;
	Ok(())
}

/// Put `code` on the subscription `terms` pays with, if one does. Returns
/// whether it did.
async fn apply(
	stripe: &StripeClient,
	terms: &Terms,
	code: &JsonValue,
) -> Result<bool, BillingError> {
	let Some(subscription) = terms
		.subscription
		.as_ref()
		.filter(|_| terms.is_subscribed())
	else {
		return Ok(false);
	};
	let code = text(code, "id").ok_or(BillingError::Malformed("a code without an id"))?;
	stripe
		.post(
			&format!("/v1/subscriptions/{}", subscription.id),
			&form(&[("discounts[0][promotion_code]", &code)]),
		)
		.await?;

	Ok(true)
}

/// `project`'s terms, with a Stripe customer made for it if it had none.
/// The idempotency key is the project's, so requests make one customer.
async fn customer_terms(
	stripe: &StripeClient,
	store: &Store,
	project: &ProjectId,
) -> Result<Terms, BillingError> {
	let terms = store.terms(project).await?;

	if terms.stripe_customer.is_some() {
		return Ok(terms);
	}

	let customer = stripe
		.post_once(
			"/v1/customers",
			&form(&[("metadata[canvist_project]", project.as_str())]),
			&format!("canvist-customer-{project}"),
		)
		.await?;
	let customer =
		text(&customer, "id").ok_or(BillingError::Malformed("a customer without an id"))?;
	// Only the customer column, and only while it is empty: a Stripe webhook
	// may be writing the subscription beside it at this moment, and terms
	// read before the request above would put back the old ones.
	let stored = store
		.set_stripe_customer_if_absent(project, &customer)
		.await?;

	Ok(Terms {
		stripe_customer: stored,
		..terms
	})
}

fn coupon_form(request: &NewCoupon, admin: &Admin) -> Vec<(String, String)> {
	let (visibility, project) = match &request.audience {
		Audience::Public { .. } => ("public", None),
		Audience::Private { project } => ("private", Some(project.as_str())),
	};
	let mut fields = form(&[
		("name", request.name.trim()),
		("duration", request.duration.as_str()),
		("metadata[canvist_visibility]", visibility),
		("metadata[canvist_created_by]", &admin.to_string()),
	]);

	match &request.discount {
		Discount::Percent { percent_off } => {
			fields.extend(form(&[("percent_off", &percent_off.to_string())]));
		}
		Discount::Amount {
			amount_off,
			currency,
		} => {
			fields.extend(form(&[
				("amount_off", &amount_off.to_string()),
				("currency", currency),
			]));
		}
	}

	if let Some(months) = request.duration_in_months {
		fields.extend(form(&[("duration_in_months", &months.to_string())]));
	}

	if let Some(project) = project {
		fields.extend(form(&[("metadata[canvist_project]", project)]));
	}

	fields
}

fn code_form(request: &NewCoupon, coupon: &str, customer: Option<&str>) -> Vec<(String, String)> {
	let mut fields = form(&[("promotion[type]", "coupon"), ("promotion[coupon]", coupon)]);
	let optional = [
		(
			"code",
			match &request.audience {
				Audience::Public { code } => code.clone(),
				Audience::Private { .. } => None,
			},
		),
		("customer", customer.map(str::to_owned)),
		(
			"max_redemptions",
			request.max_redemptions.map(|count| count.to_string()),
		),
		(
			"expires_at",
			request.expires_at.map(|seconds| seconds.to_string()),
		),
	];

	fields.extend(
		optional
			.into_iter()
			.filter_map(|(name, value)| Some((name.to_owned(), value?))),
	);
	fields
}

/// Switch off `id`'s active codes, every page of them, then delete it.
async fn remove(stripe: &StripeClient, id: &str) -> Result<(), BillingError> {
	let mut starting_after: Option<String> = None;

	loop {
		let mut query = vec![("coupon", id), ("limit", "100"), ("active", "true")];

		if let Some(after) = starting_after.as_deref() {
			query.push(("starting_after", after));
		}

		let codes = stripe.get("/v1/promotion_codes", &query).await?;
		let page = list(&codes);

		for code in page {
			if let Some(code) = text(code, "id") {
				deactivate(stripe, &code).await?;
			}
		}

		let more = codes
			.get("has_more")
			.and_then(JsonValue::as_bool)
			.unwrap_or(false);

		match (more, page.last().and_then(|code| text(code, "id"))) {
			(true, Some(last)) => starting_after = Some(last),
			_ => break,
		}
	}

	stripe.delete(&format!("/v1/coupons/{id}")).await?;
	Ok(())
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::metering::Plan;
	use crate::metering::Subscription;
	use crate::metering::SubscriptionStatus;

	fn coupon(body: JsonValue) -> NewCoupon {
		serde_json::from_value(body).expect("a coupon")
	}

	#[test]
	fn requests_read_their_discount_and_audience() {
		let public = coupon(json!({
			"name": "Launch", "percent_off": 50, "duration": "once",
			"visibility": "public", "code": "LAUNCH50",
		}));
		assert!(
			matches!(public.discount, Discount::Percent { percent_off } if (percent_off - 50.0).abs() < f64::EPSILON)
		);
		assert!(
			matches!(&public.audience, Audience::Public { code: Some(code) } if code == "LAUNCH50")
		);
		assert_eq!(public.problem(), None);

		let private = coupon(json!({
			"name": "Free forever", "amount_off": 2900, "currency": "usd",
			"duration": "repeating", "duration_in_months": 3,
			"visibility": "private", "project": "prj_abc",
		}));
		assert!(matches!(
			private.discount,
			Discount::Amount {
				amount_off: 2900,
				..
			}
		));
		assert!(
			matches!(&private.audience, Audience::Private { project } if project.as_str() == "prj_abc")
		);
		assert_eq!(private.problem(), None);
	}

	#[test]
	fn impossible_coupons_are_refused() {
		let base =
			json!({ "name": "x", "percent_off": 10, "duration": "once", "visibility": "public" });
		let with = |field: &str, value: JsonValue| {
			let mut body = base.clone();
			body[field] = value;
			coupon(body).problem()
		};

		assert!(with("percent_off", json!(0)).is_some());
		assert!(with("percent_off", json!(100.5)).is_some());
		assert!(with("name", json!("")).is_some());
		assert!(with("name", json!("a".repeat(41))).is_some());
		assert!(with("duration_in_months", json!(3)).is_some());
		assert!(with("duration", json!("repeating")).is_some());
		assert!(with("code", json!("has space")).is_some());
		assert!(with("max_redemptions", json!(0)).is_some());
		assert_eq!(with("percent_off", json!(100)), None);
	}

	#[test]
	fn views_group_codes_under_their_coupon() {
		let coupon = json!({
			"id": "co_1", "name": "Friends", "percent_off": 100.0, "duration": "forever",
			"valid": true, "times_redeemed": 1, "created": 10,
			"metadata": { "canvist_visibility": "private", "canvist_project": "prj_abc" },
		});
		let codes = [
			json!({ "id": "promo_1", "code": "FRIENDS", "active": true, "customer": "cus_1",
				"promotion": { "type": "coupon", "coupon": "co_1" }, "times_redeemed": 1 }),
			json!({ "id": "promo_2", "code": "OTHER", "active": true,
				"promotion": { "type": "coupon", "coupon": "co_2" } }),
		];
		let view = CouponView::of(&coupon, &codes).expect("a view");

		assert_eq!(view.visibility, "private");
		assert_eq!(view.project.as_deref(), Some("prj_abc"));
		assert_eq!(view.codes.len(), 1);
		assert_eq!(view.codes[0].code, "FRIENDS");
	}

	#[test]
	fn private_codes_are_bound_to_the_customer() {
		let request = coupon(json!({
			"name": "Mine", "percent_off": 100, "duration": "forever",
			"visibility": "private", "project": "prj_abc", "max_redemptions": 1,
		}));
		let fields = code_form(&request, "co_1", Some("cus_1"));
		let field = |name: &str| {
			fields
				.iter()
				.find(|(key, _)| key == name)
				.map(|(_, value)| value.as_str())
		};

		assert_eq!(field("promotion[coupon]"), Some("co_1"));
		assert_eq!(field("customer"), Some("cus_1"));
		assert_eq!(field("max_redemptions"), Some("1"));
		assert_eq!(field("code"), None);
	}

	#[tokio::test]
	async fn a_customer_made_for_a_coupon_leaves_a_subscription_alone() {
		let store = Store::memory();
		let project = store.create_project("coupons").await.expect("project");
		let subscription = Subscription {
			id: "sub_1".to_owned(),
			status: SubscriptionStatus::Active,
		};
		store
			.set_terms(
				&project,
				&Terms {
					subscription: Some(subscription.clone()),
					..Terms::of(Plan::Builder)
				},
			)
			.await
			.expect("terms");

		let stored = store
			.set_stripe_customer_if_absent(&project, "cus_1")
			.await
			.expect("sets the customer");
		assert_eq!(stored.as_deref(), Some("cus_1"));
		let kept = store
			.set_stripe_customer_if_absent(&project, "cus_2")
			.await
			.expect("keeps the first");
		assert_eq!(kept.as_deref(), Some("cus_1"), "the first customer wins");

		let terms = store.terms(&project).await.expect("terms");
		assert_eq!(terms.stripe_customer.as_deref(), Some("cus_1"));
		assert_eq!(
			terms.subscription,
			Some(subscription),
			"a webhook's subscription is not put back"
		);
	}
}
