//! Token lifetimes and refreshes are enforced on real WebSocket connections.

mod support;

use std::time::Duration;

use canvist_cloud::auth;
use canvist_cloud::auth::Scope;
use canvist_cloud::metering::Plan;
use canvist_cloud::metering::Terms;
use canvist_cloud::sync::protocol;
use futures_util::SinkExt;
use futures_util::StreamExt;
use support::Fixture;
use tokio_tungstenite::tungstenite::Message;
use yrs::StateVector;
use yrs::encoding::read::Cursor;
use yrs::encoding::read::Read;
use yrs::encoding::write::Write;
use yrs::updates::encoder::Encoder;
use yrs::updates::encoder::EncoderV1;

type Socket =
	tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>;

async fn connect(url: &str) -> Socket {
	tokio_tungstenite::connect_async(url)
		.await
		.expect("connects")
		.0
}

async fn send(socket: &mut Socket, bytes: Vec<u8>) {
	socket
		.send(Message::Binary(bytes.into()))
		.await
		.expect("sends");
}

async fn next(socket: &mut Socket) -> Message {
	tokio::time::timeout(Duration::from_secs(8), socket.next())
		.await
		.expect("responds before the deadline")
		.expect("connection remains open")
		.expect("valid frame")
}

async fn binary(socket: &mut Socket) -> Vec<u8> {
	loop {
		match next(socket).await {
			Message::Binary(bytes) => return bytes.to_vec(),
			Message::Close(reason) => panic!("unexpected close: {reason:?}"),
			_ => {}
		}
	}
}

fn frame(address: &str, message: &[u8]) -> Vec<u8> {
	let mut encoder = EncoderV1::new();
	encoder.write_string(address);
	let mut bytes = encoder.to_vec();
	bytes.extend_from_slice(message);
	bytes
}

async fn authenticate(socket: &mut Socket, address: &str, token: &str) -> (u8, String) {
	let mut encoder = EncoderV1::new();
	encoder.write_var(2_u8);
	encoder.write_var(0_u8);
	encoder.write_string(token);
	send(socket, frame(address, &encoder.to_vec())).await;

	loop {
		let bytes = binary(socket).await;
		let mut cursor = Cursor::new(&bytes);
		let document = cursor.read_string().expect("address").to_owned();
		let kind: u8 = cursor.read_var().expect("kind");

		if document == address && kind == 2 {
			return (
				cursor.read_var().expect("auth kind"),
				cursor.read_string().expect("reason").to_owned(),
			);
		}
	}
}

async fn expired(socket: &mut Socket, active: bool) {
	let deadline = tokio::time::Instant::now() + Duration::from_secs(8);
	let mut activity = tokio::time::interval(Duration::from_millis(100));

	loop {
		let message = tokio::select! {
			biased;
			message = tokio::time::timeout_at(deadline, socket.next()) => {
				message.expect("expired connection must close")
					.expect("close frame").expect("valid frame")
			}
			_ = activity.tick(), if active => {
				socket.send(Message::Ping(Vec::new().into())).await.expect("keep alive");
				continue;
			}
		};

		if let Message::Close(Some(reason)) = message {
			assert_eq!(u16::from(reason.code), 1008);
			assert!(reason.reason.contains("token expired"));
			return;
		}
	}
}

/// Advance established grants while retaining real-time network timeouts.
async fn advance(seconds: u64) {
	tokio::time::pause();
	tokio::time::advance(Duration::from_secs(seconds)).await;
	tokio::time::resume();
}

async fn assert_expiry(paused: bool) {
	let fixture = Fixture::start().await;

	if paused {
		fixture
			.store
			.set_terms(
				&fixture.project,
				&Terms {
					included_documents: Some(0),
					..Terms::of(Plan::Hobby)
				},
			)
			.await
			.expect("cap documents");
	}

	let mut sockets = Vec::new();

	for hocuspocus in [false, true] {
		for scope in [Scope::Read, Scope::Write] {
			let token = fixture.token("doc", scope);
			let socket = if hocuspocus {
				let mut socket = connect(&fixture.hocuspocus_url()).await;
				assert_eq!(authenticate(&mut socket, "doc", &token).await.0, 2);
				socket
			} else {
				let mut socket =
					connect(&format!("{}/doc?token={token}", fixture.sync_url())).await;
				binary(&mut socket).await;
				socket
			};
			sockets.push((socket, scope));
		}
	}

	advance(3606).await;

	for (mut socket, scope) in sockets {
		expired(&mut socket, scope == Scope::Write).await;
	}
	assert_eq!(
		fixture.server.app().rooms.stats().peers,
		0,
		"expiry leaves the room"
	);

	// A newly verified token can sync after the original connection expired.
	let fresh = fixture.token("doc", Scope::Write);
	let mut renewed = connect(&fixture.hocuspocus_url()).await;
	assert_eq!(authenticate(&mut renewed, "doc", &fresh).await.0, 2);
	let mut renewed = connect(&format!("{}/doc?token={fresh}", fixture.sync_url())).await;
	binary(&mut renewed).await;
}

#[tokio::test]
async fn read_and_write_connections_expire_on_both_protocols() {
	assert_expiry(false).await;
}

#[tokio::test]
async fn paused_connections_expire_on_both_protocols() {
	assert_expiry(true).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn hocuspocus_revalidates_cached_open_and_paused_grants() {
	let fixture = Fixture::start().await;
	let mut socket = connect(&fixture.hocuspocus_url()).await;
	let valid = fixture.token("doc", Scope::Write);
	assert_eq!(authenticate(&mut socket, "doc", &valid).await.0, 2);
	let other = Fixture::start().await;

	for token in [
		"garbage".to_owned(),
		fixture.token_until("doc", Scope::Write, 1),
		fixture.token("other", Scope::Write),
		other.token("doc", Scope::Read),
	] {
		assert_eq!(authenticate(&mut socket, "doc", &token).await.0, 1);
	}

	fixture
		.store
		.set_terms(
			&fixture.project,
			&Terms {
				included_documents: Some(0),
				..Terms::of(Plan::Hobby)
			},
		)
		.await
		.expect("cap documents");
	let paused = fixture.token("paused", Scope::Write);
	assert_eq!(authenticate(&mut socket, "paused", &paused).await.0, 2);
	assert_eq!(authenticate(&mut socket, "paused", "garbage").await.0, 1);
}

#[tokio::test]
async fn hocuspocus_refresh_replaces_scope_and_expiration() {
	let fixture = Fixture::start().await;
	let mut socket = connect(&fixture.hocuspocus_url()).await;
	let old = fixture.token("doc", Scope::Write);
	assert_eq!(
		authenticate(&mut socket, "doc", &old).await,
		(2, "read-write".to_owned())
	);
	let fresh = fixture.token_until("doc", Scope::Read, auth::unix_now() + 7200);
	assert_eq!(
		authenticate(&mut socket, "doc", &fresh).await,
		(2, "readonly".to_owned())
	);

	send(&mut socket, frame("doc", &protocol::update(paragraph()))).await;

	loop {
		let bytes = binary(&mut socket).await;
		let mut cursor = Cursor::new(&bytes);
		cursor.read_string().expect("address");
		let kind: u8 = cursor.read_var().expect("kind");

		if kind == 8 {
			assert_eq!(
				cursor.read_var::<u8>().expect("saved"),
				0,
				"refreshed scope prevents writes"
			);
			break;
		}
	}

	advance(3606).await;
	send(
		&mut socket,
		frame("doc", &protocol::step1(&StateVector::default())),
	)
	.await;
	let bytes = binary(&mut socket).await;
	let mut cursor = Cursor::new(&bytes);
	assert_eq!(cursor.read_string().expect("address"), "doc");
	assert_eq!(
		cursor.read_var::<u8>().expect("kind"),
		0,
		"fresh token still permits reads after the old deadline"
	);
}

#[tokio::test]
async fn hocuspocus_closes_at_the_earliest_document_expiry() {
	let fixture = Fixture::start().await;
	let mut socket = connect(&fixture.hocuspocus_url()).await;
	assert_eq!(
		authenticate(&mut socket, "long", &fixture.token("long", Scope::Write))
			.await
			.0,
		2
	);
	let short = fixture.token_until("short", Scope::Read, auth::unix_now() + 60);
	assert_eq!(authenticate(&mut socket, "short", &short).await.0, 2);
	advance(66).await;
	expired(&mut socket, true).await;
}

#[tokio::test]
async fn hocuspocus_refresh_replaces_a_paused_grant() {
	let fixture = Fixture::start().await;
	fixture
		.store
		.set_terms(
			&fixture.project,
			&Terms {
				included_documents: Some(0),
				..Terms::of(Plan::Hobby)
			},
		)
		.await
		.expect("cap documents");
	let mut socket = connect(&fixture.hocuspocus_url()).await;
	assert_eq!(
		authenticate(&mut socket, "doc", &fixture.token("doc", Scope::Write))
			.await
			.0,
		2
	);
	let short = fixture.token_until("doc", Scope::Read, auth::unix_now() + 60);
	assert_eq!(
		authenticate(&mut socket, "doc", &short).await,
		(2, "readonly".to_owned())
	);
	advance(66).await;
	expired(&mut socket, false).await;
}

#[tokio::test]
async fn api_key_connections_have_no_token_expiry() {
	let fixture = Fixture::start().await;
	let mut websocket = connect(&format!(
		"{}/doc?token={}",
		fixture.sync_url(),
		fixture.api_key
	))
	.await;
	binary(&mut websocket).await;
	let mut hocuspocus = connect(&fixture.hocuspocus_url()).await;
	assert_eq!(
		authenticate(&mut hocuspocus, "doc", &fixture.api_key)
			.await
			.0,
		2
	);
	advance(3606).await;
	let request = protocol::step1(&StateVector::default());
	send(&mut websocket, request.clone()).await;
	send(&mut hocuspocus, frame("doc", &request)).await;
	assert_eq!(binary(&mut websocket).await[0], 0);
	let bytes = binary(&mut hocuspocus).await;
	let mut cursor = Cursor::new(&bytes);
	assert_eq!(cursor.read_string().expect("address"), "doc");
	assert_eq!(cursor.read_var::<u8>().expect("kind"), 0);
}

fn paragraph() -> Vec<u8> {
	use yrs::Array;
	use yrs::Map;
	use yrs::MapPrelim;
	use yrs::Transact;

	let doc = yrs::Doc::with_client_id(99);
	let blocks = doc.get_or_insert_array("blocks");
	let mut txn = doc.transact_mut();
	let block = blocks.push_back(&mut txn, MapPrelim::default());
	block.insert(&mut txn, "type", "paragraph");
	txn.encode_update_v1()
}
