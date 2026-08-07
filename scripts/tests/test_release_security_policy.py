#!/usr/bin/env python3
"""Regression tests for the repository's release supply-chain policy."""

from __future__ import annotations

import importlib.util
import shutil
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
CHECKER = ROOT / "scripts" / "check_release_security.py"
spec = importlib.util.spec_from_file_location("check_release_security", CHECKER)
assert spec and spec.loader
policy = importlib.util.module_from_spec(spec)
spec.loader.exec_module(policy)


class ReleaseSecurityPolicyTests(unittest.TestCase):
    def test_repository_policy_passes(self) -> None:
        self.assertEqual(policy.issues(), [])

    def test_rejects_mutable_action_tags_and_unreviewed_shas(self) -> None:
        with copied_policy_tree() as root:
            workflow = root / ".github" / "workflows" / "ci.yml"
            workflow.write_text(workflow.read_text().replace("uses: ./.github/workflows/reusable-quality.yml", "uses: actions/checkout@v6"), encoding="utf-8")
            self.assertTrue(any("not pinned to a full SHA" in issue for issue in policy.issues(root)))

    def test_rejects_pr_write_trigger_and_credential_outside_host(self) -> None:
        with copied_policy_tree() as root:
            release = root / ".github" / "workflows" / "release.yml"
            release.write_text("pull_request_target:\n" + release.read_text().replace("jobs:\n", "env:\n  GH_TOKEN: ${{ secrets.GITHUB_TOKEN }}\njobs:\n", 1), encoding="utf-8")
            found = policy.issues(root)
            self.assertTrue(any("pull_request_target" in issue for issue in found))
            self.assertTrue(any("outside host" in issue for issue in found))

    def test_rejects_pipe_installers_and_unpinned_containers(self) -> None:
        with copied_policy_tree() as root:
            script = root / "scripts" / "release_smoke.sh"
            script.write_text(script.read_text() + "\ncurl https://example.invalid/install | sh\n", encoding="utf-8")
            release = root / ".github" / "workflows" / "release.yml"
            release.write_text(release.read_text().replace("jobs:\n", "container: ghcr.io/example/tool:latest\njobs:\n", 1), encoding="utf-8")
            found = policy.issues(root)
            self.assertTrue(any("pipe-to-shell" in issue for issue in found))
            self.assertTrue(any("container" in issue for issue in found))

    def test_rejects_cargo_dist_without_reviewed_checksum_provenance(self) -> None:
        with copied_policy_tree() as root:
            installer = root / "scripts" / "install_cargo_dist.sh"
            installer.write_text(installer.read_text().replace("f7bd986e758d0d47c6995aaf92f26d093635c7cd69581ed9e2451b618ea98098", "0" * 64), encoding="utf-8")
            self.assertTrue(any("version/checksum provenance" in issue for issue in policy.issues(root)))

    def test_rejects_loose_cosign_identity_and_id_token_outside_signing_jobs(self) -> None:
        with copied_policy_tree() as root:
            release = root / ".github" / "workflows" / "release.yml"
            release.write_text(release.read_text().replace("--certificate-identity-regexp \"$SIGSTORE_CERTIFICATE_IDENTITY_REGEX\"", "--certificate-identity-regexp '^https://github.com/jeprecated/brain-brew/'"), encoding="utf-8")
            self.assertTrue(any("exact release workflow tag identity" in issue for issue in policy.issues(root)))
        with copied_policy_tree() as root:
            ci = root / ".github" / "workflows" / "ci.yml"
            ci.write_text(ci.read_text().replace("permissions:\n  contents: read", "permissions:\n  contents: read\n  id-token: write"), encoding="utf-8")
            self.assertTrue(any("id-token: write" in issue for issue in policy.issues(root)))


class copied_policy_tree:
    def __enter__(self) -> Path:
        self.temporary = tempfile.TemporaryDirectory()
        self.root = Path(self.temporary.name)
        shutil.copytree(ROOT / ".github", self.root / ".github")
        shutil.copytree(ROOT / "scripts", self.root / "scripts")
        return self.root

    def __exit__(self, *_: object) -> None:
        self.temporary.cleanup()


if __name__ == "__main__":
    unittest.main()
