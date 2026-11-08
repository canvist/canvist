//! Which extensions' markup is compiled in.
//!
//! Each Cargo feature teaches the codecs one extension's markup. Writers
//! match on [`gated`] names, so a name whose feature is off falls through
//! to the arm for unknown names.

/// The block, mark, and embed names that belong to an optional extension,
/// and whether its feature is on.
const FEATURES: [(&str, bool); 26] = [
	("audio", cfg!(feature = "audio")),
	("blockMath", cfg!(feature = "mathematics")),
	("callout", cfg!(feature = "callout")),
	("comment", cfg!(feature = "comments")),
	("conditional", cfg!(feature = "conditional")),
	("details", cfg!(feature = "details")),
	("detailsContent", cfg!(feature = "details")),
	("detailsSummary", cfg!(feature = "details")),
	("emoji", cfg!(feature = "emoji")),
	("file", cfg!(feature = "file")),
	("hashtag", cfg!(feature = "hashtag")),
	("highlight", cfg!(feature = "highlight")),
	("image", cfg!(feature = "image")),
	("ink", cfg!(feature = "ink")),
	("inlineMath", cfg!(feature = "mathematics")),
	("kinetic", cfg!(feature = "kinetic")),
	("mention", cfg!(feature = "mention")),
	("subscript", cfg!(feature = "subscript")),
	("superscript", cfg!(feature = "superscript")),
	("table", cfg!(feature = "table")),
	("tableCell", cfg!(feature = "table")),
	("tableHeader", cfg!(feature = "table")),
	("tableRow", cfg!(feature = "table")),
	("textStyle", cfg!(feature = "text-style")),
	("twitch", cfg!(feature = "twitch")),
	("youtube", cfg!(feature = "youtube")),
];

/// Whether the markup for `name` is compiled in: always, unless
/// [`FEATURES`] lists it with its feature off.
pub(super) fn compiled(name: &str) -> bool {
	!FEATURES.iter().any(|&(gated, on)| gated == name && !on)
}

/// `name`, or the empty string when its feature is off, so that it matches
/// no named arm.
pub(super) fn gated(name: &str) -> &str {
	if compiled(name) { name } else { "" }
}
