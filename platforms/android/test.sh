#!/usr/bin/env bash
# Run the `canvist` library's JVM tests, then the instrumented tests of the
# library and the sample app on an Android device or emulator (see
# `emulator.sh` for how one is chosen).
set -euo pipefail

android_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
"$android_dir/build.sh"
# The JVM tests drive the Rust core built for this machine.
(cd "$android_dir/../.." && cargo build --locked --package canvist_ffi --lib)

# Gradle needs JDK 17+; default to the one bundled with Android Studio.
studio_jdk="/Applications/Android Studio.app/Contents/jbr/Contents/Home"
if [ -z "${JAVA_HOME:-}" ] && [ -d "$studio_jdk" ]; then
	export JAVA_HOME="$studio_jdk"
fi

cd "$android_dir"
./gradlew :canvist:testDebugUnitTest --console=plain
"$android_dir/emulator.sh" ./gradlew connectedDebugAndroidTest --console=plain \
	-Pandroid.testInstrumentationRunnerArguments.perfSlack="${CANVIST_PERF_SLACK:-1}"
