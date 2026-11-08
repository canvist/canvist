#!/usr/bin/env bash
# Reserve the Dart extension packages' names on pub.dev, a few at a time,
# and switch each one on for releases once it can publish from CI.
#
# pub.dev lets one account create 4 packages every 4 hours and 12 every 24
# hours, so the 42 packages in `platforms/dart_extensions` arrive over a few
# days. Until a package is switched on, its pubspec keeps `publish_to: none`
# and the `pub` job in `publish.yml` skips it, so releases go out meanwhile.
#
# For each package that still has `publish_to: none`, this script:
#
# 1. publishes a placeholder 0.0.0 if pub.dev has no package of that name,
#    stopping after LIMIT new packages (default 4) or at the first refusal;
# 2. turns on automated publishing from GitHub Actions (canvist/canvist,
#    `publish.yml`, tags `v{{version}}`, environment `publisher`), which
#    needs the uploader's credentials from `dart pub login`;
# 3. removes `publish_to: none`, so the next release publishes it.
#
# Commit the pubspecs it changes. Run it again after the rate limit
# resets until it reports nothing left.
#
#   bash tools/release/reserve_dart_extensions.sh [LIMIT]
set -euo pipefail

cd "$(dirname "$0")/../.."
limit=${1:-4}
created=0
credentials="${PUB_CACHE:-$HOME/.pub-cache}/credentials.json"
[ -f "$credentials" ] || credentials="$HOME/Library/Application Support/dart/pub-credentials.json"
[ -f "$credentials" ] || credentials="${XDG_CONFIG_HOME:-$HOME/.config}/dart/pub-credentials.json"

exists() {
	[ "$(curl --silent --output /dev/null --write-out '%{http_code}' "https://pub.dev/api/packages/$1")" = 200 ]
}

placeholder() {
	local name=$1 description=$2 directory
	directory=$(mktemp -d)
	mkdir "$directory/lib"
	cat >"$directory/pubspec.yaml" <<-EOF
		name: $name
		description: $description
		version: 0.0.0
		repository: https://github.com/canvist/canvist
		environment:
		  sdk: ^3.13.0
	EOF
	printf '/// Reserved for canvist. See https://github.com/canvist/canvist.\nlibrary;\n' >"$directory/lib/$name.dart"
	printf '# %s\n\nReserved for [canvist](https://github.com/canvist/canvist).\n' "$name" >"$directory/README.md"
	printf '## 0.0.0\n\n- Reserve the name.\n' >"$directory/CHANGELOG.md"
	cp license "$directory/LICENSE"
	(cd "$directory" && dart pub publish --force)
}

# Turn on publishing from `publish.yml` for tags like `v1.2.3`, the way the
# pub.dev admin page does. Returns non-zero when pub.dev refuses.
automate() {
	local name=$1 token
	token=$(jq -r '.accessToken // empty' "$credentials" 2>/dev/null || true)
	[ -n "$token" ] || return 1
	curl --silent --fail --request PUT "https://pub.dev/api/packages/$name/automated-publishing" \
		--header "Authorization: Bearer $token" \
		--header 'Content-Type: application/json' \
		--data '{"github":{"isEnabled":true,"repository":"canvist/canvist","tagPattern":"v{{version}}","requireEnvironment":true,"environment":"publisher","isPushEventEnabled":true,"isWorkflowDispatchEventEnabled":true}}' \
		>/dev/null
}

left=()
for pubspec in platforms/dart_extensions/*/pubspec.yaml; do
	grep -q '^publish_to: none$' "$pubspec" || continue
	name=$(sed -n 's/^name: //p' "$pubspec")

	if ! exists "$name"; then
		if [ "$created" -ge "$limit" ]; then
			left+=("$name")
			continue
		fi

		description=$(sed -n 's/^description: //p' "$pubspec")
		if ! placeholder "$name" "$description"; then
			echo "pub.dev refused $name; stopping (the rate limit, most likely)." >&2
			left+=("$name")
			break
		fi
		created=$((created + 1))
	fi

	if automate "$name"; then
		sed -i.bak -e '/^# Reserved on pub.dev?/d' -e '/^publish_to: none$/d' "$pubspec" && rm "$pubspec.bak"
		echo "released: $name"
	else
		echo "reserved $name; turn on automated publishing at https://pub.dev/packages/$name/admin (canvist/canvist, tag pattern v{{version}}, environment publisher), then delete publish_to: none from $pubspec" >&2
	fi
done

if [ "${#left[@]}" -gt 0 ]; then
	echo "Not yet reserved (${#left[@]}): ${left[*]}. Run again once the rate limit resets." >&2
fi
