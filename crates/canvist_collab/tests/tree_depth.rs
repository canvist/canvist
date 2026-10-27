//! Remote block nesting must fail before changing the mirrored document.

mod support;

use canvist_collab::Binding;
use canvist_collab::CollabError;
use canvist_collab::Encoding;
use canvist_collab::keys;
use canvist_collab::yrs;
use canvist_model::BlockId;
use canvist_model::Document;
use yrs::Array;
use yrs::ArrayPrelim;
use yrs::ArrayRef;
use yrs::Map;
use yrs::MapPrelim;
use yrs::MapRef;
use yrs::ReadTxn;
use yrs::TextPrelim;
use yrs::Transact;
use yrs::TransactionMut;

/// Build nested containers without using the model's recursive tree writer.
fn nest(txn: &mut TransactionMut<'_>, mut array: ArrayRef, depth: u32, first_seq: u32) -> ArrayRef {
	for seq in first_seq..first_seq + depth {
		let map: MapRef = array.push_back(txn, MapPrelim::default());
		map.insert(txn, keys::ID, BlockId::new(1, seq).to_string());
		map.insert(txn, keys::TYPE, "blockquote");
		array = map.insert(txn, keys::CHILDREN, ArrayPrelim::default());
	}

	array
}

#[test]
fn over_depth_state_is_rejected_without_changing_the_model() {
	let source = yrs::Doc::with_client_id(1);
	let root = source.get_or_insert_array(keys::ROOT);
	let mut txn = source.transact_mut();
	nest(&mut txn, root, 65, 0);
	let state = txn.encode_update_v1();
	drop(txn);
	let schema = support::schema();
	let mut binding = Binding::with_client_id(schema.clone(), 2).expect("binding");
	let mut doc = Document::empty(schema, 2);
	let before = doc.to_json();
	let version = doc.version();
	let result = binding.apply_update(&mut doc, &state, Encoding::V1);

	assert!(matches!(result, Err(CollabError::BlockDepthLimit)));
	assert_eq!(doc.to_json(), before);
	assert_eq!(doc.version(), version);
	assert!(binding.take_updates().is_empty());
}

#[test]
fn leaf_blocks_count_toward_the_depth_limit() {
	for containers in [63, 64] {
		let source = yrs::Doc::with_client_id(1);
		let root = source.get_or_insert_array(keys::ROOT);
		let mut txn = source.transact_mut();
		let bottom = nest(&mut txn, root, containers, 0);
		let leaf: MapRef = bottom.push_back(&mut txn, MapPrelim::default());
		leaf.insert(&mut txn, keys::ID, BlockId::new(1, containers).to_string());
		leaf.insert(&mut txn, keys::TYPE, "paragraph");
		leaf.insert(&mut txn, keys::TEXT, TextPrelim::new("leaf"));
		let update = txn.encode_update_v1();
		drop(txn);
		let schema = support::schema();
		let mut binding = Binding::with_client_id(schema.clone(), 2).expect("binding");
		let mut doc = Document::empty(schema, 2);
		let result = binding.apply_update(&mut doc, &update, Encoding::V1);

		if containers == 63 {
			result.expect("leaf at level 64");
			assert_eq!(doc.text_blocks().count(), 1);
		} else {
			assert!(matches!(result, Err(CollabError::BlockDepthLimit)));
			assert!(doc.children(None).is_empty());
		}
	}
}

#[test]
fn boundary_depth_is_accepted_in_both_encodings() {
	for encoding in [Encoding::V1, Encoding::V2] {
		for depth in [63, 64] {
			let source = yrs::Doc::with_client_id(1);
			let root = source.get_or_insert_array(keys::ROOT);
			nest(&mut source.transact_mut(), root, depth, 0);
			let txn = source.transact();
			let state = match encoding {
				Encoding::V1 => txn.encode_state_as_update_v1(&yrs::StateVector::default()),
				Encoding::V2 => txn.encode_state_as_update_v2(&yrs::StateVector::default()),
			};
			let schema = support::schema();
			let mut binding = Binding::with_client_id(schema.clone(), 2).expect("binding");
			let mut doc = Document::empty(schema, 2);
			binding
				.apply_update(&mut doc, &state, encoding)
				.expect("within limit");
			let mut children = doc.children(None);

			for _ in 0..depth {
				assert_eq!(children.len(), 1);
				children = doc.children(Some(children[0]));
			}

			assert!(children.is_empty());
			assert_eq!(
				binding.document(2).expect("full read").to_json(),
				doc.to_json()
			);
		}
	}
}

#[test]
fn deeply_nested_state_is_rejected_in_both_encodings() {
	for encoding in [Encoding::V1, Encoding::V2] {
		let source = yrs::Doc::with_client_id(1);
		let root = source.get_or_insert_array(keys::ROOT);
		nest(&mut source.transact_mut(), root, 4096, 0);
		let txn = source.transact();
		let state = match encoding {
			Encoding::V1 => txn.encode_state_as_update_v1(&yrs::StateVector::default()),
			Encoding::V2 => txn.encode_state_as_update_v2(&yrs::StateVector::default()),
		};
		let schema = support::schema();
		let mut binding = Binding::with_client_id(schema.clone(), 2).expect("binding");
		let mut doc = Document::empty(schema, 2);

		assert!(matches!(
			binding.apply_update(&mut doc, &state, encoding),
			Err(CollabError::BlockDepthLimit)
		));
		assert!(doc.children(None).is_empty());
	}
}

#[test]
fn incremental_nesting_counts_existing_ancestors_and_recovers_after_correction() {
	let source = yrs::Doc::with_client_id(1);
	let root = source.get_or_insert_array(keys::ROOT);
	let mut txn = source.transact_mut();
	let bottom = nest(&mut txn, root.clone(), 63, 0);
	let initial = txn.encode_update_v1();
	drop(txn);
	let schema = support::schema();
	let mut binding = Binding::with_client_id(schema.clone(), 2).expect("binding");
	let mut doc = Document::empty(schema, 2);
	binding
		.apply_update(&mut doc, &initial, Encoding::V1)
		.expect("initial state");
	let mut txn = source.transact_mut();
	let bottom = nest(&mut txn, bottom, 1, 63);
	let accepted = txn.encode_update_v1();
	drop(txn);
	binding
		.apply_update(&mut doc, &accepted, Encoding::V1)
		.expect("64 levels");
	let before = doc.to_json();
	let version = doc.version();
	let mut txn = source.transact_mut();
	// An otherwise valid sibling must also stay out of the model on failure.
	nest(&mut txn, root, 1, 100);
	nest(&mut txn, bottom.clone(), 1, 64);
	let rejected = txn.encode_update_v1();
	drop(txn);

	assert!(matches!(
		binding.apply_update(&mut doc, &rejected, Encoding::V1),
		Err(CollabError::BlockDepthLimit)
	));
	assert_eq!(doc.to_json(), before);
	assert_eq!(doc.version(), version);
	assert!(binding.take_updates().is_empty());
	let mut txn = source.transact_mut();
	bottom.remove(&mut txn, 0);
	let correction = txn.encode_update_v1();
	drop(txn);
	binding
		.apply_update(&mut doc, &correction, Encoding::V1)
		.expect("corrected state");

	assert_eq!(
		doc.children(None).len(),
		2,
		"the queued sibling is mirrored too"
	);
	assert_eq!(
		binding.document(2).expect("full read").to_json(),
		doc.to_json()
	);
}

#[test]
fn replaced_children_are_checked_before_the_model_changes() {
	let source = yrs::Doc::with_client_id(1);
	let root = source.get_or_insert_array(keys::ROOT);
	let mut txn = source.transact_mut();
	nest(&mut txn, root.clone(), 1, 0);
	let initial = txn.encode_update_v1();
	drop(txn);
	let schema = support::schema();
	let mut binding = Binding::with_client_id(schema.clone(), 2).expect("binding");
	let mut doc = Document::empty(schema, 2);
	binding
		.apply_update(&mut doc, &initial, Encoding::V1)
		.expect("initial state");
	let before = doc.to_json();
	let mut txn = source.transact_mut();
	let yrs::Out::YMap(map) = root.get(&txn, 0).expect("root block") else {
		panic!("block is a map");
	};
	let replacement: ArrayRef = map.insert(&mut txn, keys::CHILDREN, ArrayPrelim::default());
	nest(&mut txn, replacement, 64, 1);
	let update = txn.encode_update_v1();
	drop(txn);

	assert!(matches!(
		binding.apply_update(&mut doc, &update, Encoding::V1),
		Err(CollabError::BlockDepthLimit)
	));
	assert_eq!(doc.to_json(), before);
}

#[test]
fn full_reads_check_even_schema_hidden_subtrees() {
	let schema = support::schema();
	let mut binding = Binding::with_client_id(schema, 2).expect("binding");
	let shared = binding.yrs();
	let root = shared.get_or_insert_array(keys::ROOT);
	let mut txn = shared.transact_mut();
	nest(&mut txn, root.clone(), 65, 0);
	let yrs::Out::YMap(map) = root.get(&txn, 0).expect("root block") else {
		panic!("block is a map");
	};
	map.insert(&mut txn, keys::TYPE, "unknown");
	drop(txn);

	assert!(matches!(
		binding.document(2),
		Err(CollabError::BlockDepthLimit)
	));
}

#[test]
fn retyping_a_container_checks_hidden_subtrees_before_revealing_them() {
	let source = yrs::Doc::with_client_id(1);
	let root = source.get_or_insert_array(keys::ROOT);
	let mut txn = source.transact_mut();
	let bottom = nest(&mut txn, root.clone(), 2, 0);
	let yrs::Out::YMap(map) = root.get(&txn, 0).expect("root block") else {
		panic!("block is a map");
	};
	map.insert(&mut txn, keys::TYPE, "bullet_list");
	let initial = txn.encode_update_v1();
	drop(txn);
	let schema = support::schema();
	let mut binding = Binding::with_client_id(schema.clone(), 2).expect("binding");
	let mut doc = Document::empty(schema, 2);
	binding
		.apply_update(&mut doc, &initial, Encoding::V1)
		.expect("initial state");
	assert_eq!(binding.rejected_blocks(), 1);
	let mut txn = source.transact_mut();
	nest(&mut txn, bottom, 64, 2);
	let hidden_update = txn.encode_update_v1();
	drop(txn);
	// Changes inside hidden blocks are not mirrored until the parent allows them.
	binding
		.apply_update(&mut doc, &hidden_update, Encoding::V1)
		.expect("unmirrored children");
	let before = doc.to_json();
	let mut txn = source.transact_mut();
	map.insert(&mut txn, keys::TYPE, "blockquote");
	let reveal = txn.encode_update_v1();
	drop(txn);

	assert!(matches!(
		binding.apply_update(&mut doc, &reveal, Encoding::V1),
		Err(CollabError::BlockDepthLimit)
	));
	assert_eq!(doc.to_json(), before);
}
