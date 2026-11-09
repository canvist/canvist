//! The navigation and embedding policy shared by browser-facing document
//! serializers and the extensions that insert links and videos.

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

/// The video block that may embed `src` in an `<iframe>`: `youtube` for an
/// `http(s)://` or `//` URL on YouTube's hosts, `twitch` for one on
/// Twitch's, and `None` for anything else, such as another site or a
/// `javascript:` URL, which must never reach an `<iframe>`. TipTap's video
/// commands refuse such URLs too.
///
/// ```
/// use canvist_model::video_kind;
///
/// assert_eq!(video_kind("https://youtu.be/dQw4w9WgXcQ"), Some("youtube"));
/// assert_eq!(video_kind("//www.twitch.tv/videos/1"), Some("twitch"));
/// assert_eq!(video_kind("https://notyoutube.com/watch"), None);
/// assert_eq!(video_kind("javascript:alert(1)//youtube.com"), None);
/// ```
#[must_use]
pub fn video_kind(src: &str) -> Option<&'static str> {
	let lower = src.trim().to_ascii_lowercase();
	let rest = ["https://", "http://", "//"]
		.into_iter()
		.find_map(|scheme| lower.strip_prefix(scheme))?;
	let authority = rest.split(['/', '?', '#']).next().unwrap_or_default();
	let host = authority.rsplit('@').next().unwrap_or_default();
	let host = host.split(':').next().unwrap_or_default();
	let on = |domain: &str| {
		host.strip_suffix(domain)
			.is_some_and(|subdomain| subdomain.is_empty() || subdomain.ends_with('.'))
	};

	if ["youtube.com", "youtube-nocookie.com", "youtu.be"]
		.into_iter()
		.any(on)
	{
		Some("youtube")
	} else if on("twitch.tv") {
		Some("twitch")
	} else {
		None
	}
}
