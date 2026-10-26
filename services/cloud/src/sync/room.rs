//! One loaded document and the peers connected to it.
//!
//! A room holds the document's yrs state in memory, applies updates from
//! its peers, and fans the changes out to the others. Updates are applied
//! through [`Reorder`], so one that overtakes an earlier update from the
//! same client waits for it instead of tripping the yrs 0.28 divergence bug.
//! What actually changed (the transaction's delta) is what gets broadcast
//! and persisted, so duplicates cost nothing.
//!
//! Persistence is write-behind: changes queue in memory and
//! [`Room::persist`] appends them to the store as one merged update on the
//! next flush tick. A crash loses at most one tick of changes on the server;
//! clients still hold them and send them again when they reconnect, because
//! y-sync starts every connection by exchanging what each side is missing.

use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::MutexGuard;
use std::sync::atomic::AtomicU64;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;
use std::time::Instant;

use canvist_collab::Reorder;
use sha2::Digest;
use sha2::Sha256;
use tokio::sync::OnceCell;
use tokio::sync::broadcast;
use yrs::Doc;
use yrs::ReadTxn;
use yrs::StateVector;
use yrs::Transact;
use yrs::Update;
use yrs::sync::AwarenessUpdate;
use yrs::updates::decoder::Decode;

use super::awareness::AwarenessTable;
use super::protocol;
use super::validate;
use super::validate::Rejected;
use crate::metering::Month;
use crate::names::DocumentKey;
use crate::store::Store;
use crate::store::StoreError;

/// A connection's id within a room.
pub type PeerId = u64;

/// Messages queued per peer before a slow peer is disconnected to resync.
/// The ring is allocated up front, about 64 bytes a slot, so it is a fixed
/// cost of every loaded room: 128 slots is 8 KiB.
const FANOUT_CAPACITY: usize = 128;
/// Pending updates merged into one stored row, at most.
const MERGE_CHUNK: usize = 64;
/// Editors named in one webhook, at most.
const MAX_EDITORS: usize = 32;
/// Bound readiness scans even when each retained update is tiny.
const MAX_RETAINED_UPDATES: usize = 128;

/// What a room sends its peers.
#[derive(Clone, Debug)]
pub enum Fanout {
	/// A y-protocols message for every peer except `from`, or for all of
	/// them when `from` is `None`.
	Message {
		/// The peer the change came from.
		from: Option<PeerId>,
		/// The encoded message.
		message: Arc<[u8]>,
	},
	/// The room closed: the document was deleted.
	Closed,
}

/// Changes since the last webhook.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Changes {
	/// When the first change arrived.
	pub first: Instant,
	/// When the latest change arrived.
	pub last: Instant,
	/// Who made them: token subjects, or `server`.
	pub editors: BTreeSet<Arc<str>>,
}

/// Limits a room enforces on what it accepts.
#[derive(Clone, Copy, Debug)]
pub struct Limits {
	/// The largest update, in bytes.
	pub max_update_bytes: usize,
	/// The largest document plus retained updates, in encoded bytes, approximately.
	pub max_document_bytes: usize,
}

/// A loaded document.
pub struct Room {
	key: DocumentKey,
	state: Mutex<RoomState>,
	fanout: broadcast::Sender<Fanout>,
	loaded: OnceCell<()>,
	persisting: tokio::sync::Mutex<()>,
	next_peer: AtomicU64,
	peers: AtomicUsize,
}

impl std::fmt::Debug for Room {
	fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		formatter
			.debug_struct("Room")
			.field("key", &self.key)
			.field("peers", &self.peers())
			.finish_non_exhaustive()
	}
}

struct RoomState {
	doc: Doc,
	reorder: Reorder,
	/// Input charges since the last time both Reorder and yrs drained. Keep
	/// these even after Reorder hands an update to yrs, which may retain it.
	/// Charging the whole batch conservatively also covers partial integration.
	retained: BTreeMap<[u8; 32], usize>,
	awareness: AwarenessTable,
	/// Deltas not yet in the store, oldest first.
	pending: Vec<Vec<u8>>,
	/// Update rows in the store since its snapshot.
	stored_rows: usize,
	/// The newest stored sequence number.
	last_seq: i64,
	/// Approximate encoded size: the loaded state plus every delta since.
	size: usize,
	/// When the last peer left, if none are connected.
	idle_since: Option<Instant>,
	changes: Option<Changes>,
	/// The month this document was last admitted for, so sessions after
	/// the first skip the store.
	admitted: Option<Month>,
}

/// A peer's membership of a room.
#[derive(Debug)]
pub struct Membership {
	/// The peer's id in the room.
	pub peer: PeerId,
	/// The room's messages for this peer.
	pub receiver: broadcast::Receiver<Fanout>,
}

impl Room {
	/// An empty room for `key`; call [`Room::load`] before use.
	pub fn new(key: DocumentKey) -> Self {
		let (fanout, _) = broadcast::channel(FANOUT_CAPACITY);
		Self {
			key,
			state: Mutex::new(RoomState {
				doc: Doc::new(),
				reorder: Reorder::default(),
				retained: BTreeMap::new(),
				awareness: AwarenessTable::default(),
				pending: Vec::new(),
				stored_rows: 0,
				last_seq: 0,
				size: 0,
				idle_since: Some(Instant::now()),
				changes: None,
				admitted: None,
			}),
			fanout,
			loaded: OnceCell::new(),
			persisting: tokio::sync::Mutex::new(()),
			next_peer: AtomicU64::new(1),
			peers: AtomicUsize::new(0),
		}
	}

	/// The document this room holds.
	pub fn key(&self) -> &DocumentKey {
		&self.key
	}

	fn lock(&self) -> MutexGuard<'_, RoomState> {
		// Every critical section leaves the state consistent before it can
		// panic, so a poisoned lock is still safe to use.
		self.state
			.lock()
			.unwrap_or_else(std::sync::PoisonError::into_inner)
	}

	/// Load the stored document, once. Concurrent callers wait for the same
	/// load; a failed load is retried by the next caller.
	pub async fn load(&self, store: &Store) -> Result<(), StoreError> {
		self.loaded
			.get_or_try_init(|| self.load_from(store))
			.await
			.copied()
	}

	async fn load_from(&self, store: &Store) -> Result<(), StoreError> {
		let stored = store.load(&self.key).await?;
		let mut state = self.lock();
		let parts: Vec<&Vec<u8>> = stored.snapshot.iter().chain(&stored.updates).collect();

		for bytes in &parts {
			state.integrate_stored(&self.key, bytes);
		}

		state.stored_rows = stored.updates.len();
		state.last_seq = stored.last_seq;
		state.size = parts.iter().map(|part| part.len()).sum();
		Ok(())
	}

	/// Count a new peer in. The caller must have loaded the room.
	pub fn join(&self) -> Membership {
		let receiver = self.fanout.subscribe();
		self.peers.fetch_add(1, Ordering::SeqCst);
		self.lock().idle_since = None;
		Membership {
			peer: self.next_peer.fetch_add(1, Ordering::Relaxed),
			receiver,
		}
	}

	/// Count `peer` out and withdraw its presence.
	pub fn leave(&self, peer: PeerId) {
		let mut state = self.lock();

		if let Some(removal) = state.awareness.remove_peer(peer) {
			self.send(None, protocol::awareness(removal));
		}

		if self.peers.fetch_sub(1, Ordering::SeqCst) == 1 {
			state.idle_since = Some(Instant::now());
		}
	}

	/// Whether the document was admitted for `month`.
	pub fn is_admitted(&self, month: Month) -> bool {
		self.lock().admitted == Some(month)
	}

	/// Remember that the document was admitted for `month`.
	pub fn mark_admitted(&self, month: Month) {
		self.lock().admitted = Some(month);
	}

	/// Connected peers.
	pub fn peers(&self) -> usize {
		self.peers.load(Ordering::SeqCst)
	}

	/// Sync step 1 carrying the room's state vector.
	pub fn step1(&self) -> Vec<u8> {
		protocol::step1(&self.lock().doc.transact().state_vector())
	}

	/// Sync step 2: what a peer with `state` is missing.
	pub fn step2(&self, state: &StateVector) -> Vec<u8> {
		let update = self.lock().doc.transact().encode_diff_v1(state);
		protocol::step2(update)
	}

	/// Every peer's presence, if anyone published some.
	pub fn awareness(&self) -> Option<Vec<u8>> {
		self.lock().awareness.snapshot().map(protocol::awareness)
	}

	/// The whole document as one v1 update.
	pub fn encode_state(&self) -> Vec<u8> {
		self.lock()
			.doc
			.transact()
			.encode_state_as_update_v1(&StateVector::default())
	}

	/// Apply `update` from `peer`, acting for `editor`, and broadcast what
	/// changed. Returns whether anything did.
	pub fn apply(
		&self,
		peer: Option<PeerId>,
		editor: &Arc<str>,
		update: &[u8],
		limits: Limits,
	) -> Result<bool, Rejected> {
		validate::check(update, limits.max_update_bytes)?;
		let fingerprint: [u8; 32] = Sha256::digest(update).into();
		let mut state = self.lock();

		// Replays must not allocate another decoded update or rescan waiters.
		if state.retained.contains_key(&fingerprint) {
			return Ok(false);
		}

		let retained_bytes = state.retained.values().copied().sum::<usize>();
		let admitted_bytes = state
			.size
			.checked_add(retained_bytes)
			.and_then(|bytes| bytes.checked_add(update.len()));

		if state.retained.len() >= MAX_RETAINED_UPDATES
			|| admitted_bytes.is_none_or(|bytes| bytes > limits.max_document_bytes)
		{
			return Err(Rejected::DocumentFull);
		}

		let delta = state
			.integrate(update, fingerprint)
			.map_err(|_| Rejected::Malformed)?;

		if delta.as_slice() == Update::EMPTY_V1 {
			return Ok(false);
		}

		state.size += delta.len();
		state.note_change(editor);
		state.pending.push(delta.clone());
		// Broadcasting under the lock keeps every peer's order the same as
		// the document's.
		self.send(peer, protocol::update(delta));
		Ok(true)
	}

	/// Merge `peer`'s awareness `update` and broadcast what changed to the
	/// other peers. Unlike the reference y-websocket server, the sender does
	/// not get its own state back: clients ignore it, and with carets moving
	/// on every keystroke it was a third of a typing peer's traffic.
	/// Transports send a keepalive to quiet sockets instead.
	pub fn apply_awareness(&self, peer: PeerId, update: AwarenessUpdate) {
		let mut state = self.lock();

		if let Some(accepted) = state.awareness.apply(peer, update) {
			self.send(Some(peer), protocol::awareness(accepted));
		}
	}

	fn send(&self, from: Option<PeerId>, message: Vec<u8>) {
		// No receivers is fine: nobody else is connected.
		let _ = self.fanout.send(Fanout::Message {
			from,
			message: message.into(),
		});
	}

	/// Disconnect every peer, as when the document is deleted.
	pub fn close(&self) {
		let _ = self.fanout.send(Fanout::Closed);
	}

	/// Whether the room has had no peers for at least `ttl`.
	pub fn is_idle(&self, now: Instant, ttl: std::time::Duration) -> bool {
		self.lock()
			.idle_since
			.is_some_and(|since| now.duration_since(since) >= ttl)
	}

	/// Whether changes are waiting to be persisted.
	pub fn is_dirty(&self) -> bool {
		!self.lock().pending.is_empty()
	}

	/// The changes due for a webhook: quiet for `debounce`, or pending for
	/// `max_wait` while edits keep coming.
	pub fn take_due_changes(
		&self,
		now: Instant,
		debounce: std::time::Duration,
		max_wait: std::time::Duration,
	) -> Option<Changes> {
		let mut state = self.lock();
		let due = state.changes.as_ref().is_some_and(|changes| {
			now.duration_since(changes.last) >= debounce
				|| now.duration_since(changes.first) >= max_wait
		});
		due.then(|| state.changes.take()).flatten()
	}

	/// Changes not yet reported by a webhook, regardless of debounce.
	pub fn take_changes(&self) -> Option<Changes> {
		self.lock().changes.take()
	}

	/// Write queued changes to the store as one merged update, then compact
	/// once `compact_after` rows have built up, or whenever `force_compact`.
	pub async fn persist(
		&self,
		store: &Store,
		compact_after: usize,
		force_compact: bool,
	) -> Result<(), StoreError> {
		let _persisting = self.persisting.lock().await;
		let (pending, rows) = self.take_rows();

		if !rows.is_empty() {
			let batch: Vec<(&DocumentKey, &[u8])> =
				rows.iter().map(|row| (&self.key, row.as_slice())).collect();

			match store.append_batch(&batch).await {
				Ok(appended) => {
					if let Some(rows) = appended.get(&self.key) {
						self.record_rows(*rows);
					}
				}
				Err(error) => {
					self.requeue(pending);
					return Err(error);
				}
			}
		}

		let rows = self.lock().stored_rows;

		if rows > 0 && (force_compact || rows >= compact_after) {
			self.compact(store).await?;
		}

		Ok(())
	}

	/// Claim the room for a batched flush, unless another persist (an
	/// eviction or shutdown) holds it. The guard keeps compaction exclusive.
	pub(super) fn try_claim(&self) -> Option<tokio::sync::MutexGuard<'_, ()>> {
		self.persisting.try_lock().ok()
	}

	/// Take the queued updates, and the rows to store for them: chunks of at
	/// most [`MERGE_CHUNK`] merged into one, or the updates one by one if a
	/// chunk does not merge. Merging is quadratic in yrs, so a backlog (after
	/// a store outage, say) is merged in chunks rather than all at once.
	pub(super) fn take_rows(&self) -> (Vec<Vec<u8>>, Vec<Vec<u8>>) {
		let pending = std::mem::take(&mut self.lock().pending);
		let rows = pending
			.chunks(MERGE_CHUNK)
			.flat_map(|chunk| {
				yrs::merge_updates_v1(chunk).map_or_else(
					|error| {
						tracing::error!(document = %self.key, %error, "merging pending updates failed; storing them one by one");
						chunk.to_vec()
					},
					|merged| vec![merged],
				)
			})
			.collect();

		(pending, rows)
	}

	/// Note rows a batched flush stored.
	pub(super) fn record_rows(&self, rows: crate::store::AppendedRows) {
		let mut state = self.lock();
		state.last_seq = state.last_seq.max(rows.last_seq);
		state.stored_rows += rows.rows;
	}

	/// Update rows in the store since the last snapshot.
	pub(super) fn stored_rows(&self) -> usize {
		self.lock().stored_rows
	}

	/// Put updates that failed to store back at the front of the queue.
	pub(super) fn requeue(&self, mut updates: Vec<Vec<u8>>) {
		let mut state = self.lock();
		updates.append(&mut state.pending);
		state.pending = updates;
	}

	/// Fold the stored updates into a snapshot of the current state. The
	/// caller holds the persist claim.
	pub(super) async fn compact(&self, store: &Store) -> Result<(), StoreError> {
		let (snapshot, through) = {
			let state = self.lock();
			let snapshot = state
				.doc
				.transact()
				.encode_state_as_update_v1(&StateVector::default());
			(snapshot, state.last_seq)
		};
		store.compact(&self.key, &snapshot, through).await?;
		let mut state = self.lock();
		state.stored_rows = 0;
		state.size = snapshot.len() + state.pending.iter().map(Vec::len).sum::<usize>();
		Ok(())
	}
}

impl RoomState {
	/// Apply `update` through the reorder buffer and return the delta that
	/// actually integrated.
	fn integrate(
		&mut self,
		bytes: &[u8],
		fingerprint: [u8; 32],
	) -> Result<Vec<u8>, yrs::encoding::read::Error> {
		let update = Update::decode_v1(bytes)?;
		self.retained.insert(fingerprint, bytes.len());
		self.reorder.hold(update);
		let mut txn = self.doc.transact_mut();

		// Re-read the state after each update, so one that yrs keeps pending
		// never releases its client's later updates.
		while let Some(ready) = self.reorder.take_next(&txn.state_vector()) {
			// An update that cannot integrate stays pending inside yrs.
			if let Err(error) = txn.apply_update(ready) {
				tracing::warn!(%error, "update did not integrate");
			}
		}

		if !self.reorder.is_waiting()
			&& txn.store().pending_update().is_none()
			&& txn.store().pending_ds().is_none()
		{
			self.retained.clear();
		}

		Ok(txn.encode_update_v1())
	}

	/// Apply a stored update while loading, skipping one that does not
	/// decode rather than refusing the whole document.
	fn integrate_stored(&mut self, key: &DocumentKey, bytes: &[u8]) {
		if let Err(error) = self.integrate(bytes, Sha256::digest(bytes).into()) {
			tracing::error!(document = %key, %error, "skipping a stored update that does not decode");
		}
	}

	fn note_change(&mut self, editor: &Arc<str>) {
		let now = Instant::now();
		let changes = self.changes.get_or_insert_with(|| {
			Changes {
				first: now,
				last: now,
				editors: BTreeSet::new(),
			}
		});
		changes.last = now;

		if changes.editors.len() < MAX_EDITORS {
			changes.editors.insert(editor.clone());
		}
	}
}

#[cfg(test)]
mod tests {
	use canvist_collab::keys;
	use yrs::Array;
	use yrs::MapPrelim;

	use super::*;
	use crate::names::DocumentName;
	use crate::names::Environment;
	use crate::names::ProjectId;

	const LIMITS: Limits = Limits {
		max_update_bytes: 1 << 20,
		max_document_bytes: 16 << 20,
	};

	fn room() -> Room {
		Room::new(DocumentKey {
			project: ProjectId::new("prj_test"),
			environment: Environment::parse("test").expect("environment"),
			name: DocumentName::parse("test").expect("name"),
		})
	}

	/// A valid block insertion; later calls depend on earlier ones.
	fn append(doc: &Doc) -> Vec<u8> {
		let blocks = doc.get_or_insert_array(keys::ROOT);
		let mut txn = doc.transact_mut();
		blocks.push_back(&mut txn, MapPrelim::default());
		txn.encode_update_v1()
	}

	fn apply(room: &Room, update: &[u8], limits: Limits) -> Result<bool, Rejected> {
		room.apply(None, &Arc::from("writer"), update, limits)
	}

	#[test]
	fn held_updates_count_toward_the_document_limit() {
		let source = Doc::with_client_id(1);
		let _first = append(&source);
		let second = append(&source);
		let third = append(&source);
		let room = room();
		let limits = Limits {
			max_document_bytes: second.len() + third.len() - 1,
			..LIMITS
		};

		assert_eq!(apply(&room, &second, limits), Ok(false));
		assert_eq!(apply(&room, &third, limits), Err(Rejected::DocumentFull));
		assert!(!room.is_dirty());
		assert_eq!(room.encode_state(), Update::EMPTY_V1);
	}

	#[test]
	fn repeated_held_updates_leave_room_for_their_prerequisite() {
		let source = Doc::with_client_id(1);
		let first = append(&source);
		let second = append(&source);
		let room = room();
		let limits = Limits {
			max_document_bytes: first.len() + second.len(),
			..LIMITS
		};

		for _ in 0..512 {
			assert_eq!(apply(&room, &second, limits), Ok(false));
		}

		assert_eq!(room.lock().retained.len(), 1);
		assert_eq!(apply(&room, &first, limits), Ok(true));
		assert_eq!(
			room.encode_state(),
			source
				.transact()
				.encode_state_as_update_v1(&StateVector::default())
		);
		assert!(!room.lock().reorder.is_waiting());
		assert!(room.lock().retained.is_empty());

		let third = append(&source);
		let limits = Limits {
			max_document_bytes: room.lock().size + third.len(),
			..LIMITS
		};
		assert_eq!(apply(&room, &third, limits), Ok(true));
	}

	#[test]
	fn updates_pending_inside_yrs_count_toward_the_document_limit() {
		let alice = Doc::with_client_id(1);
		let first = append(&alice);
		let bob = Doc::with_client_id(2);
		bob.transact_mut()
			.apply_update(Update::decode_v1(&first).expect("decodes"))
			.expect("applies");
		let second = append(&bob);
		let third = append(&bob);
		let room = room();
		let limits = Limits {
			max_document_bytes: second.len() + third.len() - 1,
			..LIMITS
		};

		assert_eq!(apply(&room, &second, limits), Ok(false));
		assert!(!room.lock().reorder.is_waiting());
		assert!(
			room.lock()
				.doc
				.transact()
				.store()
				.pending_update()
				.is_some()
		);
		assert_eq!(apply(&room, &third, limits), Err(Rejected::DocumentFull));

		assert_eq!(apply(&room, &first, LIMITS), Ok(true));
		assert!(room.lock().retained.is_empty());
		assert!(
			room.lock()
				.doc
				.transact()
				.store()
				.pending_update()
				.is_none()
		);
		assert_eq!(apply(&room, &third, LIMITS), Ok(true));
		assert_eq!(
			room.encode_state(),
			bob.transact()
				.encode_state_as_update_v1(&StateVector::default())
		);
	}

	#[test]
	fn tiny_updates_hit_the_room_count_limit_even_across_peers() {
		let source = Doc::with_client_id(1);
		let _first = append(&source);
		let room = room();
		let editor = Arc::from("writer");
		let mut replay = Vec::new();

		for index in 0..MAX_RETAINED_UPDATES + 10 {
			let update = append(&source);
			let expected = if index < MAX_RETAINED_UPDATES {
				Ok(false)
			} else {
				Err(Rejected::DocumentFull)
			};
			assert_eq!(
				room.apply(Some(index as u64), &editor, &update, LIMITS),
				expected
			);

			if index == 0 {
				replay = update;
			}
		}

		assert_eq!(apply(&room, &replay, LIMITS), Ok(false));
		assert_eq!(room.lock().retained.len(), MAX_RETAINED_UPDATES);
		assert!(!room.is_dirty());
	}

	#[test]
	fn delete_only_updates_wait_with_a_charge_and_release_it() {
		let source = Doc::with_client_id(1);
		let first = append(&source);
		let blocks = source.get_or_insert_array(keys::ROOT);
		let deletion = {
			let mut txn = source.transact_mut();
			blocks.remove_range(&mut txn, 0, 1);
			txn.encode_update_v1()
		};
		let room = room();

		for _ in 0..512 {
			assert_eq!(apply(&room, &deletion, LIMITS), Ok(false));
		}

		assert_eq!(room.lock().retained.len(), 1);
		assert_eq!(apply(&room, &first, LIMITS), Ok(true));
		assert!(room.lock().retained.is_empty());
		assert!(!room.lock().reorder.is_waiting());
		assert_eq!(
			room.encode_state(),
			source
				.transact()
				.encode_state_as_update_v1(&StateVector::default())
		);
	}

	#[test]
	fn a_partially_integrated_update_keeps_its_charge() {
		let alice = Doc::with_client_id(1);
		let first = append(&alice);
		let bob = Doc::with_client_id(2);
		bob.transact_mut()
			.apply_update(Update::decode_v1(&first).expect("decodes"))
			.expect("applies");
		let orphan = append(&bob);
		let carol = Doc::with_client_id(3);
		let ready = append(&carol);
		let mixed = yrs::merge_updates_v1([orphan, ready]).expect("merges");
		let room = room();

		assert_eq!(apply(&room, &mixed, LIMITS), Ok(true));
		assert_eq!(
			room.lock().retained.values().copied().sum::<usize>(),
			mixed.len()
		);
		assert!(
			room.lock()
				.doc
				.transact()
				.store()
				.pending_update()
				.is_some()
		);
		assert_eq!(apply(&room, &mixed, LIMITS), Ok(false));
		assert_eq!(apply(&room, &first, LIMITS), Ok(true));
		assert!(room.lock().retained.is_empty());
	}
}
