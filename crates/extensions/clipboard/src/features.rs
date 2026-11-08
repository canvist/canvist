//! Which extensions' markup is compiled in.
//!
//! Each Cargo feature teaches the codecs one extension's markup. Writers
//! match on [`gated`] names, so a name whose feature is off falls through
//! to the arm for unknown names.

/// Whether the markup for `name` is compiled in: always, unless it belongs
/// to an optional extension whose feature is off. A `match`, not a table
/// scan, since the writers ask for every block and mark they write.
#[allow(
	clippy::match_like_matches_macro,
	clippy::match_same_arms,
	reason = "each arm's value depends on a feature"
)]
pub(super) fn compiled(name: &str) -> bool {
	match name {
		"audio" => cfg!(feature = "audio"),
		"callout" => cfg!(feature = "callout"),
		"comment" => cfg!(feature = "comments"),
		"conditional" => cfg!(feature = "conditional"),
		"details" | "detailsContent" | "detailsSummary" => cfg!(feature = "details"),
		"emoji" => cfg!(feature = "emoji"),
		"file" => cfg!(feature = "file"),
		"hashtag" => cfg!(feature = "hashtag"),
		"highlight" => cfg!(feature = "highlight"),
		"image" => cfg!(feature = "image"),
		"ink" => cfg!(feature = "ink"),
		"kinetic" => cfg!(feature = "kinetic"),
		"blockMath" | "inlineMath" => cfg!(feature = "mathematics"),
		"mention" => cfg!(feature = "mention"),
		"subscript" => cfg!(feature = "subscript"),
		"superscript" => cfg!(feature = "superscript"),
		"table" | "tableCell" | "tableHeader" | "tableRow" => cfg!(feature = "table"),
		"textStyle" => cfg!(feature = "text-style"),
		"twitch" => cfg!(feature = "twitch"),
		"youtube" => cfg!(feature = "youtube"),
		_ => true,
	}
}

/// `name`, or the empty string when its feature is off, so that it matches
/// no named arm.
pub(super) fn gated(name: &str) -> &str {
	if compiled(name) { name } else { "" }
}
