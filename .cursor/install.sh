#!/usr/bin/env bash
# Linux setup for Cursor cloud agents.
#
# Nix is installed single-user (--no-daemon). Cursor cloud VMs use tini as
# init, not systemd, so a multi-user nix-daemon would not stay running after
# a snapshot. Single-user Nix reads the store directly and needs no daemon.
#
# devenv then provides the same toolchain CI uses. iOS, Android, and the
# macOS-only checks still need those hosts.
set -euo pipefail

export DEBIAN_FRONTEND=noninteractive

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$root"

sudo apt-get update
sudo apt-get install -y --no-install-recommends \
	ca-certificates \
	curl \
	xz-utils \
	postgresql \
	postgresql-contrib

nix_conf_ensure() {
	local file="$1"
	local use_sudo="${2:-}"
	local line
	local tmp
	tmp="$(mktemp)"
	if [ -f "$file" ]; then
		if [ "$use_sudo" = sudo ]; then
			# shellcheck disable=SC2024
			sudo cat "$file" >"$tmp"
		else
			cat "$file" >"$tmp"
		fi
	fi
	for line in \
		"experimental-features = nix-command flakes" \
		"accept-flake-config = true" \
		"ssl-cert-file = /etc/ssl/certs/ca-certificates.crt" \
		"extra-substituters = https://devenv.cachix.org" \
		"extra-trusted-public-keys = devenv.cachix.org-1:w1cLUi8dv3hnoSPGAuibQv+f9TZLr6cv/Hm9XgU50cw="; do
		if ! grep -qxF "$line" "$tmp"; then
			printf '%s\n' "$line" >>"$tmp"
		fi
	done
	if [ "$use_sudo" = sudo ]; then
		sudo mkdir -p "$(dirname "$file")"
		sudo install -m 644 "$tmp" "$file"
	else
		mkdir -p "$(dirname "$file")"
		install -m 644 "$tmp" "$file"
	fi
	rm -f "$tmp"
}

link_nix_profile() {
	local bin
	local dir="${HOME}/.nix-profile/bin"
	if [ ! -d "$dir" ]; then
		return 0
	fi
	for bin in "$dir"/*; do
		[ -e "$bin" ] || continue
		sudo ln -sfn "$bin" "/usr/local/bin/$(basename "$bin")"
	done
}

if [ ! -x "${HOME}/.nix-profile/bin/nix" ]; then
	extra="$(mktemp)"
	cat >"$extra" <<'EOF'
experimental-features = nix-command flakes
accept-flake-config = true
ssl-cert-file = /etc/ssl/certs/ca-certificates.crt
extra-substituters = https://devenv.cachix.org
extra-trusted-public-keys = devenv.cachix.org-1:w1cLUi8dv3hnoSPGAuibQv+f9TZLr6cv/Hm9XgU50cw=
EOF
	installer="$(mktemp)"
	curl -fsSL https://nixos.org/nix/install -o "$installer"
	sh "$installer" --no-daemon --yes --nix-extra-conf-file "$extra"
	rm -f "$installer" "$extra"
fi

# shellcheck disable=SC1091
. "${HOME}/.nix-profile/etc/profile.d/nix.sh"

nix_conf_ensure "${HOME}/.config/nix/nix.conf"
nix_conf_ensure /etc/nix/nix.conf sudo
link_nix_profile

if ! command -v cachix >/dev/null 2>&1; then
	nix profile install --accept-flake-config nixpkgs#cachix
	link_nix_profile
fi
cachix use devenv

if ! command -v devenv >/dev/null 2>&1; then
	nix profile install --accept-flake-config github:cachix/devenv/latest
	link_nix_profile
fi

# Build the pinned shell once so later agents reuse the Nix store, then
# install the cargo binaries CI installs (`cargo bin --install`).
devenv shell -- true
devenv shell -- install:all
