#![doc(
	html_logo_url = "https://raw.githubusercontent.com/canvist/canvist/main/assets/brand/web/icon.svg",
	html_favicon_url = "https://raw.githubusercontent.com/canvist/canvist/main/assets/brand/web/favicon.svg"
)]
//! # `canvist_extension_ink`
//!
//! Ink for the canvist editor: freehand drawings in the flow of the
//! document, drawn by the canvas itself. [`Ink`] adds an `ink` leaf block
//! with `height` and `strokes` attributes, the `insertInk`, `addInkStroke`,
//! `undoInkStroke`, and `clearInk` commands, and the `inks` query. It is
//! canvas-native: neither TipTap nor Remirror has it, and `canvist_layout`
//! paints the strokes rather than leaving a slot for the host.
//!
//! ```
//! use std::time::Duration;
//!
//! use canvist_core::Editor;
//! use canvist_core::Env;
//! use canvist_extension_document::Document;
//! use canvist_extension_ink::DEFAULT_HEIGHT;
//! use canvist_extension_ink::Ink;
//! use canvist_extension_ink::InkStroke;
//! use canvist_extension_paragraph::Paragraph;
//!
//! let mut builder = Editor::builder().extension(Document).extension(Paragraph);
//! let ink = builder.add(Ink);
//! let mut editor = builder.build()?;
//! let mut env = Env::headless(Duration::ZERO);
//!
//! editor.run(&ink.insert(DEFAULT_HEIGHT), &mut env)?;
//! let drawing = ink.drawings(editor.state())[0];
//! let stroke = InkStroke {
//! 	points: &[10.0, 20.0, 200.0, 120.0],
//! 	color: Some("#3b2cf5"),
//! 	width: None,
//! };
//! editor.run(&ink.add_stroke(drawing, &stroke)?, &mut env)?;
//!
//! assert!(editor.can(&ink.undo_stroke(drawing), &mut env)?);
//! # Ok::<(), canvist_core::EditorError>(())
//! ```
use canvist_core::Args;
use canvist_core::Command;
use canvist_core::CommandResult;
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
use canvist_core::block_type;
use canvist_core::command;
use canvist_core::commands;
use canvist_model::Attrs;
use canvist_model::Block;
use canvist_model::BlockId;
use canvist_model::BlockSpec;
use canvist_model::Schema;
use canvist_model::SchemaBuilder;
use canvist_model::Value;

const INK: &str = "ink";

/// The width of a drawing's coordinate space, as `canvist_layout` scales
/// it: a drawing this many units wide fills its column.
pub const INK_WIDTH: f32 = 1000.0;
/// The height a new drawing gets, in drawing units.
pub const DEFAULT_HEIGHT: i64 = 400;
/// The tallest a drawing may be, in drawing units.
const MAX_HEIGHT: i64 = 4000;
/// Stored points are rounded to one part in this many drawing units.
const POINT_PRECISION: f32 = 10.0;

/// A canvas-native extension: `ink` blocks holding freehand strokes.
///
/// A drawing lives in a coordinate space [`INK_WIDTH`] units wide and its
/// `height` units tall, so it scales with its column on a phone or a wide
/// screen. Each of its `strokes` is a `points` list of `x, y` pairs, with an
/// optional `color` and `width`. `canvist_layout` draws the strokes itself,
/// as display-list strokes every canvas renderer paints, rather than
/// leaving a slot for the host.
///
/// The host captures pointer input over the block's rectangle and adds each
/// finished stroke with `addInkStroke`, which is one undo step and one Yjs
/// update. Commands name a drawing by its place among the document's
/// drawings, as `inks` lists them. HTML writes a `<figure data-type="ink">`,
/// named as an image for assistive technology, holding an SVG of the
/// drawing, and Markdown writes the same HTML.
///
/// The strokes are one attribute of the block, so in collaboration two
/// strokes added to one drawing at the same moment converge on one of them.
#[derive(Clone, Copy, Debug, Default)]
pub struct Ink;

/// One stroke for [`InkHandle::add_stroke`].
#[derive(Clone, Debug, PartialEq)]
pub struct InkStroke<'a> {
	/// `x, y` pairs in drawing units.
	pub points: &'a [f32],
	/// A `#rrggbb` colour, or the text colour.
	pub color: Option<&'a str>,
	/// The width in drawing units, or the theme's.
	pub width: Option<f32>,
}

/// Typed access to [`Ink`].
#[derive(Clone, Copy, Debug)]
pub struct InkHandle {
	_registration: Registration,
}

#[allow(
	clippy::unused_self,
	reason = "handle methods require proof of registration"
)]
impl InkHandle {
	/// Insert an empty drawing `height` units tall at the selection, kept
	/// from 1 to 4000 units, the range `insertInk` accepts.
	#[must_use]
	pub fn insert(self, height: i64) -> Command {
		insert(height)
	}

	/// Add `stroke` to drawing `block`.
	///
	/// # Errors
	///
	/// [`EditorError::InvalidArgs`] when the points are not `x, y` pairs of
	/// finite numbers, the colour is not `#rrggbb`, or the width is not
	/// positive.
	pub fn add_stroke(
		self,
		block: BlockId,
		stroke: &InkStroke<'_>,
	) -> Result<Command, EditorError> {
		let value = stroke_value(stroke.points, stroke.color, stroke.width)?;

		Ok(command(move |state| {
			let value = value.clone();
			edit(state, block, |strokes| strokes.push(value))
		}))
	}

	/// Remove the last stroke of drawing `block`.
	#[must_use]
	pub fn undo_stroke(self, block: BlockId) -> Command {
		command(move |state| edit(state, block, |strokes| drop(strokes.pop())))
	}

	/// Remove every stroke of drawing `block`.
	#[must_use]
	pub fn clear(self, block: BlockId) -> Command {
		command(move |state| edit(state, block, Vec::clear))
	}

	/// Every drawing in the document, in order.
	#[must_use]
	pub fn drawings(self, state: &EditorState) -> Vec<BlockId> {
		drawings(state)
	}
}

fn insert(height: i64) -> Command {
	let height = height.clamp(1, MAX_HEIGHT);

	command(move |state| {
		let kind = block_type(state.schema(), INK, INK)?;
		let attrs = if height == DEFAULT_HEIGHT {
			Attrs::new()
		} else {
			Attrs::new().with("height", height)
		};

		commands::insert_leaf(state, kind, attrs)
	})
}

fn drawings(state: &EditorState) -> Vec<BlockId> {
	let Some(kind) = state.schema().block_type(INK) else {
		return Vec::new();
	};

	state
		.doc()
		.iter()
		.filter(|block| block.kind() == kind)
		.map(Block::id)
		.collect()
}

/// A drawing's strokes, as stored.
fn strokes(attrs: &Attrs) -> Vec<Value> {
	attrs
		.get("strokes")
		.and_then(Value::as_array)
		.map(<[Value]>::to_vec)
		.unwrap_or_default()
}

fn height(attrs: &Attrs) -> i64 {
	attrs
		.get("height")
		.and_then(Value::as_int)
		.unwrap_or(DEFAULT_HEIGHT)
}

/// Change drawing `block`'s strokes with `change`, as one step.
fn edit(
	state: &mut EditorState,
	block: BlockId,
	change: impl FnOnce(&mut Vec<Value>),
) -> CommandResult {
	let kind = block_type(state.schema(), INK, INK)?;
	let Some(drawing) = state
		.doc()
		.block(block)
		.filter(|drawing| drawing.kind() == kind)
	else {
		return Ok(None);
	};
	let mut attrs = drawing.attrs().clone();
	let before = strokes(&attrs);
	let mut after = before.clone();
	change(&mut after);

	if after == before {
		return Ok(None);
	}

	attrs.set("strokes", after);
	commands::set_block_attrs(state, block, &attrs)
}

/// An `addInkStroke` argument error.
fn invalid(reason: &str) -> EditorError {
	EditorError::InvalidArgs {
		name: "addInkStroke".to_owned(),
		reason: reason.to_owned(),
	}
}

/// A stroke as stored: its points rounded to a tenth of a unit, which is
/// finer than any screen draws a drawing and keeps documents small.
/// Points must come in pairs of finite numbers; they are kept to the
/// drawing's width, and a colour must be `#rrggbb`.
fn stroke_value(
	points: &[f32],
	color: Option<&str>,
	width: Option<f32>,
) -> Result<Value, EditorError> {
	let mut stroke = Attrs::new().with("points", points_value(points)?);

	if let Some(color) = color {
		stroke.set("color", color_value(color)?);
	}

	if let Some(width) = width {
		stroke.set("width", width_value(width)?);
	}

	Ok(Value::from(stroke))
}

/// The points rounded and kept to the drawing.
fn points_value(points: &[f32]) -> Result<Vec<Value>, EditorError> {
	let pairs = !points.is_empty() && points.len().is_multiple_of(2);

	if !pairs || points.iter().any(|point| !point.is_finite()) {
		return Err(invalid("`points` must be `x, y` pairs of finite numbers"));
	}

	let precision = f64::from(POINT_PRECISION);
	let round = |point: f32| Value::Float(f64::from((point * POINT_PRECISION).round()) / precision);

	Ok(points
		.chunks_exact(2)
		.flat_map(|point| [point[0].clamp(0.0, INK_WIDTH), point[1].max(0.0)])
		.map(round)
		.collect())
}

/// A `#rrggbb` colour, lowercased.
fn color_value(color: &str) -> Result<String, EditorError> {
	let hex = color.strip_prefix('#').unwrap_or_default();

	if hex.len() != "rrggbb".len() || !hex.chars().all(|digit| digit.is_ascii_hexdigit()) {
		return Err(invalid("`color` must be `#rrggbb`"));
	}

	Ok(color.to_ascii_lowercase())
}

/// A positive width, as the shortest decimal that reads back as the same
/// `f32`, so a width of 3.3 is stored as 3.3 rather than 3.299999952316284,
/// and survives HTML and JSON text exactly.
fn width_value(width: f32) -> Result<f64, EditorError> {
	if !(width.is_finite() && width > 0.0) {
		return Err(invalid("`width` must be a positive number"));
	}

	Ok(width
		.to_string()
		.parse::<f64>()
		.unwrap_or_else(|_| f64::from(width)))
}

/// Drawing `index` of the document, from an `ink` argument.
fn drawing_arg(state: &EditorState, index: usize) -> Option<BlockId> {
	drawings(state).into_iter().nth(index)
}

/// The `ink` argument of command `name`: a place among the drawings, so
/// never negative. A place past the last drawing is not an error; the
/// command just does not apply, as the document may have changed.
fn ink_index(args: &Args<'_>, name: &str) -> Result<usize, EditorError> {
	usize::try_from(args.int("ink")?).map_err(|_| {
		EditorError::InvalidArgs {
			name: name.to_owned(),
			reason: "`ink` must not be negative".to_owned(),
		}
	})
}

fn ink_field() -> Field {
	Field::required(
		"ink",
		ValueType::Int,
		"Which drawing, by its place in `inks`, from zero.",
	)
}

/// A number argument as drawing units, or `None` for anything else.
fn number(value: &Value) -> Option<f32> {
	match value {
		Value::Int(number) => Some(*number as f32),
		Value::Float(number) => Some(*number as f32),
		_ => None,
	}
}

fn floats(args: &Args<'_>, key: &str) -> Result<Vec<f32>, EditorError> {
	let values = args.get(key)?.as_array().unwrap_or_default();
	Ok(values.iter().filter_map(number).collect())
}

fn add_stroke_command(args: &Args<'_>) -> Result<Command, EditorError> {
	let index = ink_index(args, "addInkStroke")?;
	let width = number(args.get("width")?);
	let value = stroke_value(
		&floats(args, "points")?,
		args.optional_string("color")?,
		width,
	)?;

	Ok(command(move |state| {
		let Some(block) = drawing_arg(state, index) else {
			return Ok(None);
		};
		let value = value.clone();
		edit(state, block, |strokes| strokes.push(value))
	}))
}

fn by_index(
	args: &Args<'_>,
	name: &str,
	change: fn(&mut Vec<Value>),
) -> Result<Command, EditorError> {
	let index = ink_index(args, name)?;

	Ok(command(move |state| {
		let Some(block) = drawing_arg(state, index) else {
			return Ok(None);
		};
		edit(state, block, change)
	}))
}

fn inks_value(state: &EditorState) -> Value {
	let list: Vec<Value> = drawings(state)
		.into_iter()
		.filter_map(|block| state.doc().block(block))
		.map(|block| {
			Value::from(
				Attrs::new()
					.with("height", height(block.attrs()))
					.with("strokes", strokes(block.attrs()).len() as i64),
			)
		})
		.collect();

	Value::from(list)
}

fn stroke_type() -> ValueType {
	ValueType::Record(vec![
		Field::required(
			"points",
			ValueType::list(ValueType::Float),
			"`x, y` pairs in drawing units.",
		),
		Field::optional(
			"color",
			ValueType::nullable(ValueType::String),
			Value::Null,
			"A `#rrggbb` colour, or null for the text colour.",
		),
		Field::optional(
			"width",
			ValueType::nullable(ValueType::Float),
			Value::Null,
			"The width in drawing units, or null for the theme's.",
		),
	])
}

impl Extension for Ink {
	fn name(&self) -> &'static str {
		INK
	}

	fn schema(&self, schema: SchemaBuilder) -> SchemaBuilder {
		schema.block(
			BlockSpec::leaf(INK).attrs(
				Attrs::new()
					.with("height", DEFAULT_HEIGHT)
					.with("strokes", Vec::<Value>::new()),
			),
		)
	}

	fn nodes(&self) -> Vec<NodeMeta> {
		vec![
			NodeMeta::new(NodeKind::Block, INK)
				.attr(Field::optional(
					"height",
					ValueType::Int,
					DEFAULT_HEIGHT,
					"The height in drawing units; the width is always 1000.",
				))
				.attr(Field::optional(
					"strokes",
					ValueType::list(stroke_type()),
					Value::from(Vec::<Value>::new()),
					"The strokes, oldest first.",
				)),
		]
	}

	fn commands(&self, _schema: &Schema) -> Result<Vec<CommandSpec>, EditorError> {
		let insert_spec = CommandSpec::new(
			"insertInk",
			"Insert an empty drawing at the selection.",
			|args| {
				let height = args.int("height")?;

				if !(1..=MAX_HEIGHT).contains(&height) {
					return Err(EditorError::InvalidArgs {
						name: "insertInk".to_owned(),
						reason: format!("`height` must be from 1 to {MAX_HEIGHT}"),
					});
				}

				Ok(insert(height))
			},
		)
		.arg(Field::optional(
			"height",
			ValueType::Int,
			DEFAULT_HEIGHT,
			"The height in drawing units, from 1 to 4000.",
		));
		let add = CommandSpec::new(
			"addInkStroke",
			"Add a stroke to a drawing.",
			add_stroke_command,
		)
		.arg(ink_field())
		.arg(Field::required(
			"points",
			ValueType::list(ValueType::Float),
			"`x, y` pairs in drawing units.",
		))
		.arg(Field::optional(
			"color",
			ValueType::nullable(ValueType::String),
			Value::Null,
			"A `#rrggbb` colour, or null for the text colour.",
		))
		.arg(Field::optional(
			"width",
			ValueType::nullable(ValueType::Float),
			Value::Null,
			"The width in drawing units, or null for the theme's.",
		));
		let undo = CommandSpec::new(
			"undoInkStroke",
			"Remove the last stroke of a drawing.",
			|args| by_index(args, "undoInkStroke", |strokes| drop(strokes.pop())),
		)
		.arg(ink_field());
		let clear = CommandSpec::new("clearInk", "Remove every stroke of a drawing.", |args| {
			by_index(args, "clearInk", Vec::clear)
		})
		.arg(ink_field());

		Ok(vec![insert_spec, add, undo, clear])
	}

	fn queries(&self, _schema: &Schema) -> Result<Vec<QuerySpec>, EditorError> {
		Ok(vec![QuerySpec::new(
			"inks",
			"Every drawing in the document, in order, with its height and how many strokes it has.",
			ValueType::list(ValueType::Record(vec![
				Field::required("height", ValueType::Int, "The height in drawing units."),
				Field::required("strokes", ValueType::Int, "How many strokes it has."),
			])),
			|state, _| Ok(inks_value(state)),
		)])
	}
}

impl TypedExtension for Ink {
	type Handle = InkHandle;

	fn handle(&self, registration: Registration) -> InkHandle {
		InkHandle {
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

	const LINE: InkStroke<'static> = InkStroke {
		points: &[0.0, 0.0, 10.0, 10.0],
		color: None,
		width: None,
	};

	fn run(editor: &mut Editor, command: &Command) -> bool {
		let mut env = Env::headless(Duration::ZERO);
		editor.run(command, &mut env).expect("ran").is_some()
	}

	/// An editor holding one drawing `height` units tall.
	fn drawing(height: i64) -> (Editor, InkHandle, BlockId) {
		let mut builder = Editor::builder().extension(Document).extension(Paragraph);
		let ink = builder.add(Ink);
		let mut editor = builder.build().expect("valid editor");

		assert!(run(&mut editor, &ink.insert(height)));
		let block = ink.drawings(editor.state())[0];

		(editor, ink, block)
	}

	fn attrs(editor: &Editor, block: BlockId) -> Attrs {
		editor
			.state()
			.doc()
			.block(block)
			.expect("a drawing")
			.attrs()
			.clone()
	}

	#[test]
	fn strokes_are_rounded_kept_to_the_page_and_validated() {
		let stroke =
			stroke_value(&[1.04, 2.06, 1200.0, -5.0], Some("#FF0000"), Some(3.0)).expect("valid");
		let map = stroke.as_map().expect("a record");
		let expected = [1.0, 2.1, f64::from(INK_WIDTH), 0.0].map(Value::Float);

		assert_eq!(map.get("points"), Some(&Value::from(expected.to_vec())));
		assert_eq!(map.get("color"), Some(&Value::from("#ff0000")));
		assert!(stroke_value(&[], None, None).is_err());
		assert!(stroke_value(&[1.0], None, None).is_err());
		assert!(stroke_value(&[1.0, f32::NAN], None, None).is_err());
		assert!(stroke_value(&[1.0, 2.0], Some("red"), None).is_err());
		assert!(stroke_value(&[1.0, 2.0], Some("ff0000"), None).is_err());
		assert!(stroke_value(&[1.0, 2.0], None, Some(0.0)).is_err());
	}

	#[test]
	fn widths_keep_their_shortest_decimal() {
		assert_eq!(width_value(3.3).ok(), Some(3.3));
		assert!(width_value(f32::INFINITY).is_err());
	}

	#[test]
	fn heights_are_kept_to_the_range_the_command_accepts() {
		let (editor, _, block) = drawing(0);
		assert_eq!(height(&attrs(&editor, block)), 1);

		let (editor, _, block) = drawing(MAX_HEIGHT * 2);
		assert_eq!(height(&attrs(&editor, block)), MAX_HEIGHT);
	}

	#[test]
	fn strokes_are_added_undone_and_cleared() {
		let (mut editor, ink, block) = drawing(DEFAULT_HEIGHT);
		let add = ink.add_stroke(block, &LINE).expect("valid stroke");

		assert!(!run(&mut editor, &ink.undo_stroke(block)));
		assert!(!run(&mut editor, &ink.clear(block)));

		assert!(run(&mut editor, &add));
		assert!(run(&mut editor, &add));
		assert_eq!(strokes(&attrs(&editor, block)).len(), 2);

		assert!(run(&mut editor, &ink.undo_stroke(block)));
		assert_eq!(strokes(&attrs(&editor, block)).len(), 1);

		assert!(run(&mut editor, &ink.clear(block)));
		assert!(strokes(&attrs(&editor, block)).is_empty());
	}

	#[test]
	fn strokes_only_go_on_drawings() {
		let (mut editor, ink, _) = drawing(DEFAULT_HEIGHT);
		let paragraph = editor
			.state()
			.doc()
			.first_text_block()
			.expect("a paragraph");
		let add = ink.add_stroke(paragraph, &LINE).expect("valid stroke");

		assert!(!run(&mut editor, &add));
	}
}
