#!/usr/bin/env bash
# Install the host CLIs CI used to take from devenv, pinned to the versions in
# devenv.lock's nixpkgs (4fe25e12eb9f4c744966dd3de5a05246cf098601) and
# ifiokjr/nixpkgs (9e95821c035cd977a2a9781bca5fe313a394f9cd).
#
# Linux x86_64 only: every job that calls this action runs on ubuntu-latest.
set -euo pipefail

if [ "$(uname -s)" != "Linux" ] || [ "$(uname -m)" != "x86_64" ]; then
	echo "setup-tools downloads Linux x86_64 binaries; this runner is $(uname -s) $(uname -m)" >&2
	exit 1
fi

bin="${HOME}/.local/bin"
mkdir -p "$bin"
echo "$bin" >>"${GITHUB_PATH:?}"

verify() {
	python3 - "$1" "$2" <<'PY'
import base64, hashlib, pathlib, sys
file, expected = sys.argv[1], sys.argv[2]
digest = "sha256-" + base64.b64encode(hashlib.sha256(pathlib.Path(file).read_bytes()).digest()).decode()
if digest != expected:
    raise SystemExit(f"checksum mismatch for {file}: {digest} != {expected}")
PY
}

fetch() {
	local url="$1" dest="$2" sum="$3"
	curl -fsSL --retry 3 --retry-delay 2 -o "$dest" "$url"
	verify "$dest" "$sum"
}

install_monochange() {
	local archive="${RUNNER_TEMP}/monochange.tar.gz"
	fetch \
		"https://github.com/monochange/monochange/releases/download/v0.21.0/monochange-x86_64-unknown-linux-musl-v0.21.0.tar.gz" \
		"$archive" \
		"sha256-lrzUNXouiRZ1UQIspeEkMVVGVljyJ/ebKoff8z4GZVM="
	tar -xzf "$archive" -C "$bin" monochange
	chmod +x "$bin/monochange"
	ln -sfn monochange "$bin/mc"
}

install_formatters() {
	if ! command -v rustup >/dev/null 2>&1; then
		echo "install the Rust toolchain before the formatters; dprint calls nightly rustfmt" >&2
		exit 1
	fi
	# Same floating nightly rustfmt the devenv shell installs for dprint.
	rustup toolchain install nightly --profile minimal --component rustfmt --no-self-update

	local archive="${RUNNER_TEMP}/dprint.zip"
	fetch \
		"https://github.com/dprint/dprint/releases/download/0.64.3/dprint-x86_64-unknown-linux-gnu.zip" \
		"$archive" \
		"sha256-9xy34i1ndkOeuhaNXLKWCS1kE/6nFI/k1waEonAqT+4="
	unzip -p "$archive" dprint >"$bin/dprint"

	fetch \
		"https://github.com/mvdan/sh/releases/download/v3.14.1/shfmt_v3.14.1_linux_amd64" \
		"$bin/shfmt" \
		"sha256-dud2QfqgJYFLd/FTspeWuOb6L8oD4MdqaRYIuGx+p78="
	fetch \
		"https://github.com/pinterest/ktlint/releases/download/1.8.0/ktlint" \
		"$bin/ktlint" \
		"sha256-o/1iAgfVxA2myniblef4I8VOhUt/ref2E+kQlqNwbXU="
	fetch \
		"https://github.com/NixOS/nixfmt/releases/download/v1.5.0/nixfmt" \
		"$bin/nixfmt" \
		"sha256-qgKSR6uJyJfi85SEY5Z1U/kC+G6G40AvooyynEpE90U="

	archive="${RUNNER_TEMP}/swift-format.tar.gz"
	fetch \
		"https://github.com/ifiokjr/nixpkgs/releases/download/prebuilt/swift-format/604.0.0/swift-format-x86_64-unknown-linux-gnu.tar.gz" \
		"$archive" \
		"sha256-5xog0rVNbFw9O3AuBlWotba+3pCLhkxSRzPFX0Wsa2c="
	tar -xzf "$archive" -C "$bin" swift-format
	chmod +x "$bin/dprint" "$bin/shfmt" "$bin/ktlint" "$bin/nixfmt" "$bin/swift-format"
}

install_linters() {
	local archive="${RUNNER_TEMP}/shellcheck.tar.xz"
	fetch \
		"https://github.com/koalaman/shellcheck/releases/download/v0.11.0/shellcheck-v0.11.0.linux.x86_64.tar.xz" \
		"$archive" \
		"sha256-jDvhKwXVwXegTCnjx4zomshvFZVoHKsUm2W5fE4icZg="
	tar -xJf "$archive" -C "$RUNNER_TEMP" shellcheck-v0.11.0/shellcheck
	install -m 755 "$RUNNER_TEMP/shellcheck-v0.11.0/shellcheck" "$bin/shellcheck"

	fetch \
		"https://github.com/ifiokjr/monostyle/releases/download/v0.3.28/monostyle-x86_64-unknown-linux-musl" \
		"$bin/monostyle" \
		"sha256-fIY61lQBH6Gk0oDJJ4y/RRd7ANh52WZ9newqxCaBI+Q="
	chmod +x "$bin/monostyle"
}

case "${INSTALL_MONOCHANGE:-false}" in
true) install_monochange ;;
esac
case "${INSTALL_FORMAT:-false}" in
true) install_formatters ;;
esac
case "${INSTALL_LINT:-false}" in
true) install_linters ;;
esac

export PATH="$bin:$PATH"
command -v monochange >/dev/null 2>&1 && monochange --version || true
command -v dprint >/dev/null 2>&1 && dprint --version || true
command -v monostyle >/dev/null 2>&1 && monostyle --version || true
command -v shellcheck >/dev/null 2>&1 && shellcheck --version || true
