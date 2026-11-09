#!/usr/bin/env bash
# Start the local Postgres that canvist Cloud's integration tests use.
# The URL matches CI: postgres://canvist:canvist@localhost:5432/canvist_test
set -euo pipefail

if ! command -v pg_isready >/dev/null 2>&1; then
	echo "postgresql is not installed; cloud tests will use the in-memory store" >&2
	exit 0
fi

if ! pg_isready -q; then
	sudo service postgresql start
fi

ready=0
for _ in 1 2 3 4 5 6 7 8 9 10 11 12 13 14 15 16 17 18 19 20 21 22 23 24 25 26 27 28 29 30; do
	if pg_isready -q; then
		ready=1
		break
	fi
	sleep 1
done

if [ "$ready" -ne 1 ]; then
	echo "postgresql did not become ready" >&2
	exit 1
fi

sudo -u postgres psql -v ON_ERROR_STOP=1 <<'SQL'
DO $$
BEGIN
	IF NOT EXISTS (SELECT FROM pg_roles WHERE rolname = 'canvist') THEN
		CREATE ROLE canvist LOGIN PASSWORD 'canvist' SUPERUSER;
	END IF;
END
$$;
SQL

if ! sudo -u postgres psql -tAc "SELECT 1 FROM pg_database WHERE datname = 'canvist_test'" | grep -q 1; then
	sudo -u postgres psql -v ON_ERROR_STOP=1 -c "CREATE DATABASE canvist_test OWNER canvist"
fi

echo "postgres ready at postgres://canvist:canvist@localhost:5432/canvist_test"
