//! Projects: creating them and their keys, listing them, and setting their
//! plans by hand.

use axum::Json;
use axum::extract::Path;
use axum::extract::State;
use axum::http::HeaderMap;
use axum::http::StatusCode;
use serde::Deserialize;
use serde_json::Value as JsonValue;
use serde_json::json;

use crate::analytics;
use crate::app::SharedApp;
use crate::auth;
use crate::error::ApiError;
use crate::metering::Month;
use crate::metering::Plan;
use crate::metering::Standing;
use crate::metering::Terms;
use crate::names::Environment;
use crate::names::ProjectId;

/// The longest project name, in bytes.
const MAX_PROJECT_NAME_BYTES: usize = 128;
/// Projects listed at most.
const MAX_PROJECTS: usize = 500;

/// `GET /v1/admin/projects`: the newest projects, with this month's usage.
pub(super) async fn list_projects(
	State(app): State<SharedApp>,
	headers: HeaderMap,
) -> Result<Json<JsonValue>, ApiError> {
	app.admins.authorize(&headers)?;
	let month = Month::current();
	let projects: Vec<JsonValue> = app
		.store
		.list_projects(month, MAX_PROJECTS)
		.await?
		.into_iter()
		.map(|project| {
			let terms = &project.terms;
			json!({
				"id": project.id,
				"name": project.name,
				"plan": terms.plan,
				"included_documents": terms.included_documents,
				"active_documents": project.active_documents,
				"standing": Standing::of(terms, project.active_documents),
				"subscription_status": terms.subscription.as_ref().map(|subscription| subscription.status),
				"stripe_customer": terms.stripe_customer,
			})
		})
		.collect();

	Ok(Json(
		json!({ "month": month.to_string(), "projects": projects }),
	))
}

#[derive(Debug, Deserialize)]
pub(super) struct ProjectRequest {
	name: String,
	#[serde(default = "default_environments")]
	environments: Vec<Environment>,
}

fn default_environments() -> Vec<Environment> {
	["production", "staging", "development"]
		.into_iter()
		.filter_map(|name| Environment::parse(name).ok())
		.collect()
}

/// `POST /v1/admin/projects`: create a project and one API key per
/// environment.
pub(super) async fn create_project(
	State(app): State<SharedApp>,
	headers: HeaderMap,
	Json(request): Json<ProjectRequest>,
) -> Result<(StatusCode, Json<JsonValue>), ApiError> {
	let admin = app.admins.authorize(&headers)?;

	if request.name.is_empty() || request.name.len() > MAX_PROJECT_NAME_BYTES {
		return Err(ApiError::bad_request("name must be 1 to 128 bytes"));
	}

	let project = app.store.create_project(&request.name).await?;
	let mut keys = serde_json::Map::new();

	for environment in &request.environments {
		let key = auth::generate_api_key();
		app.store.add_api_key(&project, environment, &key).await?;
		keys.insert(environment.to_string(), key.secret.into());
	}

	tracing::info!(%admin, %project, "created a project");
	analytics::capture(
		&app.store,
		"project created",
		&format!("project:{project}"),
		serde_json::Map::new(),
	)
	.await;

	Ok((
		StatusCode::CREATED,
		Json(json!({ "project": project, "name": request.name, "keys": keys })),
	))
}

#[derive(Debug, Deserialize)]
pub(super) struct KeyRequest {
	environment: Environment,
}

/// `POST /v1/admin/projects/{project}/keys`: another API key for an
/// environment.
pub(super) async fn create_key(
	State(app): State<SharedApp>,
	Path(project): Path<String>,
	headers: HeaderMap,
	Json(request): Json<KeyRequest>,
) -> Result<(StatusCode, Json<JsonValue>), ApiError> {
	app.admins.authorize(&headers)?;
	let project = ProjectId::new(project);
	let key = auth::generate_api_key();
	app.store
		.add_api_key(&project, &request.environment, &key)
		.await?;
	Ok((
		StatusCode::CREATED,
		Json(json!({ "project": project, "environment": request.environment, "key": key.secret })),
	))
}

#[derive(Debug, Deserialize)]
pub(super) struct TermsRequest {
	plan: Plan,
	included_documents: Option<u64>,
	stripe_customer: Option<String>,
}

/// `PUT /v1/admin/projects/{project}/terms`: change a project's plan.
pub(super) async fn set_terms(
	State(app): State<SharedApp>,
	Path(project): Path<String>,
	headers: HeaderMap,
	Json(request): Json<TermsRequest>,
) -> Result<Json<JsonValue>, ApiError> {
	let admin = app.admins.authorize(&headers)?;
	let project = ProjectId::new(project);
	let current = app.store.terms(&project).await?;
	let terms = Terms {
		plan: request.plan,
		included_documents: request
			.included_documents
			.or_else(|| request.plan.included_documents()),
		stripe_customer: request.stripe_customer,
		subscription: current.subscription,
	};
	app.store.set_terms(&project, &terms).await?;
	tracing::info!(%admin, %project, plan = terms.plan.as_str(), "set a project's terms");

	Ok(Json(json!({ "project": project, "terms": terms })))
}

/// `GET /v1/admin/stats`: the relay's loaded rooms and connections.
pub(super) async fn stats(
	State(app): State<SharedApp>,
	headers: HeaderMap,
) -> Result<Json<JsonValue>, ApiError> {
	app.admins.authorize(&headers)?;
	Ok(Json(json!(app.rooms.stats())))
}
