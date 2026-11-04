"""Prepare installable SwiftPM and CocoaPods artifacts from an Apple SDK build."""
import argparse
import hashlib
import json
import re
import shutil
import zipfile
from pathlib import Path


def archive(source: Path, destination: Path) -> str:
    """Stable ZIP layout, with no enclosing directory and no host timestamps."""
    with zipfile.ZipFile(destination, "w", zipfile.ZIP_DEFLATED) as result:
        for path in sorted(source.rglob("*")):
            if path.is_file():
                info = zipfile.ZipInfo(path.relative_to(source).as_posix())
                info.external_attr = 0o100644 << 16
                info.compress_type = zipfile.ZIP_DEFLATED
                result.writestr(info, path.read_bytes())
    return hashlib.sha256(destination.read_bytes()).hexdigest()


def package(root: Path, output: Path, version: str, sha: str) -> None:
    if not re.fullmatch(r"\d+\.\d+\.\d+(?:-[0-9A-Za-z.-]+)?", version):
        raise ValueError("Expected an SDK version, not a ref or path")
    if not re.fullmatch(r"[0-9a-f]{40}", sha):
        raise ValueError("Expected a full source commit SHA")
    apple = root / "platforms/apple"
    if not (apple / "Sources/Canvist/Canvist.swift").is_file():
        raise ValueError("Run platforms/apple/build.sh to generate Swift bindings first")
    if not (apple / "Frameworks/CanvistFFI.xcframework/Info.plist").is_file():
        raise ValueError("Run platforms/apple/build.sh to build the XCFramework first")
    # Caller supplies a fresh directory so stale files cannot enter a release.
    output.mkdir(parents=True, exist_ok=False)
    assets = output / "assets"
    assets.mkdir()
    url = f"https://github.com/canvist/canvist/releases/download/apple%2Fv{version}"
    checksum = archive(apple / "Frameworks", assets / "CanvistFFI.xcframework.zip")
    (assets / "CanvistFFI.xcframework.zip.checksum").write_text(checksum + "\n")

    swift = output / "swift-package"
    shutil.copytree(apple / "Sources", swift / "Sources")
    shutil.copyfile(root / "license", swift / "license")
    (swift / "Package.swift").write_text(f'''// swift-tools-version: 6.0
import PackageDescription

let package = Package(
  name: "Canvist",
  platforms: [.iOS(.v17)],
  products: [.library(name: "Canvist", targets: ["Canvist"])],
  targets: [
    .binaryTarget(
      name: "CanvistFFI",
      url: "{url}/CanvistFFI.xcframework.zip",
      checksum: "{checksum}"
    ),
    .target(name: "Canvist", dependencies: ["CanvistFFI"]),
  ]
)
''')
    (swift / ".canvist-release.json").write_text(json.dumps({
        "version": version, "source_sha": sha, "binary_sha256": checksum,
    }, indent=2) + "\n")
    (swift / "README.md").write_text(f'''# Canvist for Apple platforms

Generated from canvist/canvist at {sha}. Requires iOS 17 and Swift 6.

```swift
.package(url: "https://github.com/canvist/canvist-swift.git", exact: "{version}")
```

Add the `Canvist` product to your app target. This package includes the Swift
bindings and downloads a checksum-verified XCFramework (iOS and arm64 simulator).
''')

    pod = output / "cocoapods"
    shutil.copytree(apple / "Sources", pod / "Sources")
    shutil.copytree(apple / "Frameworks", pod / "Frameworks")
    shutil.copyfile(root / "license", pod / "license")
    pod_checksum = archive(pod, assets / f"Canvist-{version}.zip")
    (assets / "Canvist.podspec").write_text(f'''Pod::Spec.new do |s|
  s.name = "Canvist"
  s.version = "{version}"
  s.summary = "Native rich-text editor with a Rust core."
  s.homepage = "https://github.com/canvist/canvist"
  s.license = {{ type: "Unlicense", file: "license" }}
  s.author = {{ "Ifiok Jr." => "support@canvist.dev" }}
  s.source = {{ http: "{url}/Canvist-{version}.zip", sha256: "{pod_checksum}" }}
  s.platforms = {{ ios: "17.0" }}
  s.swift_version = "6.0"
  s.source_files = "Sources/Canvist/**/*.swift"
  s.vendored_frameworks = "Frameworks/CanvistFFI.xcframework"
  s.frameworks = "UIKit", "CoreText", "UniformTypeIdentifiers"
  s.pod_target_xcconfig = {{ "EXCLUDED_ARCHS[sdk=iphonesimulator*]" => "x86_64" }}
  s.user_target_xcconfig = {{ "EXCLUDED_ARCHS[sdk=iphonesimulator*]" => "x86_64" }}
end
''')
    shutil.copyfile(assets / "Canvist.podspec", pod / "Canvist.podspec")


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", type=Path, default=Path(__file__).resolve().parents[2])
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--version", required=True)
    parser.add_argument("--source-sha", required=True)
    args = parser.parse_args()
    package(args.root, args.output, args.version, args.source_sha)
