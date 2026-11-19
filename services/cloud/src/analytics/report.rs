//! The dashboard's numbers, aggregated from raw events.
//!
//! [`Report::build`] is pure: it takes the events of a period and the one
//! before it (for comparison) and counts. Browser events make the web
//! numbers; server events are counted on their own, so a browser can never
//! inflate them.

use std::collections::HashMap;
use std::collections::HashSet;

use serde::Serialize;

use super::Device;
use super::Event;
use super::PAGEVIEW;

/// One UTC day in milliseconds.
pub const DAY_MILLIS: i64 = 24 * 60 * 60 * 1000;
/// Rows kept in each top list.
const TOP: usize = 10;
/// The source of a session that arrived with neither referrer nor campaign.
const DIRECT: &str = "direct";

/// A span of time, `[from, to)` in Unix milliseconds.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Period {
	/// The first millisecond.
	pub from: i64,
	/// The millisecond after the last.
	pub to: i64,
}

impl Period {
	/// The last `days` whole UTC days, today included.
	pub fn last_days(days: u32, now: i64) -> Self {
		let to = (now.div_euclid(DAY_MILLIS) + 1) * DAY_MILLIS;
		Self {
			from: to - i64::from(days) * DAY_MILLIS,
			to,
		}
	}

	/// The period of the same length just before this one.
	#[must_use]
	pub fn before(self) -> Self {
		Self {
			from: 2 * self.from - self.to,
			to: self.from,
		}
	}

	fn contains(self, at: i64) -> bool {
		(self.from..self.to).contains(&at)
	}
}

/// Everything the dashboard shows for a period.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Report {
	/// The period's first millisecond.
	pub from: i64,
	/// The millisecond after the period.
	pub to: i64,
	/// The period's headline numbers.
	pub summary: Summary,
	/// The same numbers for the period before, for comparison.
	pub previous: Summary,
	/// Visitors and page views per UTC day.
	pub days: Vec<Day>,
	/// The most viewed pages.
	pub pages: Vec<Count>,
	/// Where sessions came from: a referring host, a campaign, or `direct`.
	pub sources: Vec<Count>,
	/// Page views per kind of device.
	pub devices: Vec<Count>,
	/// Page views per browser.
	pub browsers: Vec<Count>,
	/// Clicks and other events the site sent.
	pub site_events: Vec<Count>,
	/// Events the server recorded: projects created, checkouts, plans.
	pub server_events: Vec<Count>,
	/// Whether the scan stopped at its limit, so the counts are low.
	pub truncated: bool,
}

/// Headline numbers.
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct Summary {
	/// Distinct browsers.
	pub visitors: u64,
	/// Distinct sessions.
	pub sessions: u64,
	/// Pages viewed.
	pub pageviews: u64,
	/// The share of sessions that viewed one page and did nothing else.
	pub bounce_rate: f64,
	/// The mean time between a session's first and last event.
	pub average_session_seconds: u64,
}

/// One UTC day.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Day {
	/// The day's first millisecond.
	pub day: i64,
	/// Distinct browsers that day.
	pub visitors: u64,
	/// Pages viewed that day.
	pub pageviews: u64,
}

/// A row of a top list.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Count {
	/// What was counted: a path, a source, an event name.
	pub key: String,
	/// How many times.
	pub count: u64,
	/// By how many distinct visitors.
	pub visitors: u64,
}

impl Report {
	/// Count `events` over `period`, comparing with the period before.
	/// Events outside both are ignored.
	pub fn build(events: &[Event], period: Period, truncated: bool) -> Self {
		let current: Vec<&Event> = events
			.iter()
			.filter(|event| period.contains(event.at))
			.collect();
		let previous: Vec<&Event> = events
			.iter()
			.filter(|event| period.before().contains(event.at))
			.collect();
		let browser = || {
			current
				.iter()
				.copied()
				.filter(|event| event.device != Device::Server)
		};
		let pageviews = || browser().filter(|event| event.name == PAGEVIEW);

		Self {
			from: period.from,
			to: period.to,
			summary: summarize(&current),
			previous: summarize(&previous),
			days: days(period, &current),
			pages: tally(pageviews().map(|event| (event.path.clone().unwrap_or_default(), event))),
			sources: sources(browser()),
			devices: tally(pageviews().map(|event| (event.device.as_str().to_owned(), event))),
			browsers: tally(
				pageviews().map(|event| (event.browser.clone().unwrap_or_default(), event)),
			),
			site_events: tally(
				browser()
					.filter(|event| event.name != PAGEVIEW)
					.map(|event| (event.name.clone(), event)),
			),
			server_events: tally(
				current
					.iter()
					.filter(|event| event.device == Device::Server)
					.map(|event| (event.name.clone(), *event)),
			),
			truncated,
		}
	}
}

/// A session's first and last moments, its source, and what happened in it.
struct Session<'a> {
	first: &'a Event,
	last_at: i64,
	pageviews: u64,
	events: u64,
}

fn sessions<'a>(events: impl Iterator<Item = &'a Event>) -> Vec<Session<'a>> {
	let mut sessions: HashMap<&str, Session<'a>> = HashMap::new();

	for event in events {
		let Some(id) = event.session.as_deref() else {
			continue;
		};
		let session = sessions.entry(id).or_insert(Session {
			first: event,
			last_at: event.at,
			pageviews: 0,
			events: 0,
		});

		if event.at < session.first.at {
			session.first = event;
		}

		session.last_at = session.last_at.max(event.at);
		session.events += 1;
		session.pageviews += u64::from(event.name == PAGEVIEW);
	}

	sessions.into_values().collect()
}

fn summarize(events: &[&Event]) -> Summary {
	let browser = || {
		events
			.iter()
			.copied()
			.filter(|event| event.device != Device::Server)
	};
	let sessions = sessions(browser());
	let count = sessions.len() as u64;
	let bounced = sessions
		.iter()
		.filter(|session| session.pageviews == 1 && session.events == 1)
		.count() as u64;
	let seconds: i64 = sessions
		.iter()
		.map(|session| (session.last_at - session.first.at) / 1000)
		.sum();

	Summary {
		visitors: distinct(browser()),
		sessions: count,
		pageviews: browser().filter(|event| event.name == PAGEVIEW).count() as u64,
		bounce_rate: if count == 0 {
			0.0
		} else {
			bounced as f64 / count as f64
		},
		average_session_seconds: u64::try_from(seconds)
			.unwrap_or(0)
			.checked_div(count)
			.unwrap_or(0),
	}
}

fn days(period: Period, events: &[&Event]) -> Vec<Day> {
	(period.from..period.to)
		.step_by(DAY_MILLIS as usize)
		.map(|day| {
			let today = Period {
				from: day,
				to: day + DAY_MILLIS,
			};
			let browser = || {
				events
					.iter()
					.copied()
					.filter(move |event| event.device != Device::Server && today.contains(event.at))
			};
			Day {
				day,
				visitors: distinct(browser()),
				pageviews: browser().filter(|event| event.name == PAGEVIEW).count() as u64,
			}
		})
		.collect()
}

/// Sessions per source, by each session's first event.
fn sources<'a>(events: impl Iterator<Item = &'a Event>) -> Vec<Count> {
	tally(sessions(events).into_iter().map(|session| {
		let source = session
			.first
			.source
			.clone()
			.unwrap_or_else(|| DIRECT.to_owned());
		(source, session.first)
	}))
}

fn distinct<'a>(events: impl Iterator<Item = &'a Event>) -> u64 {
	events
		.map(|event| event.visitor.as_str())
		.collect::<HashSet<_>>()
		.len() as u64
}

/// Count rows by key, with distinct visitors, most frequent first.
fn tally<'a>(rows: impl Iterator<Item = (String, &'a Event)>) -> Vec<Count> {
	let mut counts: HashMap<String, (u64, HashSet<&str>)> = HashMap::new();

	for (key, event) in rows {
		let (count, visitors) = counts.entry(key).or_default();
		*count += 1;
		visitors.insert(&event.visitor);
	}

	let mut rows: Vec<Count> = counts
		.into_iter()
		.map(|(key, (count, visitors))| {
			Count {
				key,
				count,
				visitors: visitors.len() as u64,
			}
		})
		.collect();
	rows.sort_by(|left, right| {
		right
			.count
			.cmp(&left.count)
			.then_with(|| left.key.cmp(&right.key))
	});
	rows.truncate(TOP);
	rows
}

#[cfg(test)]
mod tests {
	use serde_json::Map;

	use super::*;

	/// Noon on 2026-01-10, UTC.
	const NOW: i64 = 1_768_046_400_000;
	const MINUTE: i64 = 60_000;

	fn event(name: &str, visitor: &str, session: &str, at: i64, path: &str) -> Event {
		Event {
			uuid: format!("{visitor}-{session}-{at}-{name}"),
			name: name.to_owned(),
			visitor: visitor.to_owned(),
			session: Some(session.to_owned()),
			at,
			path: Some(path.to_owned()),
			source: None,
			device: Device::Desktop,
			browser: Some("Firefox".to_owned()),
			os: Some("Linux".to_owned()),
			properties: Map::new(),
		}
	}

	fn server(name: &str, project: &str, at: i64) -> Event {
		Event {
			session: None,
			path: None,
			device: Device::Server,
			browser: None,
			os: None,
			..event(name, project, "", at, "/")
		}
	}

	fn fixture() -> Vec<Event> {
		let mut arrived = event(PAGEVIEW, "alice", "a1", NOW - 10 * MINUTE, "/");
		arrived.source = Some("news.ycombinator.com".to_owned());
		vec![
			arrived,
			event(PAGEVIEW, "alice", "a1", NOW - 8 * MINUTE, "/pricing"),
			event("plan chosen", "alice", "a1", NOW - 6 * MINUTE, "/pricing"),
			// Bob looks at one page and leaves.
			event(PAGEVIEW, "bob", "b1", NOW - DAY_MILLIS, "/"),
			server("project created", "project:p1", NOW - MINUTE),
			// Last period: one visit.
			event(PAGEVIEW, "carol", "c1", NOW - 10 * DAY_MILLIS, "/docs"),
		]
	}

	#[test]
	fn periods_are_whole_utc_days() {
		let week = Period::last_days(7, NOW);
		assert_eq!(week.to % DAY_MILLIS, 0);
		assert_eq!(week.to - week.from, 7 * DAY_MILLIS);
		assert!(week.contains(NOW));
		assert_eq!(week.before().to, week.from);
		assert_eq!(week.before().to - week.before().from, 7 * DAY_MILLIS);
	}

	#[test]
	fn summaries_count_visitors_sessions_and_bounces() {
		let report = Report::build(&fixture(), Period::last_days(7, NOW), false);

		assert_eq!(
			report.summary,
			Summary {
				visitors: 2,
				sessions: 2,
				pageviews: 3,
				bounce_rate: 0.5,
				// Alice stayed four minutes, Bob no time at all.
				average_session_seconds: 120,
			}
		);
		assert_eq!(report.previous.visitors, 1);
		assert_eq!(report.previous.pageviews, 1);
	}

	#[test]
	fn days_cover_the_whole_period() {
		let report = Report::build(&fixture(), Period::last_days(7, NOW), false);

		assert_eq!(report.days.len(), 7);
		let today = report.days.last().expect("today");
		assert_eq!((today.visitors, today.pageviews), (1, 2));
		let yesterday = &report.days[5];
		assert_eq!((yesterday.visitors, yesterday.pageviews), (1, 1));
		assert_eq!(report.days[0].pageviews, 0);
	}

	#[test]
	fn top_lists_rank_and_keep_server_events_apart() {
		let report = Report::build(&fixture(), Period::last_days(7, NOW), true);

		assert_eq!(report.pages[0].key, "/");
		assert_eq!((report.pages[0].count, report.pages[0].visitors), (2, 2));
		assert_eq!(
			report
				.sources
				.iter()
				.map(|row| row.key.as_str())
				.collect::<Vec<_>>(),
			["direct", "news.ycombinator.com"]
		);
		assert_eq!(report.devices[0].key, "desktop");
		assert_eq!(report.site_events[0].key, "plan chosen");
		assert_eq!(report.server_events[0].key, "project created");
		assert_eq!(report.summary.visitors, 2, "server events are not visitors");
		assert!(report.truncated);
	}
}
