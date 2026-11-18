#!/usr/bin/env bash
# Widget tests for the Flutter package (`flutter test`).
set -euo pipefail

cd "$(dirname "${BASH_SOURCE[0]}")"
if command -v fvm >/dev/null; then
	exec fvm flutter test
else
	exec flutter test
fi
