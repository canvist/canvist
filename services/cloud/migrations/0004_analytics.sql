-- First-party analytics: page views and clicks from canvist.dev, and the
-- server's own events. See `src/analytics/mod.rs`.

CREATE TABLE analytics_events (
	-- Made by the sender, so a batch sent twice is stored once.
	uuid text PRIMARY KEY,
	name text NOT NULL,
	-- An anonymous browser id, or `project:<id>` for server events.
	visitor text NOT NULL,
	-- NULL for server events.
	session text,
	occurred_at timestamptz NOT NULL,
	path text,
	source text,
	device text NOT NULL,
	browser text,
	os text,
	properties jsonb NOT NULL DEFAULT '{}'
);

CREATE INDEX analytics_events_occurred ON analytics_events (occurred_at);

-- Projects list newest first on the admin dashboard.
CREATE INDEX projects_created ON projects (created_at);
