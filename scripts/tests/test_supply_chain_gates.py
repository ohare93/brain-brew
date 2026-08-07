#!/usr/bin/env python3
"""Deterministic fixture tests for supply-chain policy and artifact metadata."""

from __future__ import annotations

import hashlib
import importlib.util
import json
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]


def load(name: str):
    spec = importlib.util.spec_from_file_location(name, ROOT / "scripts" / f"{name}.py")
    assert spec and spec.loader
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


policy = load("check_dependency_policy")
metadata = load("generate_release_metadata")


class DependencyPolicyTests(unittest.TestCase):
    def policy_file(self, root: Path, exceptions: str = "") -> Path:
        path = root / "policy.toml"
        path.write_text(
            "[licenses]\nallowed = [\"MIT\", \"Apache-2.0\"]\n\n"
            "[advisory_exceptions]\n" + exceptions,
            encoding="utf-8",
        )
        return path

    def test_unknown_production_advisory_and_license_fail(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            found = policy.issues(
                self.policy_file(root),
                {"vulnerabilities": {"left-pad": {"via": [{"url": "https://github.com/advisories/GHSA-bad"}]}}},
                {"packages": [{"name": "left-pad", "version": "1.0.0", "license": "GPL-3.0-only", "scope": "production"}]},
                today="2026-07-11",
            )
            self.assertTrue(any("untriaged production npm advisory GHSA-bad" in issue for issue in found))
            self.assertTrue(any("unapproved production license GPL-3.0-only" in issue for issue in found))

    def test_expired_exception_fails_and_dev_only_is_reported_not_blocking(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            found = policy.issues(
                self.policy_file(root, '"GHSA-known" = { owner = "release", expires = "2026-07-01", rationale = "tracked" }\n'),
                {"vulnerabilities": {"known": {"via": [{"url": "https://github.com/advisories/GHSA-known"}]}, "dev": {"scope": "dev", "via": [{"url": "https://github.com/advisories/GHSA-dev"}]}}},
                {"packages": [{"name": "dev-tool", "version": "1", "license": "GPL-3.0-only", "scope": "development"}]},
                today="2026-07-11",
            )
            self.assertTrue(any("expired advisory exception GHSA-known" in issue for issue in found))
            self.assertFalse(any("GHSA-dev" in issue for issue in found))
            self.assertFalse(any("GPL-3.0-only" in issue for issue in found))


class ArtifactMetadataTests(unittest.TestCase):
    def generated(self, root: Path) -> tuple[Path, Path]:
        artifact = root / "brainbrew.tar.gz"
        artifact.write_bytes(b"produced artifact")
        output = root / "metadata"
        metadata.generate([artifact], output, source_sha="a" * 40, workflow_run="123")
        return artifact, output

    def test_tampered_checksum_sbom_and_provenance_fail(self) -> None:
        for name in ("checksum", "sbom", "provenance"):
            with self.subTest(name=name), tempfile.TemporaryDirectory() as temporary:
                root = Path(temporary)
                artifact, output = self.generated(root)
                target = next(output.glob(f"*.{name if name != 'checksum' else 'sha256'}*")) if name != "checksum" else output / "SHA256SUMS"
                target.write_text("tampered\n", encoding="utf-8")
                found = metadata.verify([artifact], output, source_sha="a" * 40, workflow_run="123")
                self.assertTrue(found)

    def test_wrong_source_or_workflow_binding_fails(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            artifact, output = self.generated(Path(temporary))
            self.assertTrue(metadata.verify([artifact], output, source_sha="b" * 40, workflow_run="123"))
            self.assertTrue(metadata.verify([artifact], output, source_sha="a" * 40, workflow_run="999"))


if __name__ == "__main__":
    unittest.main()
