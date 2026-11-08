//! Which extensions' markup is compiled in.
//!
//! Each Cargo feature teaches the codecs one extension's markup. Each
//! extension's arm in a reader or writer is guarded by its constant here,
//! such as `"table" if TABLE`, so a build without the feature drops that
//! code entirely, and its names fall through to the arms for unknown
//! names, as [`gated`] and [`compiled`] report them.

pub(super) const AUDIO: bool = cfg!(feature = "audio");
pub(super) const CALLOUT: bool = cfg!(feature = "callout");
pub(super) const COMMENTS: bool = cfg!(feature = "comments");
pub(super) const CONDITIONAL: bool = cfg!(feature = "conditional");
pub(super) const DETAILS: bool = cfg!(feature = "details");
pub(super) const EMOJI: bool = cfg!(feature = "emoji");
pub(super) const FILE: bool = cfg!(feature = "file");
pub(super) const HASHTAG: bool = cfg!(feature = "hashtag");
pub(super) const HIGHLIGHT: bool = cfg!(feature = "highlight");
pub(super) const IMAGE: bool = cfg!(feature = "image");
pub(super) const INK: bool = cfg!(feature = "ink");
pub(super) const KINETIC: bool = cfg!(feature = "kinetic");
pub(super) const MATHEMATICS: bool = cfg!(feature = "mathematics");
pub(super) const MENTION: bool = cfg!(feature = "mention");
pub(super) const SUBSCRIPT: bool = cfg!(feature = "subscript");
pub(super) const SUPERSCRIPT: bool = cfg!(feature = "superscript");
pub(super) const TABLE: bool = cfg!(feature = "table");
pub(super) const TEXT_STYLE: bool = cfg!(feature = "text-style");
pub(super) const TWITCH: bool = cfg!(feature = "twitch");
pub(super) const YOUTUBE: bool = cfg!(feature = "youtube");

/// Whether the markup for `name` is compiled in: always, unless it belongs
/// to an optional extension whose feature is off.
pub(super) fn compiled(name: &str) -> bool {
	match name {
		"audio" => AUDIO,
		"blockMath" | "inlineMath" => MATHEMATICS,
		"callout" => CALLOUT,
		"comment" => COMMENTS,
		"conditional" => CONDITIONAL,
		"details" | "detailsContent" | "detailsSummary" => DETAILS,
		"emoji" => EMOJI,
		"file" => FILE,
		"hashtag" => HASHTAG,
		"highlight" => HIGHLIGHT,
		"image" => IMAGE,
		"ink" => INK,
		"kinetic" => KINETIC,
		"mention" => MENTION,
		"subscript" => SUBSCRIPT,
		"superscript" => SUPERSCRIPT,
		"table" | "tableCell" | "tableHeader" | "tableRow" => TABLE,
		"textStyle" => TEXT_STYLE,
		"twitch" => TWITCH,
		"youtube" => YOUTUBE,
		_ => true,
	}
}

/// `name`, or the empty string when its feature is off, so that it matches
/// no named arm.
pub(super) fn gated(name: &str) -> &str {
	if compiled(name) { name } else { "" }
}
