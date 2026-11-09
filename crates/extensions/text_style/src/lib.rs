#![doc(
	html_logo_url = "https://raw.githubusercontent.com/canvist/canvist/main/assets/brand/web/icon.svg",
	html_favicon_url = "https://raw.githubusercontent.com/canvist/canvist/main/assets/brand/web/favicon.svg"
)]
//! # `canvist_extension_text_style`
//!
//! Inline styles for the canvist editor: the `textStyle` mark and the
//! extensions that give it attributes, as in TipTap's `TextStyleKit`.
//!
//! [`TextStyle`] registers a `textStyle` mark with no attributes of its own
//! and a `removeEmptyTextStyle` command. The others each add one attribute to
//! it, with a command to set it, one to clear it, and a query of the same
//! name as the attribute that reads the value the selection shares:
//!
//! | Extension           | Attribute         | Commands                                         |
//! | ------------------- | ----------------- | ------------------------------------------------ |
//! | [`Color`]           | `color`           | `setColor`, `unsetColor`                         |
//! | [`BackgroundColor`] | `backgroundColor` | `setBackgroundColor`, `unsetBackgroundColor`     |
//! | [`FontFamily`]      | `fontFamily`      | `setFontFamily`, `unsetFontFamily`               |
//! | [`FontSize`]        | `fontSize`        | `setFontSize`, `unsetFontSize`                   |
//! | [`LineHeight`]      | `lineHeight`      | `setLineHeight`, `unsetLineHeight`               |
//!
//! A run carries at most one `textStyle` mark, so setting its colour keeps
//! its font size, and clearing its last attribute removes the mark. Register
//! [`TextStyle`] with at least one of the others.
//!
//! ```
//! use std::time::Duration;
//!
//! use canvist_core::Editor;
//! use canvist_core::Env;
//! use canvist_extension_document::Document;
//! use canvist_extension_paragraph::Paragraph;
//! use canvist_extension_text_style::Color;
//! use canvist_extension_text_style::TextStyle;
//!
//! let mut builder = Editor::builder().extension(Document).extension(Paragraph);
//! builder.add(TextStyle);
//! let color = builder.add(Color);
//! let mut editor = builder.build()?;
//! let mut env = Env::headless(Duration::ZERO);
//!
//! editor.run(&color.set("#958df1"), &mut env)?;
//! editor.insert_text("purple", &mut env)?;
//!
//! assert_eq!(color.value(editor.state()).as_deref(), Some("#958df1"));
//! # Ok::<(), canvist_core::EditorError>(())
//! ```
mod style_attr;

use canvist_core::Command;
use canvist_core::CommandSpec;
use canvist_core::EditorError;
use canvist_core::Extension;
use canvist_core::NodeKind;
use canvist_core::NodeMeta;
use canvist_core::Registration;
use canvist_core::TypedExtension;
use canvist_core::command;
use canvist_core::commands;
use canvist_core::mark_type;
use canvist_model::Attrs;
use canvist_model::MarkSpec;
use canvist_model::Schema;
use canvist_model::SchemaBuilder;
use canvist_model::Value;
pub use style_attr::BackgroundColor;
pub use style_attr::Color;
pub use style_attr::FontFamily;
pub use style_attr::FontSize;
pub use style_attr::LineHeight;
pub use style_attr::StyleAttrHandle;

/// The mark every style attribute lives on.
const TEXT_STYLE: &str = "textStyle";

/// The `textStyle` mark: a `<span style>` that [`Color`], [`FontFamily`],
/// and the other style extensions add attributes to. Register it with at
/// least one of them.
#[derive(Clone, Copy, Debug, Default)]
pub struct TextStyle;

/// Typed access to [`TextStyle`].
#[derive(Clone, Copy, Debug)]
pub struct TextStyleHandle {
	_registration: Registration,
}

#[allow(
	clippy::unused_self,
	reason = "handle methods require proof of registration"
)]
impl TextStyleHandle {
	/// Remove `textStyle` marks whose attributes are all unset.
	#[must_use]
	pub fn remove_empty(self) -> Command {
		remove_empty()
	}
}

/// Whether every attribute is unset: absent or null.
fn is_empty(attrs: &Attrs) -> bool {
	attrs.iter().all(|(_, value)| *value == Value::Null)
}

fn remove_empty() -> Command {
	command(|state| {
		let kind = mark_type(state.schema(), TEXT_STYLE, TEXT_STYLE)?;
		let update = |current: Option<&Attrs>| current.filter(|attrs| !is_empty(attrs)).cloned();
		commands::update_mark_attrs(state, kind, &update)
	})
}

impl Extension for TextStyle {
	fn name(&self) -> &'static str {
		TEXT_STYLE
	}

	/// Above the default, so the style span wraps other marks, as in TipTap.
	fn priority(&self) -> i32 {
		101
	}

	fn schema(&self, schema: SchemaBuilder) -> SchemaBuilder {
		schema.mark(MarkSpec::new(TEXT_STYLE))
	}

	fn nodes(&self) -> Vec<NodeMeta> {
		vec![NodeMeta::new(NodeKind::Mark, TEXT_STYLE)]
	}

	fn commands(&self, _schema: &Schema) -> Result<Vec<CommandSpec>, EditorError> {
		Ok(vec![CommandSpec::simple(
			"removeEmptyTextStyle",
			"Remove text styles whose attributes are all unset.",
			remove_empty(),
		)])
	}
}

impl TypedExtension for TextStyle {
	type Handle = TextStyleHandle;

	fn handle(&self, registration: Registration) -> TextStyleHandle {
		TextStyleHandle {
			_registration: registration,
		}
	}
}

#[cfg(test)]
mod tests {
	use std::time::Duration;

	use canvist_core::Editor;
	use canvist_core::Env;
	use canvist_core::query;
	use canvist_extension_document::Document;
	use canvist_extension_paragraph::Paragraph;

	use super::*;

	struct Styled {
		editor: Editor,
		color: StyleAttrHandle,
		size: StyleAttrHandle,
	}

	/// An editor holding `text` in red at 18px, with all of it selected.
	fn styled(text: &str) -> Styled {
		let mut builder = Editor::builder().extension(Document).extension(Paragraph);
		builder.add(TextStyle);
		let color = builder.add(Color);
		let size = builder.add(FontSize);
		let mut editor = builder.build().expect("valid editor");
		let mut env = Env::headless(Duration::ZERO);

		editor.insert_text(text, &mut env).expect("typed");
		editor
			.run(&command(commands::select_all), &mut env)
			.expect("selected");
		editor.run(&color.set("red"), &mut env).expect("coloured");
		editor.run(&size.set("18px"), &mut env).expect("sized");

		Styled {
			editor,
			color,
			size,
		}
	}

	fn has_text_style(editor: &Editor) -> bool {
		query::mark_active(editor.state(), TEXT_STYLE)
	}

	#[test]
	fn setting_one_style_keeps_the_others() {
		let Styled {
			editor,
			color,
			size,
		} = styled("styled");

		assert_eq!(color.value(editor.state()).as_deref(), Some("red"));
		assert_eq!(size.value(editor.state()).as_deref(), Some("18px"));
	}

	#[test]
	fn clearing_the_last_style_removes_the_mark() {
		let Styled {
			mut editor,
			color,
			size,
		} = styled("styled");
		let mut env = Env::headless(Duration::ZERO);

		editor.run(&color.unset(), &mut env).expect("cleared");
		assert_eq!(color.value(editor.state()), None);
		assert!(has_text_style(&editor));

		editor.run(&size.unset(), &mut env).expect("cleared");
		assert!(!has_text_style(&editor));
	}

	#[test]
	fn a_mixed_selection_shares_no_value() {
		let Styled {
			mut editor, color, ..
		} = styled("styled");
		let mut env = Env::headless(Duration::ZERO);
		let block = editor
			.state()
			.doc()
			.first_text_block()
			.expect("a paragraph");
		let selection = canvist_model::Selection::text(
			canvist_model::Point::new(block, 0),
			canvist_model::Point::new(block, 3),
		);

		editor.select(selection, &mut env).expect("selected");
		editor.run(&color.set("blue"), &mut env).expect("coloured");
		editor
			.run(&command(commands::select_all), &mut env)
			.expect("selected");

		assert_eq!(color.value(editor.state()), None);
	}

	#[test]
	fn empty_means_every_attribute_is_null() {
		let empty = Attrs::new().with("color", Value::Null);

		assert!(is_empty(&empty));
		assert!(is_empty(&Attrs::new()));
		assert!(!is_empty(&empty.with("fontSize", Value::from("18px"))));
	}
}
