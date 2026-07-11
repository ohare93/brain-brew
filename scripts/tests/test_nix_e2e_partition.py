#!/usr/bin/env python3
"""Regression checks for the deterministic-package/prepared-browser-E2E split."""

from __future__ import annotations

import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]


class NixE2ePartitionTests(unittest.TestCase):
    def setUp(self) -> None:
        self.flake = (ROOT / "flake.nix").read_text(encoding="utf-8")
        self.ci = (ROOT / ".github/workflows/ci.yml").read_text(encoding="utf-8")
        self.release = (ROOT / ".github/workflows/release.yml").read_text(encoding="utf-8")
        self.quality = (ROOT / ".github/workflows/reusable-quality.yml").read_text(encoding="utf-8")

    def test_package_check_owns_every_non_browser_workspace_package_explicitly(self) -> None:
        for package in [
            "brain-brew-core",
            "brain-brew-formats",
            "brainbrew",
            "brain-brew-workbench-ui",
        ]:
            self.assertIn(f'"{package}"', self.flake)
        self.assertNotIn('cargoTestFlags = [\n              "--workspace"', self.flake)

    def test_browser_e2e_is_owned_by_the_prepared_devenv_job(self) -> None:
        self.assertIn("quality-gates", self.ci)
        self.assertIn("uses: ./.github/workflows/reusable-quality.yml", self.ci)
        self.assertIn("uses: ./.github/workflows/reusable-quality.yml", self.release)
        self.assertIn("devenv shell e2e", self.quality)
        self.assertIn("devenv", self.quality)
        self.assertIn("scripts/run_workbench_e2e.sh", (ROOT / "devenv.nix").read_text())
        self.assertIn("cargo build -p brainbrew --features workbench-write-dev", (ROOT / "scripts/run_workbench_e2e.sh").read_text())

    def test_ci_requires_nix_package_and_prepared_browser_gates(self) -> None:
        self.assertIn("quality-gates:", self.ci)
        self.assertIn(".#checks.x86_64-linux.brainbrew", self.quality)
        self.assertNotIn(".#checks.x86_64-linux.workbench-e2e", self.quality)
        self.assertNotIn("if: ${{ false }}", self.quality)

    def test_release_plan_waits_for_the_same_required_gates(self) -> None:
        self.assertIn("quality-gates:", self.release)
        self.assertIn("uses: ./.github/workflows/reusable-quality.yml", self.release)
        self.assertIn(".#checks.x86_64-linux.brainbrew", self.quality)
        self.assertIn("devenv shell e2e", self.quality)
        self.assertIn("- quality-gates", self.release)


if __name__ == "__main__":
    unittest.main()
