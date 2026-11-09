#![doc(
	html_logo_url = "https://raw.githubusercontent.com/canvist/canvist/main/assets/brand/web/icon.svg",
	html_favicon_url = "https://raw.githubusercontent.com/canvist/canvist/main/assets/brand/web/favicon.svg"
)]
//! # `canvist_extension_typography`
//!
//! Smart punctuation as you type for the canvist editor, as in TipTap's
//! `@tiptap/extension-typography`. [`Typography`] adds no nodes, marks, or
//! commands, only input rules: `--` becomes `—`, `...` becomes `…`, straight
//! quotes curl, `->` becomes `→`, `(c)` becomes `©`, `1/2 ` becomes `½ `, and
//! so on. The rules are TipTap's, under TipTap's option names, plus
//! Remirror's `<->` arrow. [`Typography::without`] turns rules off and
//! [`Typography::with_output`] changes what one produces.
//!
//! A rule whose pattern has a capture group replaces only that group, so
//! `1/2 ` becomes `½ ` and keeps its space. Backspace straight after a rule
//! fires puts back what was typed. Rules never fire inside inline code.
//!
//! Input rules only run when the typed character is punctuation or white
//! space, so typing letters and digits never pays for them. TipTap's rules
//! that end in a digit (`2x3`, `^2`, `^3`) therefore fire on the space or
//! punctuation typed after them instead.
//!
//! ```
//! use std::time::Duration;
//!
//! use canvist_core::Editor;
//! use canvist_core::Env;
//! use canvist_extension_document::Document;
//! use canvist_extension_paragraph::Paragraph;
//! use canvist_extension_typography::Typography;
//!
//! let mut builder = Editor::builder().extension(Document).extension(Paragraph);
//! builder.add(Typography::default());
//! let mut editor = builder.build()?;
//! let mut env = Env::headless(Duration::ZERO);
//!
//! for character in "wait -- \"what\"...".chars() {
//! 	editor.insert_text(&character.to_string(), &mut env)?;
//! }
//!
//! assert_eq!(editor.state().doc().plain_text(), "wait — “what”…");
//! # Ok::<(), canvist_core::EditorError>(())
//! ```
use std::sync::Arc;

use canvist_core::CommandResult;
use canvist_core::EditKind;
use canvist_core::EditorError;
use canvist_core::EditorState;
use canvist_core::Extension;
use canvist_core::InputRule;
use canvist_core::Lookback;
use canvist_core::Registration;
use canvist_core::RuleMatch;
use canvist_core::TypedExtension;
use canvist_core::commands;
use canvist_model::Point;
use canvist_model::Schema;
use canvist_model::Selection;

/// What ends a word, so a rule ending in a digit can fire.
const BOUNDARY: &str = r"[\s.,;:!?)\]]";
/// The characters [`BOUNDARY`] matches, which are the only typed characters
/// that can complete a bounded rule.
const BOUNDARY_CHARS: &str = " \t\n.,;:!?)]";
/// How much text a bounded rule needs before the caret: the longest is a
/// multiplication such as `1024 x 768`.
const BOUNDED_LOOKBACK: Lookback = Lookback::Bytes(24);

/// One rule: its TipTap option name, its pattern without the trailing `$`,
/// and what it produces.
#[derive(Clone, Copy, Debug)]
struct Rule {
	name: &'static str,
	pattern: &'static str,
	output: &'static str,
	/// Whether the pattern ends at a typed word boundary, which the rule
	/// keeps, rather than at the replaced text.
	bounded: bool,
	/// The typed character that completes a rule whose pattern ends in a
	/// group, such as an opening quote.
	trigger: Option<&'static str>,
}

const fn rule(name: &'static str, pattern: &'static str, output: &'static str) -> Rule {
	Rule {
		name,
		pattern,
		output,
		bounded: false,
		trigger: None,
	}
}

/// A rule completed by typing `trigger`, whose pattern cannot say so.
const fn on(
	trigger: &'static str,
	name: &'static str,
	pattern: &'static str,
	output: &'static str,
) -> Rule {
	Rule {
		name,
		pattern,
		output,
		bounded: false,
		trigger: Some(trigger),
	}
}

const fn bounded(name: &'static str, pattern: &'static str, output: &'static str) -> Rule {
	Rule {
		name,
		pattern,
		output,
		bounded: true,
		trigger: Some(BOUNDARY_CHARS),
	}
}

/// Every rule, in the order they are tried. Opening quotes come before
/// closing ones, and `←>` before `->`.
const RULES: [Rule; 23] = [
	rule("emDash", "--", "—"),
	rule("ellipsis", r"\.\.\.", "…"),
	on("\"", "openDoubleQuote", r#"(?:^|[\s{\[(<'"‘“])(")"#, "“"),
	rule("closeDoubleQuote", "\"", "”"),
	on("'", "openSingleQuote", r#"(?:^|[\s{\[(<'"‘“])(')"#, "‘"),
	rule("closeSingleQuote", "'", "’"),
	rule("leftArrow", "<-", "←"),
	rule("leftRightArrow", "←>", "↔"),
	rule("rightArrow", "->", "→"),
	rule("copyright", r"\(c\)", "©"),
	rule("trademark", r"\(tm\)", "™"),
	rule("servicemark", r"\(sm\)", "℠"),
	rule("registeredTrademark", r"\(r\)", "®"),
	bounded("oneHalf", r"(?:^|\s)(1/2)", "½"),
	rule("plusMinus", r"\+/-", "±"),
	rule("notEqual", "!=", "≠"),
	rule("laquo", "<<", "«"),
	rule("raquo", ">>", "»"),
	bounded("multiplication", r"\d+\s?([*x])\s?\d+", "×"),
	bounded("superscriptTwo", r"(\^2)", "²"),
	bounded("superscriptThree", r"(\^3)", "³"),
	bounded("oneQuarter", r"(?:^|\s)(1/4)", "¼"),
	bounded("threeQuarters", r"(?:^|\s)(3/4)", "¾"),
];

/// Smart punctuation as you type, as in TipTap's Typography.
///
/// ```
/// use canvist_extension_typography::Typography;
///
/// // Keep straight quotes and use guillemets for `<<` and `>>`.
/// let typography = Typography::default()
/// 	.without(&["openDoubleQuote", "closeDoubleQuote"])
/// 	.with_output("laquo", "‹");
/// assert_eq!(typography.rule_names().count(), 21);
/// ```
#[derive(Clone, Debug, Default)]
pub struct Typography {
	/// Rules turned off, by TipTap option name.
	pub disabled: Vec<&'static str>,
	/// Rules whose output is replaced, by TipTap option name.
	pub outputs: Vec<(&'static str, Arc<str>)>,
}

impl Typography {
	/// Turn off the rules named in `names`, such as `"emDash"`.
	#[must_use]
	pub fn without(mut self, names: &[&'static str]) -> Self {
		self.disabled.extend_from_slice(names);
		self
	}

	/// Make the rule named `name` produce `output` instead. A later call
	/// for the same rule replaces an earlier one.
	#[must_use]
	pub fn with_output(mut self, name: &'static str, output: &str) -> Self {
		self.outputs.push((name, Arc::from(output)));
		self
	}

	/// The names of the rules that are on, in the order they are tried.
	pub fn rule_names(&self) -> impl Iterator<Item = &'static str> + '_ {
		self.rules().map(|(rule, _)| rule.name)
	}

	/// The rules that are on, with their outputs.
	fn rules(&self) -> impl Iterator<Item = (Rule, Arc<str>)> + '_ {
		RULES
			.into_iter()
			.filter(|rule| !self.disabled.contains(&rule.name))
			.map(|rule| {
				let output = self
					.outputs
					.iter()
					.rfind(|(name, _)| *name == rule.name)
					.map_or_else(|| Arc::from(rule.output), |(_, output)| output.clone());
				(rule, output)
			})
	}
}

/// Typed access to [`Typography`]. It has no commands: its rules fire as
/// you type.
#[derive(Clone, Copy, Debug)]
pub struct TypographyHandle {
	_registration: Registration,
}

/// Whether the typed text would be inline code, where typography never
/// runs, as in TipTap: `i--` and `a->b` are code, not prose.
fn in_inline_code(state: &EditorState) -> bool {
	state
		.schema()
		.mark_type("code")
		.is_some_and(|code| commands::active_marks(state).get(code).is_some())
}

/// The input rule for `rule`, producing `output`.
fn input_rule(rule: Rule, output: Arc<str>) -> Result<InputRule, EditorError> {
	let boundary = if rule.bounded { BOUNDARY } else { "" };
	let pattern = format!("{}{boundary}$", rule.pattern);
	let mut input = InputRule::new(&pattern, move |state, found| replace(state, found, &output))?;

	if let Some(trigger) = rule.trigger {
		input = input.on(trigger);
	}

	if rule.bounded {
		input = input.lookback(BOUNDED_LOOKBACK);
	}

	Ok(input)
}

/// Replace capture group 1 of `found` (or the whole match) with `output`,
/// keeping the rest of what was typed, such as the space after `1/2`.
///
/// The match runs over the text before the caret followed by the typed
/// text, which is not in the document yet. Typing one character puts the
/// group partly before the caret; inserting a string such as `" ->"` at
/// once can put it wholly inside the typed text.
fn replace(state: &mut EditorState, found: &RuleMatch, output: &str) -> CommandResult {
	let Some(target) = found
		.groups
		.get(1)
		.or(found.groups.first())
		.cloned()
		.flatten()
	else {
		return Ok(None);
	};

	if in_inline_code(state) {
		return Ok(None);
	}

	let block = found.block;
	let caret = found.caret;
	let start = target.start.min(caret);
	let removed_end = target.end.min(caret);
	// The typed text before and after the group.
	let typed_head = &found.typed[..(target.start.saturating_sub(caret) as usize)];
	let typed_tail = &found.typed[(target.end.saturating_sub(caret) as usize)..];
	let replacement = format!("{typed_head}{output}");
	let marks = commands::marks_at(state.doc(), Point::new(block, start));

	state.change(EditKind::InputRule, |tr| {
		tr.delete_text(block, start..removed_end)?;
		let mut end = tr.insert_text(Point::new(block, start), &replacement, marks.clone())?;

		if !typed_tail.is_empty() {
			// Document text between the group and the caret stays put.
			let typed_at = Point::new(block, end.offset + caret - removed_end);
			end = tr.insert_text(typed_at, typed_tail, marks)?;
		}

		tr.set_selection(Selection::caret(end));
		Ok(true)
	})
}

impl Extension for Typography {
	fn name(&self) -> &'static str {
		"typography"
	}

	fn input_rules(&self, _schema: &Schema) -> Result<Vec<InputRule>, EditorError> {
		self.rules()
			.map(|(rule, output)| input_rule(rule, output))
			.collect()
	}
}

impl TypedExtension for Typography {
	type Handle = TypographyHandle;

	fn handle(&self, registration: Registration) -> TypographyHandle {
		TypographyHandle {
			_registration: registration,
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn every_pattern_compiles() {
		let schema = Schema::builder()
			.block(canvist_model::BlockSpec::text("paragraph"))
			.build()
			.expect("valid schema");
		let rules = Typography::default()
			.input_rules(&schema)
			.expect("patterns compile");

		assert_eq!(rules.len(), RULES.len());
	}

	#[test]
	fn the_last_output_for_a_rule_wins() {
		let typography = Typography::default()
			.with_output("emDash", "–")
			.with_output("emDash", "―");
		let (_, output) = typography
			.rules()
			.find(|(rule, _)| rule.name == "emDash")
			.expect("emDash is on");

		assert_eq!(&*output, "―");
	}

	#[test]
	fn turned_off_rules_are_skipped() {
		let typography = Typography::default().without(&["emDash", "ellipsis"]);

		assert_eq!(typography.rule_names().count(), RULES.len() - 2);
		assert!(!typography.rule_names().any(|name| name == "emDash"));
	}
}
