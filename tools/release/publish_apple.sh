#!/usr/bin/env bash
# Publish already-built Apple artifacts; run only in the protected publisher job.
set -euo pipefail
: "${GH_TOKEN:?GitHub release access is required}"
: "${SWIFTPM_GITHUB_TOKEN:?A token scoped to the SwiftPM repository is required}"
artifacts="${1:?Path to the staged Apple package}"
version="$(python3 -c 'import json,sys; print(json.load(open(sys.argv[1]))["version"])' "$artifacts/swift-package/.canvist-release.json")"
source_sha="$(python3 -c 'import json,sys; print(json.load(open(sys.argv[1]))["source_sha"])' "$artifacts/swift-package/.canvist-release.json")"
tag="v$version"
asset_tag="apple/$tag"
scratch="$(mktemp -d)"
trap 'rm -rf "$scratch"' EXIT

# Binary assets have their own public release so SwiftPM can
# download them before the overall SDK release leaves draft status.
if gh release view "$asset_tag" --repo canvist/canvist >/dev/null 2>&1; then
	published_sha="$(gh api "repos/canvist/canvist/commits/$asset_tag" --jq .sha)"
	[ "$published_sha" = "$source_sha" ] || {
		echo "Apple release already points at another source commit" >&2
		exit 1
	}
else
	gh release create "$asset_tag" --repo canvist/canvist --target "$source_sha" \
		--title "Canvist Apple $version" --notes "Apple binaries built from canvist/canvist@$source_sha." --latest=false
fi
# Never replace an existing binary: tagged Swift packages pin its checksum.
for name in CanvistFFI.xcframework.zip CanvistFFI.xcframework.zip.checksum; do
	file="$artifacts/assets/$name"
	if gh release download "$asset_tag" --repo canvist/canvist --pattern "$name" --dir "$scratch" >/dev/null 2>&1; then
		cmp "$file" "$scratch/$name" || {
			echo "Existing Apple artifact differs: $name" >&2
			exit 1
		}
	else
		gh release upload "$asset_tag" "$file" --repo canvist/canvist
	fi
done

# A fine-grained token is scoped only to canvist/canvist-swift contents.
# Askpass keeps it out of clone URLs, git config, command arguments and logs.
cat >"$scratch/askpass" <<'ASKPASS'
#!/usr/bin/env bash
case "$1" in
*Username*) printf '%s\n' x-access-token ;;
*Password*) printf '%s\n' "$SWIFTPM_GITHUB_TOKEN" ;;
esac
ASKPASS
chmod 700 "$scratch/askpass"
export GIT_ASKPASS="$scratch/askpass"
export GIT_TERMINAL_PROMPT=0
git clone https://github.com/canvist/canvist-swift.git "$scratch/repository"
cd "$scratch/repository"
git config user.name 'github-actions[bot]'
git config user.email '41898282+github-actions[bot]@users.noreply.github.com'
if git rev-parse --verify "refs/tags/$tag" >/dev/null 2>&1; then
	git checkout --detach "$tag"
	cmp .canvist-release.json "$artifacts/swift-package/.canvist-release.json" || {
		echo "SwiftPM tag already names different artifacts" >&2
		exit 1
	}
else
	rm -rf Sources
	cp -R "$artifacts/swift-package/." .
	git add Package.swift Sources license README.md .canvist-release.json
	git commit -m "Release Canvist $version from $source_sha"
	git tag "$tag"
	git push --atomic origin HEAD:main "refs/tags/$tag"
fi
