# Native publishing

Every SDK release includes `@canvist/react-native`, `@canvist/lynx`, `canvist_desktop`, `canvist_dictation` and `canvist_dictation_relay`. Their placeholders must have the same npm/crates.io trusted publisher configuration as the rest of the SDK: repository `canvist/canvist`, workflow `publish.yml`, environment `publisher`.

## Apple distribution

The source checkout keeps its local `platforms/apple/Package.swift` and podspec for development. `tools/release/package_apple.py` creates release-specific consumer packages after `platforms/apple/build.sh` generates the bindings and XCFramework:

- A SwiftPM package containing **all** Swift sources, including the generated UniFFI bindings, plus a binary target pinned to its ZIP's SHA-256.

The public distribution repository is [canvist/canvist-swift](https://github.com/canvist/canvist-swift). On real releases, `tools/release/publish_apple.sh` publishes binaries in a public `apple/v<version>` release in the main repository, commits the SwiftPM sources/manifest to the distribution repository, tags that commit `v<version>`. The separate Apple asset release makes download URLs available to SwiftPM while the overall SDK release remains a draft. Existing binaries and SwiftPM tags cannot be overwritten with different artifacts.

Consumer installation after publication:

```swift
.package(url: "https://github.com/canvist/canvist-swift.git", exact: "0.2.0")
```

The supported platforms remain iOS 17+ on device and arm64 simulators. CocoaPods publication is discontinued: releases do not generate pod archives, upload podspecs or submit to Trunk. Local development podspecs remain in the source checkout. React Native and Lynx's current podspecs reference `Canvist`; consumers must supply that dependency locally until those integrations migrate to a distribution that does not depend on Trunk.

The `publisher` environment needs:

| Secret                 | Purpose                                                                                                                                                                                                                                                                         |
| ---------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `SWIFTPM_GITHUB_TOKEN` | Fine-grained GitHub token granting **Contents: read and write** to **only** `canvist/canvist-swift`. The organization's policy rejects deploy keys, so the workflow uses a scoped token through Git askpass. Never use a broad personal token or embed credentials in git URLs. |

## Maven Central

The artifact coordinate is `io.github.ifiokjr:canvist-android`. Confirm the `io.github.ifiokjr` namespace in the [Central Portal](https://central.sonatype.com/publishing/namespaces). A GitHub-based namespace must be verified with the owning GitHub account.

Generate a Central Portal user token and configure these `publisher` environment secrets:

| Secret                       | Value                                                                                                             |
| ---------------------------- | ----------------------------------------------------------------------------------------------------------------- |
| `MAVEN_CENTRAL_USERNAME`     | User-token username from the Portal, not the GitHub login.                                                        |
| `MAVEN_CENTRAL_PASSWORD`     | User-token password from the same generated token.                                                                |
| `MAVEN_SIGNING_KEY`          | ASCII-armored OpenPGP secret key. Publish its public key to a Maven Central-supported keyserver before releasing. |
| `MAVEN_SIGNING_KEY_PASSWORD` | Passphrase for that signing key.                                                                                  |

A dedicated RSA-4096 signing key has been generated and its encrypted secret key and passphrase are stored in the protected GitHub environment. The public key is checked in as [maven-signing-key.asc](maven-signing-key.asc), fingerprint `37EE0A86DAB3CDA38D147FA94DC451D073A0E983`, expiring 2028-11-02. The Ubuntu keyserver accepted its upload, and independent retrieval into a fresh keyring succeeded.

The Gradle Maven Publish plugin creates the POM, sources and documentation artifacts. CI builds the Rust native libraries first. Dry runs publish to Maven Local; real releases always run `publishAndReleaseToMavenCentral`. There is no enable flag that silently falls back to Maven Local.

## Preflight and validation

Before **any** SDK package is published, a protected preflight job requires all five native secrets. Dry runs require no secrets and perform no native publication. Missing credentials fail the release rather than allowing npm/crates.io to publish first.

Local checks:

```sh
bash tools/release/build_native_js.sh
python3 -m unittest discover -s tools/release -p 'package_apple_test.py' -v
deno test --allow-read --allow-write --allow-run=git,bash tools/release/publish_provenance_test.ts
```

The mobile npm build needs `@canvist/headless` built first (`bash tools/release/build_web.sh`). macOS CI additionally builds the actual XCFramework, parses the generated SwiftPM manifest, checks its binary checksum with SwiftPM. The local Xcode installation on mini03 currently cannot load CoreSimulator; fixture tests are not a replacement for that full CI build.

Account-side setup is complete only after Central namespace ownership, public signing-key retrieval, the scoped SwiftPM token have been verified. Repository configuration and passing dry runs alone do not prove those external credentials.
