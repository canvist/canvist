#!/usr/bin/env bash
# Readability and complexity gate. `devenv`'s `lint:style` and CI both call this.
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$root"

check() {
	monostyle check "$1" --fail-under 90
}

check crates/canvist
check crates/canvist_model
check crates/canvist_layout
check crates/canvist_core
check crates/canvist_extensions
check crates/extensions
check crates/canvist_collab
check crates/canvist_scene
check crates/canvist_canvas
check crates/canvist_canvas_wasm
check crates/canvist_paint
check crates/canvist_headless
check crates/canvist_headless_wasm
check packages/canvist-core
check packages/canvist-headless
check packages/canvist-collab
check packages/canvist-ssr
check packages/canvist-scene
check crates/canvist_web
check crates/canvist_desktop
check examples/desktop
check crates/canvist_ffi
check platforms/apple
check platforms/android
check packages/canvist-web
check packages/canvist-react
check packages/canvist-vue
check packages/canvist-canvas
check platforms/dart
check platforms/flutter
check services/cloud
check crates/canvist_dictation
check services/dictation
check tools/footage
check tools/bench
