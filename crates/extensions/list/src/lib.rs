#![doc(
	html_logo_url = "https://raw.githubusercontent.com/canvist/canvist/main/assets/brand/web/icon.svg",
	html_favicon_url = "https://raw.githubusercontent.com/canvist/canvist/main/assets/brand/web/favicon.svg"
)]
//! # `canvist_extension_list`
//!
//! TipTap's lists for the canvist editor, as in TipTap's
//! `@tiptap/extension-list`: bullet, ordered, and task lists, and the items
//! inside them.
//!
//! | Extension       | Adds                                          |
//! | --------------- | --------------------------------------------- |
//! | [`BulletList`]  | `bulletList`, `- `, `Mod-Shift-8`             |
//! | [`OrderedList`] | `orderedList`, `1. `, `Mod-Shift-7`           |
//! | [`ListItem`]    | `listItem`, Enter, Tab, and Shift-Tab         |
//! | [`TaskList`]    | `taskList`, `taskItem`, `[ ] `, `Mod-Shift-9` |
//!
//! Bullet and ordered lists hold [`ListItem`]s, so register it with
//! either. Every list type shares one set of item commands from
//! [`canvist_core::commands`]; the extensions here only name the types,
//! bind keys, and add input rules.
//!
//! ```
//! use std::time::Duration;
//!
//! use canvist_core::Editor;
//! use canvist_core::Env;
//! use canvist_extension_document::Document;
//! use canvist_extension_list::BulletList;
//! use canvist_extension_list::ListItem;
//! use canvist_extension_paragraph::Paragraph;
//!
//! let mut builder = Editor::builder().extension(Document).extension(Paragraph);
//! let bullets = builder.add(BulletList);
//! builder.add(ListItem);
//! let mut editor = builder.build()?;
//! let mut env = Env::headless(Duration::ZERO);
//!
//! editor.insert_text("-", &mut env)?;
//! editor.insert_text(" ", &mut env)?;
//!
//! assert!(bullets.is_active(editor.state()));
//! # Ok::<(), canvist_core::EditorError>(())
//! ```
mod bullet_list;
mod list_item;
mod lists;
mod ordered_list;
mod task_list;

pub use bullet_list::BulletList;
pub use bullet_list::BulletListHandle;
pub use list_item::ListItem;
pub use list_item::ListItemHandle;
pub use ordered_list::OrderedList;
pub use ordered_list::OrderedListHandle;
pub use task_list::TaskList;
pub use task_list::TaskListHandle;

#[cfg(test)]
mod tests {
	use std::time::Duration;

	use canvist_core::Command;
	use canvist_core::Editor;
	use canvist_core::Env;
	use canvist_extension_document::Document;
	use canvist_extension_paragraph::Paragraph;

	use super::*;

	struct Lists {
		editor: Editor,
		bullets: BulletListHandle,
		ordered: OrderedListHandle,
		tasks: TaskListHandle,
	}

	/// An editor with every list type, after typing `script` a character at
	/// a time.
	fn typed(script: &str) -> Lists {
		let mut builder = Editor::builder().extension(Document).extension(Paragraph);
		let bullets = builder.add(BulletList);
		let ordered = builder.add(OrderedList);
		let tasks = builder.add(TaskList);
		builder.add(ListItem);
		let mut editor = builder.build().expect("valid editor");
		let mut env = Env::headless(Duration::ZERO);

		for character in script.chars() {
			editor
				.insert_text(&character.to_string(), &mut env)
				.expect("typed");
		}

		Lists {
			editor,
			bullets,
			ordered,
			tasks,
		}
	}

	fn run(editor: &mut Editor, command: &Command) -> bool {
		let mut env = Env::headless(Duration::ZERO);
		editor.run(command, &mut env).expect("ran").is_some()
	}

	#[test]
	fn a_typed_number_starts_an_ordered_list_there() {
		let lists = typed("3. ");

		assert!(lists.ordered.is_active(lists.editor.state()));
		assert_eq!(lists.ordered.start(lists.editor.state()), Some(3));

		let lists = typed("1. ");
		assert_eq!(lists.ordered.start(lists.editor.state()), Some(1));
	}

	#[test]
	fn a_number_too_long_to_read_starts_at_one() {
		let lists = typed("99999999999999999999. ");

		assert_eq!(lists.ordered.start(lists.editor.state()), Some(1));
	}

	#[test]
	fn x_in_the_brackets_starts_a_checked_task() {
		let mut lists = typed("[x] ");

		assert!(lists.tasks.is_active(lists.editor.state()));
		assert_eq!(lists.tasks.is_checked(lists.editor.state()), Some(true));

		assert!(run(&mut lists.editor, &lists.tasks.toggle_checked()));
		assert_eq!(lists.tasks.is_checked(lists.editor.state()), Some(false));

		let lists = typed("[ ] ");
		assert_eq!(lists.tasks.is_checked(lists.editor.state()), Some(false));
	}

	#[test]
	fn toggling_another_list_type_converts_in_place() {
		let mut lists = typed("- item");

		assert!(run(&mut lists.editor, &lists.ordered.toggle()));
		assert!(lists.ordered.is_active(lists.editor.state()));
		assert!(!lists.bullets.is_active(lists.editor.state()));

		assert!(run(&mut lists.editor, &lists.ordered.toggle()));
		assert!(!lists.ordered.is_active(lists.editor.state()));
		assert_eq!(lists.editor.state().doc().plain_text(), "item");
	}

	#[test]
	fn tasks_outside_a_task_list_have_no_checked_state() {
		let mut lists = typed("plain");

		assert_eq!(lists.tasks.is_checked(lists.editor.state()), None);
		assert!(!run(&mut lists.editor, &lists.tasks.toggle_checked()));
		assert!(!run(&mut lists.editor, &lists.ordered.set_start(4)));
	}
}
