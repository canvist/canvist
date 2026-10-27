//! Bound remote nesting before recursive tree readers or model steps run.

use std::sync::Arc;

use canvist_model::BlockId;
use rustc_hash::FxHashSet;
use yrs::Array;
use yrs::ArrayRef;
use yrs::MapRef;
use yrs::Out;
use yrs::ReadTxn;

use crate::CollabError;
use crate::keys;
use crate::local::map_ref;
use crate::registry::Owner;
use crate::registry::Registry;
use crate::registry::branch_id;
use crate::remote::record::Record;
use crate::tree;

/// Maximum block nesting in a shared document, counting root blocks as 1.
pub const MAX_BLOCK_DEPTH: usize = 64;

/// Checks shared subtrees iteratively, including hidden blocks that could
/// become visible after a type change. A batch descends into each container
/// only once.
pub(crate) struct Check<'a, T> {
	txn: &'a T,
	seen: FxHashSet<yrs::ID>,
}

impl<'a, T: ReadTxn> Check<'a, T> {
	pub(crate) fn new(txn: &'a T) -> Self {
		Self {
			txn,
			seen: FxHashSet::default(),
		}
	}

	/// Check all blocks in an array at the given parent's depth.
	pub(crate) fn array(&mut self, array: &ArrayRef, depth: usize) -> Result<(), CollabError> {
		let mut stack = vec![(array.clone(), depth)];

		while let Some((array, depth)) = stack.pop() {
			for item in array.iter(self.txn) {
				if let Out::YMap(map) = item
					&& let Some(children) = self.block(&map, depth + 1)?
				{
					stack.push((children, depth + 1));
				}
			}
		}

		Ok(())
	}

	/// Check the subtree rooted at `map`, counting its shown ancestors.
	pub(crate) fn subtree(&mut self, map: &MapRef, depth: usize) -> Result<(), CollabError> {
		if let Some(children) = self.block(map, depth)? {
			self.array(&children, depth)?;
		}

		Ok(())
	}

	/// Check only changed subtrees, so a keystroke never scans the document.
	pub(crate) fn changes(
		&mut self,
		records: &[Record],
		registry: &Registry,
		root: &ArrayRef,
	) -> Result<(), CollabError> {
		for record in records {
			match record {
				Record::Array {
					array, inserted, ..
				} => self.inserted(*array, inserted, registry, root)?,
				Record::Map { map, keys } => self.changed(map, keys, registry)?,
				Record::Text { .. } => {}
			}
		}

		Ok(())
	}

	/// Validate inserted subtrees in arrays the mirror reads.
	fn inserted(
		&mut self,
		array: Option<yrs::ID>,
		inserted: &FxHashSet<yrs::ID>,
		registry: &Registry,
		root: &ArrayRef,
	) -> Result<(), CollabError> {
		// Initial state and whole-root replacements already contain as many
		// changed maps as root items. Walking the array avoids one store-index
		// lookup per map while checking the same complete subtrees.
		if array.is_none() && inserted.len() == root.len(self.txn) as usize {
			return self.array(root, 0);
		}

		let parent = match array.map(|id| registry.owner(&id)) {
			None => None,
			Some(Some(Owner::Children(parent))) => Some(parent),
			_ => return Ok(()),
		};
		let depth = parent_depth(registry, parent)? + 1;

		for id in inserted {
			if let Some(map) = map_ref(self.txn, id) {
				self.subtree(&map, depth)?;
			}
		}

		Ok(())
	}

	/// Mirror visibility checks may re-read a hidden map on any key change.
	fn changed(
		&mut self,
		map: &yrs::ID,
		changed: &[Arc<str>],
		registry: &Registry,
	) -> Result<(), CollabError> {
		let parent = match registry.owner(map) {
			Some(Owner::Block(block))
				if changed.iter().any(|key| {
					[keys::ID, keys::TYPE, keys::TEXT, keys::CHILDREN].contains(&&**key)
				}) =>
			{
				registry
					.block(block)
					.ok_or(CollabError::NotShared(block))?
					.parent
			}
			Some(Owner::Hidden(parent)) => parent,
			_ => return Ok(()),
		};

		if let Some(map) = map_ref(self.txn, map) {
			self.subtree(&map, parent_depth(registry, parent)? + 1)?;
		}

		Ok(())
	}

	/// Return children only for a live map not already checked in this batch.
	fn block(&mut self, map: &MapRef, depth: usize) -> Result<Option<ArrayRef>, CollabError> {
		if map.as_ref().is_deleted() {
			return Ok(None);
		}

		check_depth(depth)?;
		let Some(children) = tree::children_of(map, self.txn) else {
			return Ok(None);
		};

		// Leaf blocks cannot repeat a subtree, so only containers need a
		// visited-set entry when checking a large, flat document.
		if !branch_id(map).is_some_and(|id| self.seen.insert(id)) {
			return Ok(None);
		}

		Ok(Some(children))
	}
}

/// Count existing ancestors as well as nesting introduced by this update.
pub(crate) fn parent_depth(
	registry: &Registry,
	mut parent: Option<BlockId>,
) -> Result<usize, CollabError> {
	let mut depth = 0;

	while let Some(block) = parent {
		depth += 1;
		check_depth(depth)?;
		parent = registry
			.block(block)
			.ok_or(CollabError::NotShared(block))?
			.parent;
	}

	Ok(depth)
}

fn check_depth(depth: usize) -> Result<(), CollabError> {
	if depth > MAX_BLOCK_DEPTH {
		return Err(CollabError::BlockDepthLimit);
	}

	Ok(())
}
