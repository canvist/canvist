"""Verify consumer archive layouts, checksums and generated SwiftPM manifests."""
import hashlib
import json
import tempfile
import unittest
import zipfile
from pathlib import Path

from package_apple import archive, package


class ApplePackageTest(unittest.TestCase):
    def test_consumer_packages_contain_generated_bindings_and_pinned_binaries(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            apple = root / "platforms/apple"
            sources = apple / "Sources/Canvist"
            sources.mkdir(parents=True)
            (sources / "Canvist.swift").write_text("// Generated Swift bindings\n")
            (sources / "CanvistView.swift").write_text("// Native view\n")
            framework = apple / "Frameworks/CanvistFFI.xcframework"
            framework.mkdir(parents=True)
            (framework / "Info.plist").write_text("framework fixture")
            (root / "license").write_text("Unlicense")
            output = root / "output"
            package(root, output, "0.2.0", "a" * 40)
            assets = output / "assets"
            binary = assets / "CanvistFFI.xcframework.zip"
            checksum = hashlib.sha256(binary.read_bytes()).hexdigest()
            manifest = (output / "swift-package/Package.swift").read_text()
            self.assertIn(checksum, manifest)
            self.assertIn("apple%2Fv0.2.0/CanvistFFI.xcframework.zip", manifest)
            metadata = json.loads((output / "swift-package/.canvist-release.json").read_text())
            self.assertEqual(metadata["source_sha"], "a" * 40)
            with zipfile.ZipFile(assets / "Canvist-0.2.0.zip") as source:
                self.assertIn("Sources/Canvist/Canvist.swift", source.namelist())
                self.assertIn("Frameworks/CanvistFFI.xcframework/Info.plist", source.namelist())
                self.assertIn("license", source.namelist())
            pod_hash = hashlib.sha256((assets / "Canvist-0.2.0.zip").read_bytes()).hexdigest()
            self.assertIn(pod_hash, (assets / "Canvist.podspec").read_text())
            self.assertEqual(archive(apple / "Frameworks", root / "again.zip"), checksum)

    def test_missing_generated_bindings_fail_before_creating_output(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            with self.assertRaisesRegex(ValueError, "generate Swift bindings"):
                package(root, root / "output", "0.2.0", "a" * 40)
            self.assertFalse((root / "output").exists())

    def test_invalid_version_is_rejected(self):
        with self.assertRaisesRegex(ValueError, "Expected an SDK version"):
            package(Path("."), Path("output"), "../escape", "a" * 40)


if __name__ == "__main__":
    unittest.main()
