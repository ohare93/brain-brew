#!/usr/bin/env python3
"""Fast regression coverage for the extracted-crate verifier helpers."""

from __future__ import annotations

import importlib.util
import re
import shutil
import subprocess
import tempfile
import unittest
from pathlib import Path
from unittest import mock

SCRIPT = Path(__file__).resolve().parents[1] / "verify_extracted_crates.py"
spec = importlib.util.spec_from_file_location("verify_extracted_crates", SCRIPT)
assert spec and spec.loader
verify = importlib.util.module_from_spec(spec)
spec.loader.exec_module(verify)


class ExtractedCrateVerificationTests(unittest.TestCase):
    def test_pre_publish_test_command_is_explicitly_offline(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            extracted = Path(temporary)
            manifest = extracted / "example-0.1.0/Cargo.toml"
            manifest.parent.mkdir()
            manifest.write_text("[package]\nname = \"example\"\nversion = \"0.1.0\"\n")
            with mock.patch.object(verify, "run") as run:
                verify.test_extracted("example", "example", "0.1.0", extracted)

            command = run.call_args.args[0]
            self.assertEqual(command[0:2], ["cargo", "test"])
            self.assertIn("--offline", command)

    def test_translation_json_documentation_matches_source_schema(self) -> None:
        root = Path(__file__).resolve().parents[2]
        source = (root / "crates/brain-brew-cli/src/commands/translations.rs").read_text(
            encoding="utf-8"
        )
        match = re.search(
            r"const TRANSLATION_JSON_SCHEMA_VERSION: u32 = (\d+);", source
        )
        self.assertIsNotNone(match)
        documentation = (
            root / "documentation/docs/authoring/translations.md"
        ).read_text(encoding="utf-8")
        self.assertIn(f'"schema_version": {match.group(1)}', documentation)
        for kind in (
            "translation_report",
            "translation_context",
            "translation_summary",
            "stale_resolution",
        ):
            self.assertIn(kind, documentation)

    def test_repository_only_tests_are_declared_outside_package_boundaries(self) -> None:
        root = Path(__file__).resolve().parents[2]
        manifests = {
            "brain-brew-formats": root / "crates/brain-brew-formats/Cargo.toml",
            "brainbrew": root / "crates/brain-brew-cli/Cargo.toml",
        }
        for package, paths in verify.REPOSITORY_ONLY_PACKAGE_FILES.items():
            manifest = manifests[package].read_text(encoding="utf-8")
            for path in paths:
                self.assertIn(f'\"{path}\"', manifest)

    def write_package(self, root: Path, name: str, source: str, dependency: str = "") -> None:
        root.mkdir()
        (root / "src").mkdir()
        (root / "src/lib.rs").write_text(source)
        (root / "README.md").write_text(f"# {name}\n\n[example](https://example.invalid/{name})\n")
        (root / "LICENSE").write_text("Unlicense\n")
        (root / "Cargo.toml").write_text(
            "[package]\n"
            f'name = "{name}"\nversion = "0.1.0"\nedition = "2021"\n'
            'license-file = "LICENSE"\nreadme = "README.md"\n'
            f"{dependency}"
        )

    def package(self, root: Path, name: str) -> Path:
        result = subprocess.run(["cargo", "package", "--allow-dirty", "--no-verify"], cwd=root, text=True, capture_output=True)
        if result.returncode:
            self.fail(f"cargo package failed for {root}: {result.stderr}")
        return root / "target/package" / f"{name}-0.1.0.crate"

    def test_staged_artifact_rejects_a_changed_exact_core_interface(self) -> None:
        """Alpha.1-style API drift fails even when the version requirement is exact."""
        with tempfile.TemporaryDirectory() as temporary:
            temporary = Path(temporary)
            original = temporary / "fixture-core"
            changed = temporary / "fixture-core-changed"
            consumer = temporary / "fixture-formats"
            self.write_package(original, "fixture-core", "pub struct Required;\n")
            self.write_package(changed, "fixture-core", "pub struct Renamed;\n")
            self.write_package(
                consumer,
                "fixture-formats",
                "use fixture_core::Required; pub fn uses_core() { let _ = Required; }\n",
                '[dependencies]\nfixture-core = { path = "../fixture-core", version = "=0.1.0" }\n',
            )
            # Cargo packages against the normalized manifest, so make the original
            # fixture core available as a local registry patch during packaging.
            (consumer / ".cargo").mkdir()
            (consumer / ".cargo/config.toml").write_text(
                '[patch.crates-io]\nfixture-core = { path = "../fixture-core" }\n'
            )
            consumer_archive = self.package(consumer, "fixture-formats")
            changed_archive = self.package(changed, "fixture-core")

            extracted = temporary / "extracted"
            verify.unpack_archive(changed_archive, extracted, "fixture-core", "0.1.0")
            verify.unpack_archive(consumer_archive, extracted, "fixture-formats", "0.1.0")
            registry = temporary / "registry"
            registry.mkdir()
            staged_core = registry / "fixture-core-0.1.0"
            shutil.copytree(extracted / "fixture-core-0.1.0", staged_core)
            verify.directory_checksum_manifest(staged_core, changed_archive)
            package_root = extracted / "fixture-formats-0.1.0"
            verify.write_source_config(package_root, registry)

            result = subprocess.run(
                ["cargo", "build", "--offline", "--manifest-path", str(package_root / "Cargo.toml")],
                cwd=package_root,
                text=True,
                capture_output=True,
            )
            self.assertNotEqual(result.returncode, 0)
            self.assertIn("Required", result.stderr)

    def test_archive_material_is_required(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary) / "missing-readme"
            self.write_package(root, "missing-readme", "pub fn value() {}\n")
            archive = self.package(root, "missing-readme")
            # Cargo includes the declared README; remove it from a deliberately bad
            # archive to keep this test focused on the archive inspector.
            import tarfile

            bad = root / "bad.crate"
            with tarfile.open(archive, "r:gz") as source, tarfile.open(bad, "w:gz") as destination:
                for member in source.getmembers():
                    if member.name.endswith("README.md"):
                        continue
                    data = source.extractfile(member) if member.isreg() else None
                    destination.addfile(member, data)
            with self.assertRaisesRegex(verify.VerificationError, "README.md"):
                verify.unpack_archive(bad, Path(temporary) / "out", "missing-readme", "0.1.0")


if __name__ == "__main__":
    unittest.main()
