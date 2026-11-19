//! What a `User-Agent` says: a bot, or a device, browser and OS.
//!
//! Families only, never versions: enough to see who visits, too little to
//! fingerprint anyone.

use serde::Serialize;

/// The kind of device an event came from.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Device {
	/// A computer.
	Desktop,
	/// A phone.
	Mobile,
	/// A tablet.
	Tablet,
	/// The canvist server itself.
	Server,
}

impl Device {
	/// The name as stored.
	pub fn as_str(self) -> &'static str {
		match self {
			Self::Desktop => "desktop",
			Self::Mobile => "mobile",
			Self::Tablet => "tablet",
			Self::Server => "server",
		}
	}

	/// The device stored as `name`.
	pub fn parse(name: &str) -> Option<Self> {
		[Self::Desktop, Self::Mobile, Self::Tablet, Self::Server]
			.into_iter()
			.find(|device| device.as_str() == name)
	}
}

/// A browser's user agent, classified.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Agent {
	/// The kind of device.
	pub device: Device,
	/// The browser family.
	pub browser: &'static str,
	/// The operating system.
	pub os: &'static str,
}

/// Fragments that only crawlers, scripts and headless browsers send.
const BOTS: [&str; 14] = [
	"bot",
	"crawl",
	"spider",
	"slurp",
	"headless",
	"lighthouse",
	"preview",
	"facebookexternalhit",
	"curl/",
	"wget/",
	"python",
	"node-fetch",
	"go-http-client",
	"axios/",
];

/// Browser families, most specific first: Edge and Opera also say Chrome,
/// and Chrome also says Safari.
const BROWSERS: [(&[&str], &str); 5] = [
	(&["edg/", "edga/", "edgios/"], "Edge"),
	(&["opr/", "opera"], "Opera"),
	(&["firefox/", "fxios/"], "Firefox"),
	(&["chrome/", "crios/", "chromium/"], "Chrome"),
	(&["safari/"], "Safari"),
];

/// Operating systems, most specific first: iPads and Android also say
/// macOS and Linux.
const SYSTEMS: [(&[&str], &str); 6] = [
	(&["iphone", "ipad", "ipod"], "iOS"),
	(&["android"], "Android"),
	(&["windows"], "Windows"),
	(&["cros"], "ChromeOS"),
	(&["mac os x", "macintosh"], "macOS"),
	(&["linux"], "Linux"),
];

impl Agent {
	/// Classify `user_agent`, or `None` for a bot or an empty agent.
	pub fn classify(user_agent: &str) -> Option<Self> {
		let agent = user_agent.to_ascii_lowercase();

		if agent.trim().is_empty() || BOTS.iter().any(|bot| agent.contains(bot)) {
			return None;
		}

		Some(Self {
			device: device(&agent),
			browser: first_match(&agent, &BROWSERS),
			os: first_match(&agent, &SYSTEMS),
		})
	}
}

fn device(agent: &str) -> Device {
	if agent.contains("ipad") || agent.contains("tablet") {
		Device::Tablet
	} else if agent.contains("android") {
		// Android tablets leave `mobile` out.
		if agent.contains("mobile") {
			Device::Mobile
		} else {
			Device::Tablet
		}
	} else if agent.contains("mobi") || agent.contains("iphone") {
		Device::Mobile
	} else {
		Device::Desktop
	}
}

fn first_match(agent: &str, families: &[(&[&str], &'static str)]) -> &'static str {
	families
		.iter()
		.find(|(fragments, _)| fragments.iter().any(|fragment| agent.contains(fragment)))
		.map_or("Other", |(_, name)| name)
}

#[cfg(test)]
mod tests {
	use super::*;

	const MAC_CHROME: &str = concat!(
		"Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 ",
		"(KHTML, like Gecko) Chrome/130.0.0.0 Safari/537.36",
	);
	const IPHONE_SAFARI: &str = concat!(
		"Mozilla/5.0 (iPhone; CPU iPhone OS 18_0 like Mac OS X) AppleWebKit/605.1.15 ",
		"(KHTML, like Gecko) Version/18.0 Mobile/15E148 Safari/604.1",
	);
	const ANDROID_TABLET: &str = concat!(
		"Mozilla/5.0 (Linux; Android 14; SM-X710) AppleWebKit/537.36 ",
		"(KHTML, like Gecko) Chrome/130.0.0.0 Safari/537.36",
	);
	const WINDOWS_EDGE: &str = concat!(
		"Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 ",
		"(KHTML, like Gecko) Chrome/130.0.0.0 Safari/537.36 Edg/130.0.0.0",
	);

	#[test]
	fn browsers_are_classified_by_family() {
		let agent = |ua| Agent::classify(ua).expect("a browser");
		assert_eq!(
			agent(MAC_CHROME),
			Agent {
				device: Device::Desktop,
				browser: "Chrome",
				os: "macOS"
			}
		);
		assert_eq!(
			agent(IPHONE_SAFARI),
			Agent {
				device: Device::Mobile,
				browser: "Safari",
				os: "iOS"
			}
		);
		assert_eq!(agent(ANDROID_TABLET).device, Device::Tablet);
		assert_eq!(agent(ANDROID_TABLET).os, "Android");
		assert_eq!(agent(WINDOWS_EDGE).browser, "Edge");
		assert_eq!(agent(WINDOWS_EDGE).os, "Windows");
	}

	#[test]
	fn bots_and_scripts_are_dropped() {
		for ua in [
			"Mozilla/5.0 (compatible; Googlebot/2.1; +http://www.google.com/bot.html)",
			"Mozilla/5.0 (X11; Linux x86_64) HeadlessChrome/130.0.0.0 Safari/537.36",
			"curl/8.7.1",
			"",
		] {
			assert_eq!(Agent::classify(ua), None, "{ua}");
		}
	}

	#[test]
	fn devices_round_trip_through_their_names() {
		for device in [
			Device::Desktop,
			Device::Mobile,
			Device::Tablet,
			Device::Server,
		] {
			assert_eq!(Device::parse(device.as_str()), Some(device));
		}
		assert_eq!(Device::parse("toaster"), None);
	}
}
