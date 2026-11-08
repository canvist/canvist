#![doc(
	html_logo_url = "https://raw.githubusercontent.com/canvist/canvist/main/assets/brand/web/icon.svg",
	html_favicon_url = "https://raw.githubusercontent.com/canvist/canvist/main/assets/brand/web/favicon.svg"
)]
//! # `canvist_extension_kinetic`
//!
//! Kinetic type for the canvist editor: text that moves, glyph by glyph, on
//! the canvas. [`Kinetic`] adds a `kinetic` mark whose `effect` attribute is
//! one of the [`KineticEffect`]s (`wave`, `shake`, `pulse`, or `rainbow`),
//! the `setKinetic`, `toggleKinetic`, and `unsetKinetic` commands, and the
//! `kineticEffect` query. It is canvas-native: neither TipTap nor Remirror has
//! it, and the document only stores the mark, which `canvist_layout`
//! animates.
//!
//! ```
//! use std::time::Duration;
//!
//! use canvist_core::Editor;
//! use canvist_core::Env;
//! use canvist_core::command;
//! use canvist_core::commands;
//! use canvist_extension_document::Document;
//! use canvist_extension_kinetic::Kinetic;
//! use canvist_extension_kinetic::KineticEffect;
//! use canvist_extension_paragraph::Paragraph;
//!
//! let mut builder = Editor::builder().extension(Document).extension(Paragraph);
//! let kinetic = builder.add(Kinetic);
//! let mut editor = builder.build()?;
//! let mut env = Env::headless(Duration::ZERO);
//!
//! editor.insert_text("Hooray", &mut env)?;
//! editor.run(&command(commands::select_all), &mut env)?;
//! editor.run(&kinetic.set(KineticEffect::Rainbow), &mut env)?;
//!
//! assert_eq!(kinetic.effect(editor.state()), Some(KineticEffect::Rainbow));
//! # Ok::<(), canvist_core::EditorError>(())
//! ```
use canvist_core::Args;
use canvist_core::Command;
use canvist_core::CommandSpec;
use canvist_core::EditorError;
use canvist_core::EditorState;
use canvist_core::Extension;
use canvist_core::Field;
use canvist_core::NodeKind;
use canvist_core::NodeMeta;
use canvist_core::QuerySpec;
use canvist_core::Registration;
use canvist_core::TypedExtension;
use canvist_core::ValueType;
use canvist_core::command;
use canvist_core::commands;
use canvist_core::mark_type;
use canvist_core::query;
use canvist_model::Attrs;
use canvist_model::Mark;
use canvist_model::MarkSpec;
use canvist_model::Schema;
use canvist_model::SchemaBuilder;
use canvist_model::Value;

const KINETIC: &str = "kinetic";
const EFFECT: &str = "effect";

/// How kinetic text moves.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum KineticEffect {
	/// Each glyph rises and falls a little after the one before it.
	#[default]
	Wave,
	/// Glyphs jitter, as if shivering.
	Shake,
	/// Glyphs fade in and out in a ripple.
	Pulse,
	/// Glyphs cycle through the colour wheel.
	Rainbow,
}

impl KineticEffect {
	/// Every effect.
	pub const ALL: [Self; 4] = [Self::Wave, Self::Shake, Self::Pulse, Self::Rainbow];

	/// The attribute value, such as `wave`, which `canvist_layout`'s
	/// `Motion` reads.
	#[must_use]
	pub const fn name(self) -> &'static str {
		match self {
			Self::Wave => "wave",
			Self::Shake => "shake",
			Self::Pulse => "pulse",
			Self::Rainbow => "rainbow",
		}
	}

	fn from_name(name: &str) -> Option<Self> {
		Self::ALL.into_iter().find(|effect| effect.name() == name)
	}
}

/// A canvas-native extension: the `kinetic` mark, whose `effect` makes its
/// text wave, shake, pulse, or cycle through colours.
///
/// The document stores only the mark; the motion happens when a host draws
/// a display list with a time. `canvist_layout` then places each glyph of
/// kinetic text where its effect puts it at that moment, and reports that
/// the list is animated, so the host keeps asking for frames while kinetic
/// text is on screen and stops when it scrolls away. Hosts pass no time to
/// readers who prefer reduced motion, and the text stays still. HTML writes
/// `<span data-kinetic="wave">`, which `canvist_layout`'s server renderer
/// animates with CSS; Markdown keeps only the text.
#[derive(Clone, Copy, Debug, Default)]
pub struct Kinetic;

/// Typed access to [`Kinetic`].
#[derive(Clone, Copy, Debug)]
pub struct KineticHandle {
	_registration: Registration,
}

#[allow(
	clippy::unused_self,
	reason = "handle methods require proof of registration"
)]
impl KineticHandle {
	/// Make the selection move with `effect`.
	#[must_use]
	pub fn set(self, effect: KineticEffect) -> Command {
		set(effect)
	}

	/// Make the selection still.
	#[must_use]
	pub fn unset(self) -> Command {
		unset()
	}

	/// Make the selection move with `effect`, or still when it already
	/// moves with `effect`.
	#[must_use]
	pub fn toggle(self, effect: KineticEffect) -> Command {
		toggle(effect)
	}

	/// The effect of the text at the selection, if it moves.
	#[must_use]
	pub fn effect(self, state: &EditorState) -> Option<KineticEffect> {
		effect(state)
	}
}

/// The effect every selected character moves with, if they share one.
fn effect(state: &EditorState) -> Option<KineticEffect> {
	let value = query::shared_mark_attr(state, KINETIC, EFFECT)?;
	value.as_str().and_then(KineticEffect::from_name)
}

fn mark(state: &EditorState, effect: KineticEffect) -> Result<Mark, EditorError> {
	let kind = mark_type(state.schema(), KINETIC, KINETIC)?;
	Ok(Mark::with_attrs(
		kind,
		Attrs::new().with(EFFECT, effect.name()),
	))
}

fn set(effect: KineticEffect) -> Command {
	command(move |state| {
		let mark = mark(state, effect)?;
		commands::set_mark(state, &mark)
	})
}

/// Still when the selection already moves with `effect`; otherwise it moves
/// with `effect`, so toggling another effect switches to it, as toggling
/// a highlight colour does.
fn toggle(effect: KineticEffect) -> Command {
	command(move |state| {
		let mark = mark(state, effect)?;

		if self::effect(state) == Some(effect) {
			commands::unset_mark(state, mark.kind)
		} else {
			commands::set_mark(state, &mark)
		}
	})
}

fn unset() -> Command {
	command(|state| {
		let kind = mark_type(state.schema(), KINETIC, KINETIC)?;
		commands::unset_mark(state, kind)
	})
}

fn effect_arg(args: &Args<'_>) -> Result<KineticEffect, EditorError> {
	let name = args.string(EFFECT)?;

	KineticEffect::from_name(name).ok_or_else(|| {
		EditorError::InvalidArgs {
			name: KINETIC.to_owned(),
			reason: format!("no kinetic effect `{name}`"),
		}
	})
}

fn effect_type() -> ValueType {
	ValueType::OneOf(
		KineticEffect::ALL
			.iter()
			.map(|effect| effect.name())
			.collect(),
	)
}

fn effect_field() -> Field {
	Field::optional(
		EFFECT,
		effect_type(),
		KineticEffect::Wave.name(),
		"How the text moves: wave, shake, pulse, or rainbow.",
	)
}

impl Extension for Kinetic {
	fn name(&self) -> &'static str {
		KINETIC
	}

	fn schema(&self, schema: SchemaBuilder) -> SchemaBuilder {
		schema.mark(
			MarkSpec::new(KINETIC).attrs(Attrs::new().with(EFFECT, KineticEffect::Wave.name())),
		)
	}

	fn nodes(&self) -> Vec<NodeMeta> {
		vec![NodeMeta::new(NodeKind::Mark, KINETIC).attr(effect_field())]
	}

	fn commands(&self, _schema: &Schema) -> Result<Vec<CommandSpec>, EditorError> {
		Ok(vec![
			CommandSpec::new("setKinetic", "Make the selection move.", |args| {
				Ok(set(effect_arg(args)?))
			})
			.arg(effect_field()),
			CommandSpec::new(
				"toggleKinetic",
				"Make the selection move, or still when it already does.",
				|args| Ok(toggle(effect_arg(args)?)),
			)
			.arg(effect_field()),
			CommandSpec::simple("unsetKinetic", "Make the selection still.", unset()),
		])
	}

	fn queries(&self, _schema: &Schema) -> Result<Vec<QuerySpec>, EditorError> {
		Ok(vec![QuerySpec::new(
			"kineticEffect",
			"How the text at the selection moves, or null when it is still.",
			ValueType::nullable(effect_type()),
			|state, _| Ok(effect(state).map_or(Value::Null, |effect| Value::from(effect.name()))),
		)])
	}
}

impl TypedExtension for Kinetic {
	type Handle = KineticHandle;

	fn handle(&self, registration: Registration) -> KineticHandle {
		KineticHandle {
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

	use super::*;

	fn run(editor: &mut Editor, command: &Command) -> bool {
		let mut env = Env::headless(Duration::ZERO);
		editor.run(command, &mut env).expect("ran").is_some()
	}

	/// An editor holding `text`, all of it selected.
	fn selected(text: &str) -> (Editor, KineticHandle) {
		let mut builder = Editor::builder().extension(Document).extension(Paragraph);
		let kinetic = builder.add(Kinetic);
		let mut editor = builder.build().expect("valid editor");
		let mut env = Env::headless(Duration::ZERO);

		editor.insert_text(text, &mut env).expect("typed");
		assert!(run(&mut editor, &command(commands::select_all)));

		(editor, kinetic)
	}

	#[test]
	fn names_round_trip() {
		for effect in KineticEffect::ALL {
			assert_eq!(KineticEffect::from_name(effect.name()), Some(effect));
		}

		assert_eq!(KineticEffect::from_name("spin"), None);
	}

	#[test]
	fn toggling_another_effect_switches_and_the_same_one_stills() {
		let (mut editor, kinetic) = selected("text");

		assert!(run(&mut editor, &kinetic.toggle(KineticEffect::Wave)));
		assert!(run(&mut editor, &kinetic.toggle(KineticEffect::Shake)));
		assert_eq!(kinetic.effect(editor.state()), Some(KineticEffect::Shake));

		assert!(run(&mut editor, &kinetic.toggle(KineticEffect::Shake)));
		assert_eq!(kinetic.effect(editor.state()), None);
	}

	#[test]
	fn the_query_reads_the_shared_effect() {
		let (mut editor, kinetic) = selected("text");

		assert_eq!(
			editor.query("kineticEffect", &Attrs::new()).ok(),
			Some(Value::Null)
		);

		run(&mut editor, &kinetic.set(KineticEffect::Pulse));
		assert_eq!(
			editor.query("kineticEffect", &Attrs::new()).ok(),
			Some(Value::from("pulse"))
		);

		run(&mut editor, &kinetic.unset());
		assert_eq!(kinetic.effect(editor.state()), None);
	}
}
