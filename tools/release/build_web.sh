#!/usr/bin/env bash
# Build the web SDK from a checkout, in the same order used for publication.
set -euo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")/../.."

# wasm-pack may compile its matching wasm-bindgen CLI outside this workspace,
# where Cargo.toml's host-tool override does not apply.
if [ "$(uname -s)" = "Darwin" ]; then
	export CARGO_PROFILE_RELEASE_BUILD_OVERRIDE_STRIP=none
fi

# @canvist/core first: every other package builds against it.
packages=(
	canvist-core canvist-headless canvist-collab canvist-ssr canvist-scene
	canvist-web canvist-react canvist-vue canvist-canvas
)

for package in "${packages[@]}"; do
	(
		cd "packages/$package"
		npm ci --no-audit --no-fund
		case "$package" in
		canvist-core | canvist-react | canvist-vue) ;;
		*) npm run build:wasm ;;
		esac
		npm run build
	)
done

node tools/release/check_npm_pack.ts "${packages[@]/#/packages/}"
