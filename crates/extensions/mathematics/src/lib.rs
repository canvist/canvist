#![doc(
	html_logo_url = "https://raw.githubusercontent.com/canvist/canvist/main/assets/brand/web/icon.svg",
	html_favicon_url = "https://raw.githubusercontent.com/canvist/canvist/main/assets/brand/web/favicon.svg"
)]
//! # `canvist_extension_mathematics`
//!
//! TipTap's Mathematics for the canvist editor, as in
//! `@tiptap/extension-mathematics`: LaTeX formulas inline and as blocks.
//! [`Mathematics`] adds the `inlineMath` embed and the `blockMath` block,
//! each with the formula in `latex`, and the `insertInlineMath` and
//! `insertBlockMath` commands.
//!
//! Typing `$$x^2$$` inserts an inline formula, and `$$$x^2$$$` alone on a
//! line a block one. The host typesets them, with KaTeX or MathJax, in the
//! slot the canvas reserves. Its handle is a [`MathematicsHandle`].
//!
//! ```
//! use std::time::Duration;
//!
//! use canvist_core::Editor;
//! use canvist_core::Env;
//! use canvist_extension_document::Document;
//! use canvist_extension_mathematics::Mathematics;
//! use canvist_extension_paragraph::Paragraph;
//!
//! let mut builder = Editor::builder().extension(Document).extension(Paragraph);
//! let math = builder.add(Mathematics);
//! let mut editor = builder.build()?;
//! let mut env = Env::headless(Duration::ZERO);
//!
//! for character in "Area: $$\\pi r^2$$".chars() {
//! 	editor.insert_text(&character.to_string(), &mut env)?;
//! }
//! assert_eq!(editor.state().doc().plain_text(), "Area: \u{fffc}", "the formula is an embed");
//!
//! editor.run(&math.insert_inline("e^{i\\pi}"), &mut env)?;
//! assert_eq!(editor.state().doc().plain_text(), "Area: \u{fffc}\u{fffc}");
//! # Ok::<(), canvist_core::EditorError>(())
//! ```

use canvist_core::Command;
use canvist_core::CommandResult;
use canvist_core::CommandSpec;
use canvist_core::EditKind;
use canvist_core::EditorError;
use canvist_core::EditorState;
use canvist_core::Extension;
use canvist_core::Field;
use canvist_core::InputRule;
use canvist_core::NodeKind;
use canvist_core::NodeMeta;
use canvist_core::Registration;
use canvist_core::RuleMatch;
use canvist_core::TypedExtension;
use canvist_core::ValueType;
use canvist_core::block_type;
use canvist_core::command;
use canvist_core::commands;
use canvist_core::embed_type;
use canvist_model::Attrs;
use canvist_model::BlockSpec;
use canvist_model::BlockTypeId;
use canvist_model::Embed;
use canvist_model::EmbedSpec;
use canvist_model::EmbedTypeId;
use canvist_model::InlineText;
use canvist_model::Point;
use canvist_model::Schema;
use canvist_model::SchemaBuilder;
use canvist_model::Selection;
use canvist_model::Step;

const INLINE: &str = "inlineMath";
const BLOCK: &str = "blockMath";

/// TipTap's Mathematics: an `inlineMath` embed and a `blockMath` block,
/// each holding LaTeX in `latex`.
///
/// Typing `$$x^2$$` inserts an inline formula, and `$$$x^2$$$` alone on a
/// line a block one. The canvas draws inline LaTeX as its source in a tinted
/// box and reserves a slot for block formulas, so the host can typeset them
/// with KaTeX or MathJax. HTML uses TipTap's `data-type` spans and divs;
/// Markdown writes `$…$` and `$$` blocks.
#[derive(Clone, Copy, Debug, Default)]
pub struct Mathematics;

/// Typed access to [`Mathematics`].
#[derive(Clone, Copy, Debug)]
pub struct MathematicsHandle {
	_registration: Registration,
}

#[allow(
	clippy::unused_self,
	reason = "handle methods require proof of registration"
)]
impl MathematicsHandle {
	/// Insert an inline formula at the selection.
	#[must_use]
	pub fn insert_inline(self, latex: &str) -> Command {
		insert_inline(latex)
	}

	/// Insert a block formula at the selection.
	#[must_use]
	pub fn insert_block(self, latex: &str) -> Command {
		insert_block(latex)
	}
}

fn latex(latex: &str) -> Attrs {
	Attrs::new().with("latex", latex)
}

fn insert_inline(source: &str) -> Command {
	let attrs = latex(source);

	command(move |state| {
		let kind = embed_type(state.schema(), INLINE, INLINE)?;
		commands::insert_embed(
			state,
			Embed {
				kind,
				attrs: attrs.clone(),
			},
		)
	})
}

fn insert_block(source: &str) -> Command {
	let attrs = latex(source);

	command(move |state| {
		let kind = block_type(state.schema(), BLOCK, BLOCK)?;
		commands::insert_leaf(state, kind, attrs.clone())
	})
}

/// Replace typed `$$latex$$` with an inline formula.
fn typed_inline(state: &mut EditorState, found: &RuleMatch, kind: EmbedTypeId) -> CommandResult {
	let (Some(Some(source)), Some(text)) = (found.groups.get(1).cloned(), found.group(1)) else {
		return Ok(None);
	};
	let start = source.start - 2;
	let marks = commands::marks_at(state.doc(), Point::new(found.block, start));
	let content = InlineText::embed(
		Embed {
			kind,
			attrs: latex(text),
		},
		marks,
	);
	let end = Point::new(found.block, start + content.len());

	state.change(EditKind::InputRule, |tr| {
		tr.delete_text(found.block, start..found.caret)?;
		tr.step(Step::InsertInline {
			block: found.block,
			offset: start,
			content,
		})?;
		tr.set_selection(Selection::caret(end));
		Ok(true)
	})
}

/// Replace a line typed as `$$$latex$$$` with a block formula.
fn typed_block(state: &mut EditorState, found: &RuleMatch, kind: BlockTypeId) -> CommandResult {
	let Some(text) = found.group(1) else {
		return Ok(None);
	};
	let attrs = latex(text);

	state.atomic(|state| {
		let cleared = state.change(EditKind::InputRule, |tr| {
			tr.delete_text(found.block, found.start..found.caret)?;
			tr.set_selection(Selection::caret(Point::new(found.block, found.start)));
			Ok(true)
		})?;
		let inserted = commands::insert_leaf(state, kind, attrs)?;

		Ok(cleared
			.zip(inserted)
			.map(|(cleared, inserted)| cleared.then(inserted)))
	})
}

fn latex_field(required: bool) -> Field {
	let doc = "The formula, in LaTeX.";

	if required {
		Field::required("latex", ValueType::String, doc)
	} else {
		Field::optional("latex", ValueType::String, "", doc)
	}
}

impl Extension for Mathematics {
	fn name(&self) -> &'static str {
		"mathematics"
	}

	fn schema(&self, schema: SchemaBuilder) -> SchemaBuilder {
		schema
			.embed(EmbedSpec::new(INLINE).attrs(latex("")))
			.block(BlockSpec::leaf(BLOCK).attrs(latex("")))
	}

	fn nodes(&self) -> Vec<NodeMeta> {
		vec![
			NodeMeta::new(NodeKind::Embed, INLINE).attr(latex_field(false)),
			NodeMeta::new(NodeKind::Block, BLOCK).attr(latex_field(false)),
		]
	}

	fn commands(&self, _schema: &Schema) -> Result<Vec<CommandSpec>, EditorError> {
		let inline = CommandSpec::new(
			"insertInlineMath",
			"Insert an inline formula at the selection.",
			|args| Ok(insert_inline(args.string("latex")?)),
		);
		let block = CommandSpec::new(
			"insertBlockMath",
			"Insert a block formula at the selection.",
			|args| Ok(insert_block(args.string("latex")?)),
		);

		Ok(vec![
			inline.arg(latex_field(true)),
			block.arg(latex_field(true)),
		])
	}

	fn input_rules(&self, schema: &Schema) -> Result<Vec<InputRule>, EditorError> {
		let inline = embed_type(schema, INLINE, INLINE)?;
		let block = block_type(schema, BLOCK, BLOCK)?;
		let block_rule = InputRule::new(r"^\$\$\$([^$]+)\$\$\$$", move |state, found| {
			typed_block(state, found, block)
		})?;
		let inline_rule = InputRule::new(r"(?:^|[^$])\$\$([^$\n]+)\$\$$", move |state, found| {
			typed_inline(state, found, inline)
		})?;

		Ok(vec![block_rule, inline_rule])
	}
}

impl TypedExtension for Mathematics {
	type Handle = MathematicsHandle;

	fn handle(&self, registration: Registration) -> MathematicsHandle {
		MathematicsHandle {
			_registration: registration,
		}
	}
}

#[cfg(test)]
mod tests {
	use std::time::Duration;

	use canvist_core::Editor;
	use canvist_core::Env;
	use canvist_extension_document::Document;
	use canvist_extension_paragraph::Paragraph;
	use canvist_model::Value;

	use super::*;

	/// The editor after typing `text`, one character at a time.
	fn typed(text: &str) -> Result<Editor, EditorError> {
		let mut editor = Editor::builder()
			.extension(Document)
			.extension(Paragraph)
			.extension(Mathematics)
			.build()?;
		let mut env = Env::headless(Duration::ZERO);

		for character in text.chars() {
			editor.insert_text(&character.to_string(), &mut env)?;
		}

		Ok(editor)
	}

	/// Every formula in the document, inline or block, in order.
	fn formulas(editor: &Editor) -> Vec<Value> {
		let doc = editor.state().doc();
		let latex = |attrs: &Attrs| attrs.get("latex").cloned().unwrap_or_default();
		doc.iter()
			.flat_map(|block| {
				let inline = block.inline().into_iter().flat_map(|text| {
					text.runs()
						.filter_map(|run| run.embed.map(|embed| latex(&embed.attrs)))
						.collect::<Vec<_>>()
				});
				let leaf = (doc.schema().block(block.kind()).name.as_ref() == BLOCK)
					.then(|| latex(block.attrs()));
				inline.chain(leaf).collect::<Vec<_>>()
			})
			.collect()
	}

	#[test]
	fn typed_double_dollars_make_inline_formulas() -> Result<(), EditorError> {
		let editor = typed("a $$x^2$$ and $$y$$")?;

		assert_eq!(formulas(&editor), [Value::from("x^2"), Value::from("y")]);
		assert_eq!(editor.state().doc().plain_text(), "a \u{fffc} and \u{fffc}");
		Ok(())
	}

	#[test]
	fn three_dollars_alone_on_a_line_make_a_block_formula() -> Result<(), EditorError> {
		let editor = typed("$$$\\sum x$$$")?;
		let doc = editor.state().doc();
		let first = doc.children(None)[0];
		let kind = doc
			.block(first)
			.map(|block| doc.schema().block(block.kind()).name.to_string());

		assert_eq!(kind.as_deref(), Some(BLOCK));
		assert_eq!(formulas(&editor), [Value::from("\\sum x")]);
		Ok(())
	}

	#[test]
	fn empty_or_unclosed_formulas_stay_text() -> Result<(), EditorError> {
		for text in ["$$$$", "$$x$", "price $5 or $$"] {
			let editor = typed(text)?;
			assert!(formulas(&editor).is_empty(), "{text:?}");
			assert_eq!(editor.state().doc().plain_text(), text);
		}

		Ok(())
	}
}
