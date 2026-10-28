//! Provisional text: laid out in a block without being part of the
//! document, drawn faint, and fading into ink once it is final.
//!
//! Geometry is in terms of the [`FixedMeasurer`]: every character advances
//! [`ADVANCE`] pixels.

#![allow(clippy::float_cmp, reason = "layout with the fixed measurer is exact")]

mod support;

use canvist_layout::Caret;
use canvist_layout::DisplayItem;
use canvist_layout::DisplayList;
use canvist_layout::DisplayOptions;
use canvist_layout::DocumentLayout;
use canvist_layout::FixedMeasurer;
use canvist_layout::LineEdge;
use canvist_layout::Provisional;
use canvist_model::BlockId;
use canvist_model::Document;
use canvist_model::Point;
use canvist_model::Selection;
use support::layout;
use support::paragraphs;

/// The advance of every character in the 16 px body font: 0.6 em.
const ADVANCE: f32 = 9.6;
/// The height of a body line: 16 px at a line height of 1.5.
const LINE: f32 = 24.0;
/// Wide enough that no paragraph in these tests wraps.
const WIDE: f32 = 600.0;
/// Fits "aaa bbb " but not "aaa bbb ccc", so that text wraps after "bbb".
const NARROW: f32 = 70.0;
/// The alpha of the body text colour.
const OPAQUE: u8 = 255;
/// The alpha of provisional text with no animation clock: 45% of opaque.
const FAINT: u8 = 115;

/// The width of `text`.
fn width(text: &str) -> f32 {
	support::r(text.chars().count() as f32 * ADVANCE)
}

fn first_block(doc: &Document) -> BlockId {
	doc.children(None)[0]
}

/// `text` laid out at `width` with `provisional` shown at byte `at` of the
/// first paragraph.
fn with_provisional(
	text: &str,
	at: u32,
	provisional: &str,
	width: f32,
) -> (Document, DocumentLayout) {
	let doc = paragraphs(&[text]);
	let mut layout = layout(&doc, width);
	let block = first_block(&doc);

	assert!(layout.set_provisional(Some(Provisional::new(Point::new(block, at), provisional))));
	relayout(&doc, &mut layout);
	(doc, layout)
}

fn relayout(doc: &Document, layout: &mut DocumentLayout) {
	layout
		.layout_all(doc, &mut FixedMeasurer::new())
		.expect("the fixed measurer honours the contract");
}

fn caret_x(layout: &DocumentLayout, block: BlockId, offset: u32) -> f32 {
	let rect = layout
		.caret_rect(Caret::new(Point::new(block, offset)))
		.expect("a caret rectangle");
	support::r(rect.x + rect.width / 2.0)
}

/// The painted text as runs of one alpha each: neighbouring runs with the
/// same alpha are joined, so tests see faded regions rather than where
/// layout split words.
fn runs(list: &DisplayList) -> Vec<(String, u8)> {
	let mut regions: Vec<(String, u8)> = Vec::new();

	for item in &list.items {
		let DisplayItem::Text(run) = item else {
			continue;
		};

		match regions.last_mut() {
			Some((text, alpha)) if *alpha == run.color.a => text.push_str(run.text()),
			_ => regions.push((run.text().to_owned(), run.color.a)),
		}
	}

	regions
}

fn paint(layout: &DocumentLayout, options: &DisplayOptions) -> DisplayList {
	layout.display_list(options)
}

#[test]
fn a_caret_where_provisional_text_shows_sits_after_it() {
	let (doc, layout) = with_provisional("Hello", 5, " world", WIDE);
	let block = first_block(&doc);

	assert_eq!(caret_x(&layout, block, 5), width("Hello world"));
	assert_eq!(caret_x(&layout, block, 2), width("He"));
}

#[test]
fn text_after_provisional_text_reflows_as_it_will_once_final() {
	let (doc, layout) = with_provisional("aaa ccc", 4, "bbb ", NARROW);
	let block = first_block(&doc);

	assert_eq!(layout.line_count(block), Some(2));
	// Line ranges stay in document offsets: "aaa " then "ccc".
	assert_eq!(layout.line_ranges(block), Some(vec![0..4, 4..7]));
	assert_eq!(caret_x(&layout, block, 5), width("c"));
}

#[test]
fn clicking_provisional_text_puts_the_caret_where_it_shows() {
	let (doc, layout) = with_provisional("Hello", 5, " world", WIDE);
	let block = first_block(&doc);
	let inside = width("Hello wo");

	let hit = layout.hit_test(inside, LINE / 2.0).expect("a hit");
	assert_eq!(hit.point, Point::new(block, 5));

	let (doc, layout) = with_provisional("ab cd", 2, " xyz", WIDE);
	let after = width("ab xyz c") + ADVANCE / 4.0;
	let hit = layout.hit_test(after, LINE / 2.0).expect("a hit");
	assert_eq!(hit.point, Point::new(first_block(&doc), 4));
}

#[test]
fn selections_and_line_edges_stay_in_document_offsets() {
	let (doc, mut layout) = with_provisional("aaa ccc", 4, "bbb ", NARROW);
	let block = first_block(&doc);
	let selection = Selection::text(Point::new(block, 4), Point::new(block, 7));
	let rects = layout.selection_rects(&selection);

	// "ccc" alone is selected, on the second line; provisional text is not.
	assert_eq!(rects.len(), 1);
	assert_eq!(support::r(rects[0].x), 0.0);
	assert_eq!(support::r(rects[0].width), width("ccc"));

	let mut measurer = FixedMeasurer::new();
	let mut navigator = layout.navigator(&doc, &mut measurer);
	let end = navigator
		.line_edge(Caret::new(Point::new(block, 0)), LineEdge::End)
		.expect("the line's end");
	assert_eq!(end.point, Point::new(block, 4));
}

#[test]
fn provisional_text_is_drawn_faint_beside_the_document_text() {
	let (_, layout) = with_provisional("Hello", 5, " world", WIDE);
	let list = paint(&layout, &DisplayOptions::viewport(0.0, LINE));

	assert_eq!(
		runs(&list),
		[("Hello".to_owned(), OPAQUE), (" world".to_owned(), FAINT)]
	);
	assert!(!list.animated);
}

#[test]
fn provisional_text_breathes_while_the_clock_runs() {
	let (_, layout) = with_provisional("Hello", 5, " world", WIDE);
	let list = paint(&layout, &DisplayOptions::viewport(0.0, LINE).time(0.6));

	let (_, alpha) = runs(&list)[1].clone();
	assert!(alpha > FAINT);
	assert!(list.animated);
}

#[test]
fn final_text_fades_into_ink_while_the_clock_runs() {
	let doc = paragraphs(&["Hello world"]);
	let layout = layout(&doc, WIDE);
	let block = first_block(&doc);
	let settling = |elapsed| {
		DisplayOptions::viewport(0.0, LINE)
			.time(1.0)
			.settling(block, 6..11, elapsed)
	};

	let start = paint(&layout, &settling(0.0));
	assert_eq!(
		runs(&start),
		[("Hello ".to_owned(), OPAQUE), ("world".to_owned(), FAINT)]
	);
	assert!(start.animated);

	let done = paint(&layout, &settling(1.0));
	assert_eq!(runs(&done), [("Hello world".to_owned(), OPAQUE)]);
	assert!(!done.animated);
}

#[test]
fn without_a_clock_final_text_is_ink_at_once() {
	let doc = paragraphs(&["Hello world"]);
	let layout = layout(&doc, WIDE);
	let options = DisplayOptions::viewport(0.0, LINE).settling(first_block(&doc), 6..11, 0.0);

	assert_eq!(
		runs(&paint(&layout, &options)),
		[("Hello world".to_owned(), OPAQUE)]
	);
}

#[test]
fn removing_provisional_text_restores_the_document_layout() {
	let (doc, mut layout) = with_provisional("Hello", 5, " world", WIDE);
	let block = first_block(&doc);

	assert!(!layout.set_provisional(layout.provisional().cloned()));
	assert!(layout.set_provisional(None));
	relayout(&doc, &mut layout);

	assert_eq!(caret_x(&layout, block, 5), width("Hello"));
	assert_eq!(
		runs(&paint(&layout, &DisplayOptions::viewport(0.0, LINE))),
		[("Hello".to_owned(), OPAQUE)]
	);
}

#[test]
fn a_point_the_block_no_longer_has_shows_nothing() {
	let (doc, layout) = with_provisional("Hi", 9, "lost", WIDE);

	let lines = layout.line_ranges(first_block(&doc)).expect("measured");
	assert_eq!(lines.len(), 1);
	assert_eq!(lines[0], 0..2);
}

/// Each text run's text, left edge, and width.
fn geometry(list: &DisplayList) -> Vec<(String, f32, f32)> {
	list.items
		.iter()
		.filter_map(|item| {
			let DisplayItem::Text(run) = item else {
				return None;
			};
			Some((
				run.text().to_owned(),
				support::r(run.x),
				support::r(run.width),
			))
		})
		.collect()
}

#[test]
fn a_fade_inside_a_word_splits_it_where_its_letters_are() {
	let doc = paragraphs(&["Hello"]);
	let layout = layout(&doc, WIDE);
	let options =
		DisplayOptions::viewport(0.0, LINE)
			.time(1.0)
			.settling(first_block(&doc), 2..5, 0.0);

	assert_eq!(
		geometry(&paint(&layout, &options)),
		[
			("He".to_owned(), 0.0, width("He")),
			("llo".to_owned(), width("He"), width("llo")),
		]
	);
}

#[test]
fn a_fade_inside_right_to_left_text_splits_from_the_right() {
	let doc = paragraphs(&["שלום"]);
	let layout = layout(&doc, WIDE);
	let options =
		DisplayOptions::viewport(0.0, LINE)
			.time(1.0)
			.settling(first_block(&doc), 4..8, 0.0);
	let parts = geometry(&paint(&layout, &options));

	// The first two letters are drawn rightmost, the last two left of them.
	assert_eq!(parts.len(), 2);
	assert_eq!(parts[0].0, "של");
	assert_eq!(
		support::r(parts[0].1 + parts[0].2 - parts[1].1),
		width("שלום")
	);
	assert_eq!(parts[1].0, "ום");
	assert_eq!(support::r(parts[1].1 + parts[1].2), support::r(parts[0].1));
}
