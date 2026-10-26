//! The navigation policy shared by browser-facing document serializers.

/// Whether a link target may be emitted as an HTML `href`.
///
/// Allows HTTP, HTTPS, and mailto schemes (case-insensitively), plus
/// relative references, including fragments and protocol-relative URLs.
/// Callers must still HTML-escape the attribute value. This policy is for
/// links, not resource URLs such as image sources or iframe targets.
#[must_use]
pub fn is_safe_link_url(href: &str) -> bool {
	let lower = href.trim_start().to_ascii_lowercase();
	let path_start = lower.find(['/', '?', '#']).unwrap_or(lower.len());
	// A colon after the path starts is part of the reference, not a scheme.
	let has_scheme = lower.find(':').is_some_and(|end| end < path_start);

	!has_scheme
		|| ["http:", "https:", "mailto:"]
			.iter()
			.any(|scheme| lower.starts_with(scheme))
}
