//! Adornments: text laid out in a block without being part of the
//! document, such as revealed Markdown syntax, sitting between the upstream
//! and downstream carets at its point and drawn faint.
//!
//! Geometry is in terms of the [`FixedMeasurer`]: every character advances
//! [`ADVANCE`] pixels.

#![allow(clippy::float_cmp, reason = "layout with the fixed measurer is exact")]

mod support;

use canvist_layout::Adornment;
use canvist_layout::Affinity;
use canvist_layout::Caret;
use canvist_layout::DisplayItem;
use canvist_layout::DisplayOptions;
use canvist_layout::DocumentLayout;
use canvist_layout::FixedMeasurer;
use canvist_layout::LineEdge;
use canvist_model::BlockId;
use canvist_model::Document;
use canvist_model::Point;
use support::layout;
use support::paragraphs;

/// The advance of every character in the 16 px body font: 0.6 em.
const ADVANCE: f32 = 9.6;
/// The height of a body line: 16 px at a line height of 1.5.
const LINE: f32 = 24.0;
/// Wide enough that no paragraph in these tests wraps.
const WIDE: f32 = 600.0;
/// The alpha of the body text colour.
const OPAQUE: u8 = 255;
/// The alpha of adornments: 42% of opaque.
const FAINT: u8 = 107;

fn width(text: &str) -> f32 {
	support::r(text.chars().count() as f32 * ADVANCE)
}

fn first_block(doc: &Document) -> BlockId {
	doc.children(None)[0]
}

fn relayout(doc: &Document, layout: &mut DocumentLayout) {
	layout
		.layout_all(doc, &mut FixedMeasurer::new())
		.expect("the fixed measurer honours the contract");
}

/// "Say bold now" with `**` around "bold", as Markdown reveal shows it.
fn bold_revealed() -> (Document, DocumentLayout, BlockId) {
	let doc = paragraphs(&["Say bold now"]);
	let mut layout = layout(&doc, WIDE);
	let block = first_block(&doc);

	assert!(layout.set_adornments(vec![
		Adornment::new(Point::new(block, 4), "**"),
		Adornment::new(Point::new(block, 8), "**"),
	]));
	relayout(&doc, &mut layout);
	(doc, layout, block)
}

fn caret_x(layout: &DocumentLayout, block: BlockId, offset: u32, affinity: Affinity) -> f32 {
	let rect = layout
		.caret_rect(Caret::with_affinity(Point::new(block, offset), affinity))
		.expect("a caret rectangle");
	support::r(rect.x + rect.width / 2.0)
}

#[test]
fn an_adornment_sits_between_the_two_carets_at_its_point() {
	let (_, layout, block) = bold_revealed();

	assert_eq!(
		caret_x(&layout, block, 4, Affinity::Upstream),
		width("Say ")
	);
	assert_eq!(
		caret_x(&layout, block, 4, Affinity::Downstream),
		width("Say **")
	);
	assert_eq!(
		caret_x(&layout, block, 8, Affinity::Upstream),
		width("Say **bold")
	);
	assert_eq!(
		caret_x(&layout, block, 8, Affinity::Downstream),
		width("Say **bold**")
	);
	assert_eq!(
		caret_x(&layout, block, 10, Affinity::Downstream),
		width("Say **bold** n")
	);
}

#[test]
fn clicking_an_adornment_lands_on_its_nearer_side() {
	let (_, layout, block) = bold_revealed();
	let at = |x: f32| layout.hit_test(x, LINE / 2.0).expect("a hit");

	// The left half of the opening `**` is before it, the right half after.
	let before = at(width("Say ") + ADVANCE / 4.0);
	assert_eq!(
		(before.point, before.affinity),
		(Point::new(block, 4), Affinity::Upstream)
	);
	let after = at(width("Say **") - ADVANCE / 4.0);
	assert_eq!(
		(after.point, after.affinity),
		(Point::new(block, 4), Affinity::Downstream)
	);
	// Text after the adornments keeps its document offset.
	let word = at(width("Say **bold** n") + ADVANCE / 4.0);
	assert_eq!(word.point, Point::new(block, 10));
}

#[test]
fn line_edges_and_line_ranges_stay_in_document_offsets() {
	let doc = paragraphs(&["Title"]);
	let mut layout = layout(&doc, WIDE);
	let block = first_block(&doc);
	layout.set_adornments(vec![Adornment::new(Point::new(block, 0), "## ")]);
	relayout(&doc, &mut layout);

	let ranges = layout.line_ranges(block).expect("laid out");
	assert_eq!(ranges.len(), 1);
	assert_eq!(
		ranges[0],
		0..5,
		"the line holds the document's five bytes, not the `## `"
	);
	let mut measurer = FixedMeasurer::new();
	let mut lines = layout.navigator(&doc, &mut measurer);
	let start = lines
		.line_edge(Caret::new(Point::new(block, 3)), LineEdge::Start)
		.expect("a line start");
	// The start of the line is before the adornment.
	assert_eq!(
		(start.point, start.affinity),
		(Point::new(block, 0), Affinity::Upstream)
	);
}

#[test]
fn adornments_are_drawn_faint_beside_the_document_text() {
	let (_, layout, _) = bold_revealed();
	let list = layout.display_list(&DisplayOptions::viewport(0.0, 600.0));
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

	assert_eq!(
		regions,
		vec![
			("Say ".to_owned(), OPAQUE),
			("**".to_owned(), FAINT),
			("bold".to_owned(), OPAQUE),
			("**".to_owned(), FAINT),
			(" now".to_owned(), OPAQUE),
		]
	);
	assert!(!list.animated, "adornments are steady");
}

#[test]
fn removing_adornments_restores_the_document_layout() {
	let (doc, mut layout, block) = bold_revealed();

	assert!(layout.set_adornments(Vec::new()));
	assert!(!layout.set_adornments(Vec::new()), "nothing changed");
	relayout(&doc, &mut layout);
	assert_eq!(
		caret_x(&layout, block, 8, Affinity::Downstream),
		width("Say bold")
	);
}
