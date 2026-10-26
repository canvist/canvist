//! Admission precedes body reads, including when uploads stall.

use std::convert::Infallible;
use std::sync::Arc;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;
use std::time::Duration;

use axum::body::Body;
use axum::body::Bytes;
use axum::body::to_bytes;
use axum::http::Request;
use futures_util::stream;
use tokio::sync::oneshot;
use tower::ServiceExt;

use super::*;
use crate::Config;
use crate::Server;

/// A running server and an API key, using only the in-memory store.
async fn fixture() -> (Server, String) {
	let mut config = Config::local("test-secret-test-secret-test-secret!");
	config.convert.concurrency = 1;
	config.convert.max_input_bytes = 1024;
	let server = Server::start(config).await.expect("server starts");
	let store = &server.app().store;
	let project = store.create_project("admission").await.expect("project");
	let environment = Environment::parse("production").expect("environment");
	let key = auth::generate_api_key();
	store
		.add_api_key(&project, &environment, &key)
		.await
		.expect("key");

	(server, key.secret)
}

/// Build a JSON request without consuming its body.
fn request(key: &str, body: Body) -> Request<Body> {
	Request::post("/v1/convert")
		.header("authorization", format!("Bearer {key}"))
		.header(CONTENT_TYPE, "application/json")
		.body(body)
		.expect("request")
}

#[tokio::test]
async fn saturated_conversions_reject_without_reading_bodies_and_recover() {
	let (server, key) = fixture().await;
	let router = router(server.app().clone());
	let (reading, started) = oneshot::channel();
	let stalled = Body::from_stream(stream::once(async move {
		reading.send(()).expect("reader waiting");
		std::future::pending::<Result<Bytes, Infallible>>().await
	}));
	let uploading = tokio::spawn(router.clone().oneshot(request(&key, stalled)));
	started.await.expect("body read starts");
	let reads = Arc::new(AtomicUsize::new(0));
	let payload = Bytes::from(format!(
		r#"{{"from":"text","to":"html","input":"{}"}}"#,
		"x".repeat(960)
	));

	for _ in 0..32 {
		let reads = reads.clone();
		let payload = payload.clone();
		let body = Body::from_stream(stream::once(async move {
			reads.fetch_add(1, Ordering::SeqCst);
			Ok::<_, Infallible>(payload)
		}));
		let response = tokio::time::timeout(
			Duration::from_secs(1),
			router.clone().oneshot(request(&key, body)),
		)
		.await
		.expect("saturation rejects promptly")
		.expect("response");
		assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
		let body = to_bytes(response.into_body(), 1024)
			.await
			.expect("error body");
		let error: JsonValue = serde_json::from_slice(&body).expect("JSON error");
		assert_eq!(error["error"]["code"], "converter_unavailable");
	}

	assert_eq!(
		reads.load(Ordering::SeqCst),
		0,
		"rejected bodies stay unread"
	);
	uploading.abort();
	assert!(
		uploading
			.await
			.expect_err("cancelled upload")
			.is_cancelled()
	);
	let response = router
		.oneshot(request(&key, Body::from(payload)))
		.await
		.expect("response");
	assert_eq!(
		response.status(),
		StatusCode::OK,
		"cancellation frees capacity"
	);
	server.shutdown().await;
}

#[tokio::test]
async fn unauthenticated_conversions_reject_without_reading_the_body() {
	let (server, _) = fixture().await;
	let unread = Body::from_stream(stream::pending::<Result<Bytes, Infallible>>());
	let response = tokio::time::timeout(
		Duration::from_secs(1),
		router(server.app().clone()).oneshot(request("invalid", unread)),
	)
	.await
	.expect("authentication rejects before reading")
	.expect("response");
	assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
	server.shutdown().await;
}

#[tokio::test]
async fn rejected_inputs_release_conversion_capacity() {
	let (server, key) = fixture().await;
	let router = router(server.app().clone());

	for (input, status) in [
		("{".to_owned(), StatusCode::BAD_REQUEST),
		("{}".to_owned(), StatusCode::UNPROCESSABLE_ENTITY),
		(
			r#"{"from":"unknown","to":"html","input":"x"}"#.to_owned(),
			StatusCode::BAD_REQUEST,
		),
		(
			r#"{"from":"json","to":"html","input":"invalid"}"#.to_owned(),
			StatusCode::UNPROCESSABLE_ENTITY,
		),
		("x".repeat(1025), StatusCode::PAYLOAD_TOO_LARGE),
		(
			r#"{"from":"text","to":"html","input":"x"}"#.to_owned(),
			StatusCode::OK,
		),
	] {
		let response = router
			.clone()
			.oneshot(request(&key, Body::from(input)))
			.await
			.expect("response");
		assert_eq!(response.status(), status);
	}

	server.shutdown().await;
}

#[tokio::test]
async fn exports_admit_before_loading_and_raw_yjs_bypasses_conversion_capacity() {
	let (server, key) = fixture().await;
	let router = router(server.app().clone());
	let conversion = server.app().converter.try_admit().expect("capacity");

	for (path, status) in [
		(
			"/v1/documents/missing?format=text",
			StatusCode::SERVICE_UNAVAILABLE,
		),
		("/v1/documents/missing?format=yjs", StatusCode::NOT_FOUND),
	] {
		let request = Request::get(path)
			.header("authorization", format!("Bearer {key}"))
			.body(Body::empty())
			.expect("request");
		let response = router.clone().oneshot(request).await.expect("response");
		assert_eq!(response.status(), status);
	}

	drop(conversion);
	server.shutdown().await;
}
