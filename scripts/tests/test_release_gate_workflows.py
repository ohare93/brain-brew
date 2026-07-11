#!/usr/bin/env python3
"""Static regression tests for fail-closed release-gate wiring.

GitHub Actions expressions are not executable in this test environment, so these
checks deliberately inspect the security-relevant workflow graph and commands.
"""

from __future__ import annotations

import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
WORKFLOWS = ROOT / ".github" / "workflows"


class ReleaseGateWorkflowTests(unittest.TestCase):
    def setUp(self) -> None:
        self.ci = (WORKFLOWS / "ci.yml").read_text(encoding="utf-8")
        self.release = (WORKFLOWS / "release.yml").read_text(encoding="utf-8")
        self.quality = (WORKFLOWS / "reusable-quality.yml").read_text(encoding="utf-8")

    def test_pr_and_release_call_one_immutable_sha_gate(self) -> None:
        for caller in (self.ci, self.release):
            self.assertIn("uses: ./.github/workflows/reusable-quality.yml", caller)
            self.assertIn("target_sha:", caller)
        self.assertIn("workflow_call:", self.quality)
        self.assertIn("target_sha:", self.quality)
        self.assertIn("ref: ${{ inputs.target_sha }}", self.quality)
        self.assertIn("must be a full immutable commit SHA", self.quality)
        self.assertIn("git rev-parse --verify", self.quality)

    def test_release_host_cannot_bypass_a_failed_or_skipped_gate(self) -> None:
        host = self.release.split("  host:\n", 1)[1]
        self.assertIn("- quality-gates", host)
        self.assertIn("needs.quality-gates.outputs.target-sha", host)
        self.assertNotIn("always()", host)
        self.assertNotIn("result == 'skipped'", host)
        self.assertIn("dist host", host)
        self.assertIn("gh release create", host)

    def test_quality_gate_covers_required_independent_evidence(self) -> None:
        for command in (
            "devenv shell ci",
            "supply-chain:check",
            "devenv shell e2e",
            "verify_extracted_crates.py pre-publish",
            "package_artifact_smoke.py",
            ".#checks.x86_64-linux.brainbrew",
            "smoke_release_archive.py",
            "representative-consumer",
        ):
            self.assertIn(command, self.quality)

    def test_supply_chain_and_artifact_metadata_are_required_before_host(self) -> None:
        for command in ("supply-chain:check", "generate_release_metadata.py"):
            self.assertIn(command, self.quality)
        self.assertIn("actions/attest-build-provenance@977bb373ede98d70efdf65b84cb5f73e068dcc2a", self.release)
        self.assertIn("id-token: write", self.release)
        self.assertIn("attestations: write", self.release)
        host = self.release.split("  host:\n", 1)[1]
        self.assertIn("Verify produced checksums, SBOMs, and workflow-bound provenance offline", host)
        self.assertIn("gh release upload", host)

    def test_artifact_smoke_never_installs_from_a_workspace_path(self) -> None:
        package_smoke = (ROOT / "scripts" / "package_artifact_smoke.py").read_text(encoding="utf-8")
        self.assertIn("package_archives", package_smoke)
        self.assertIn("unpack_archive", package_smoke)
        self.assertNotIn("cargo install --path", package_smoke)
        self.assertNotIn("target/debug/brainbrew", package_smoke)

    def test_release_uses_explicit_live_consumer_contract_and_pr_is_safe(self) -> None:
        self.assertIn("require_representative_consumer: false", self.ci)
        self.assertIn("require_representative_consumer: true", self.release)
        self.assertIn("representative_consumer_evidence_url", self.quality)
        self.assertIn("representative_consumer_evidence_sha256", self.quality)
        self.assertIn("blocked", self.quality)
        self.assertIn("ultimate-geography-live", self.quality)

    def test_callers_have_read_only_default_permissions(self) -> None:
        self.assertIn("contents: read", self.ci)
        self.assertIn("contents: read", self.release)
        self.assertIn("  host:\n", self.release)
        self.assertIn("    permissions:\n      contents: write", self.release)


if __name__ == "__main__":
    unittest.main()
