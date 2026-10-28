//! Faded text: provisional text drawn faint, and text that just became
//! final fading into ink.
//!
//! Provisional text breathes gently while the animation clock runs, so a
//! draft reads as alive. Text dictation has just inserted fades from the
//! provisional opacity to full over [`SETTLE_SECONDS`]. Without a clock, as
//! for readers who prefer reduced motion, provisional text is a steady
//! [`PROVISIONAL_ALPHA`] and final text is at full opacity at once.

use std::f32::consts::TAU;
use std::ops::Range;

use canvist_model::BlockId;

use super::TextRun;
use crate::text::Item;
use crate::text::TextLayout;

/// How opaque provisional text is, as a fraction of its colour's alpha.
const PROVISIONAL_ALPHA: f32 = 0.45;

/// How far provisional text breathes either side of [`PROVISIONAL_ALPHA`].
const BREATH: f32 = 0.1;

/// Seconds per breath.
const BREATH_PERIOD: f32 = 2.4;

/// Seconds text takes to settle from provisional to full opacity.
pub(crate) const SETTLE_SECONDS: f32 = 0.45;

/// A byte range of a block's laid-out text, drawn at `strength` times its
/// colour's alpha.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Fade {
	pub(crate) range: Range<u32>,
	pub(crate) strength: f32,
}

/// The faded ranges of `block`'s laid-out `text`, in order, and whether any
/// of them changes with time. `settling` holds document ranges and the
/// seconds since each settled; `time` is the animation clock.
pub(crate) fn fades(
	block: BlockId,
	text: &TextLayout,
	settling: &[(BlockId, Range<u32>, f32)],
	time: Option<f32>,
) -> (Vec<Fade>, bool) {
	let mut fades = Vec::new();
	let mut animated = false;

	if let Some(range) = &text.provisional {
		animated |= time.is_some();
		fades.push(Fade {
			range: range.clone(),
			strength: provisional_strength(time),
		});
	}

	if time.is_some() {
		for (_, range, elapsed) in settling.iter().filter(|(id, ..)| *id == block) {
			let Some(strength) = settling_strength(*elapsed) else {
				continue;
			};
			let range = text.laid_out(range.start)..text.laid_out(range.end);

			animated = true;
			fades.push(Fade { range, strength });
		}
	}

	fades.sort_by_key(|fade| fade.range.start);
	(fades, animated)
}

/// How opaque provisional text is at `time`.
fn provisional_strength(time: Option<f32>) -> f32 {
	let Some(time) = time else {
		return PROVISIONAL_ALPHA;
	};

	PROVISIONAL_ALPHA + BREATH * (TAU * time / BREATH_PERIOD).sin()
}

/// How opaque settling text is `elapsed` seconds after it settled, or `None`
/// once it is fully opaque. It eases out, so it arrives softly.
fn settling_strength(elapsed: f32) -> Option<f32> {
	let progress = (elapsed / SETTLE_SECONDS).max(0.0);

	if progress >= 1.0 {
		return None;
	}

	let eased = 1.0 - (1.0 - progress).powi(3);
	Some(PROVISIONAL_ALPHA + (1.0 - PROVISIONAL_ALPHA) * eased)
}

/// `run`, drawn from `item` of `text`, cut where `fades` start and end, each
/// part in its colour faded by the fade covering it. A run no fade touches
/// comes back whole.
pub(crate) fn split(run: TextRun, item: &Item, text: &str, fades: &[Fade]) -> Vec<TextRun> {
	let touches =
		|fade: &&Fade| fade.range.start < run.range.end && run.range.start < fade.range.end;

	if !fades.iter().any(|fade| touches(&fade)) {
		return vec![run];
	}

	let mut cuts: Vec<u32> = fades
		.iter()
		.filter(touches)
		.flat_map(|fade| [fade.range.start, fade.range.end])
		.filter(|&cut| run.range.start < cut && cut < run.range.end)
		.collect();
	cuts.push(run.range.start);
	cuts.push(run.range.end);
	cuts.sort_unstable();
	cuts.dedup();

	// Measured advances can differ from the drawn width, as on justified
	// lines, so parts share the run's width in proportion.
	let measured = item.span(text, run.range.start, run.range.end);
	let scale = if measured > 0.0 {
		run.width / measured
	} else {
		0.0
	};

	cuts.windows(2)
		.map(|cut| part(&run, cut[0]..cut[1], item, text, scale, fades))
		.collect()
}

/// The part of `run` covering `range`.
fn part(
	run: &TextRun,
	range: Range<u32>,
	item: &Item,
	text: &str,
	scale: f32,
	fades: &[Fade],
) -> TextRun {
	let before = item.span(text, run.range.start, range.start) * scale;
	let width = item.span(text, range.start, range.end) * scale;
	// Right-to-left text runs from the right edge.
	let x = if run.rtl {
		run.x + run.width - before - width
	} else {
		run.x + before
	};
	let strength = fades
		.iter()
		.find(|fade| fade.range.start <= range.start && range.start < fade.range.end)
		.map_or(1.0, |fade| fade.strength);
	let mut color = run.color;
	color.a = (f32::from(color.a) * strength).round() as u8;

	TextRun {
		range,
		x,
		width,
		color,
		bounds: crate::Rect::new(x, run.bounds.y, width, run.bounds.height),
		..run.clone()
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn settling_eases_from_provisional_to_opaque() {
		assert_eq!(settling_strength(0.0), Some(PROVISIONAL_ALPHA));
		assert!(settling_strength(SETTLE_SECONDS / 2.0).is_some_and(|strength| strength > 0.8));
		assert_eq!(settling_strength(SETTLE_SECONDS), None);
	}

	#[test]
	fn provisional_text_is_steady_without_a_clock() {
		assert!((provisional_strength(None) - PROVISIONAL_ALPHA).abs() < f32::EPSILON);

		let quarter = provisional_strength(Some(BREATH_PERIOD / 4.0));
		assert!((quarter - (PROVISIONAL_ALPHA + BREATH)).abs() < 1e-5);
	}
}
