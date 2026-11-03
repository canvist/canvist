#!/usr/bin/env bash
# Build mobile wrappers after the headless package's types are available.
set -euo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")/../.."
for package in canvist-react-native canvist-lynx; do
	(cd "packages/$package" && npm ci --no-audit --no-fund && npm run build)
done
node tools/release/check_npm_pack.ts packages/canvist-react-native packages/canvist-lynx
