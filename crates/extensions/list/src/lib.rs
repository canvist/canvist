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
