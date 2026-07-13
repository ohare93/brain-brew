#!/usr/bin/env python3
"""Fail closed on mutable executable release inputs and credential sprawl.

This is intentionally a narrow repository policy, not a generic workflow linter.
Every permitted external executable source is named below so an addition requires
both review and a documented pin update.
"""

from __future__ import annotations

import json
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
WORKFLOWS = ROOT / ".github" / "workflows"
ACTION_PINS = {
    "actions/checkout": "df4cb1c069e1874edd31b4311f1884172cec0e10",
    "actions/upload-artifact": "b7c566a772e6b6bfb58ed0dc250532a479d7789f",
    "actions/download-artifact": "37930b1c2abaa49bbe596cd826c3c89aef350131",
    "cachix/install-nix-action": "a49548c11d9846ad46ecc0115273879b045f001c",
    "actions/attest-build-provenance": "977bb373ede98d70efdf65b84cb5f73e068dcc2a",
    "sigstore/cosign-installer": "d7543c93d881b35a8faa02e8e3605f69b7a1ce62",
}
ACTION_RE = re.compile(r"^\s*(?:-\s*)?uses:\s+([^\s#]+)(?:\s+(#.*))?$", re.MULTILINE)
SHA_RE = re.compile(r"^[0-9a-f]{40}$")
SIGSTORE_IDENTITY_REGEX = r"^https://github\.com/jeprecated/brain-brew/\.github/workflows/release\.yml@refs/tags/"
ATTEST_ACTION = "actions/attest-build-provenance@977bb373ede98d70efdf65b84cb5f73e068dcc2a"


def issues(root: Path = ROOT) -> list[str]:
    found: list[str] = []
    workflow_sources: dict[Path, str] = {
        path: path.read_text(encoding="utf-8")
        for path in sorted((root / ".github" / "workflows").glob("*.y*ml"))
    }
    if not workflow_sources:
        found.append("no workflow YAML files found")

    for path, source in workflow_sources.items():
        relative = path.relative_to(root)
        for match in ACTION_RE.finditer(source):
            action, comment = match.groups()
            if action.startswith("./"):
                continue
            if "@" not in action:
                found.append(f"{relative}: action lacks an immutable SHA: {action}")
                continue
            name, pin = action.rsplit("@", 1)
            if not SHA_RE.fullmatch(pin):
                found.append(f"{relative}: action is not pinned to a full SHA: {action}")
                continue
            if name not in ACTION_PINS:
                found.append(f"{relative}: action is not in the reviewed pin map: {name}")
            elif pin != ACTION_PINS[name]:
                found.append(f"{relative}: action pin disagrees with the reviewed pin map: {action}")
            if not comment or "documentation/docs/reference/release-security.md" not in comment:
                found.append(f"{relative}: action pin lacks adjacent version/provenance reference: {action}")

        if re.search(r"^\s*pull_request_target\s*:", source, re.MULTILINE):
            found.append(f"{relative}: pull_request_target is forbidden")
        if "permissions: write-all" in source or "permissions: write" in source:
            found.append(f"{relative}: broad workflow permissions are forbidden")
        if "docker://" in source or re.search(r"^\s*container:\s*", source, re.MULTILINE):
            found.append(f"{relative}: container actions/images must be replaced with reviewed runner tooling")
        if re.search(r"@[vV][^\s#]+", source):
            found.append(f"{relative}: mutable action tag found")

    for path in sorted((root / ".github" / "workflows").glob("*.y*ml")):
        source = workflow_sources[path]
        relative = path.relative_to(root)
        if "permissions:\n  contents: read" not in source:
            found.append(f"{relative}: workflow default must be contents: read")

    release = workflow_sources.get(root / ".github" / "workflows" / "release.yml", "")
    host = release.split("  host:\n", 1)[-1]
    non_host = release.split("  host:\n", 1)[0]
    if "  host:\n" not in release or "    permissions:\n      contents: write" not in host:
        found.append("release.yml: only the host job must declare contents: write")
    if "contents: write" in non_host:
        found.append("release.yml: a non-host job has write contents permission")
    for token in ("GITHUB_TOKEN", "GH_TOKEN", "CARGO_REGISTRY_TOKEN"):
        if token in non_host:
            found.append(f"release.yml: publication credential {token} appears outside host")
    if "env:\n      GH_TOKEN:" in host:
        found.append("release.yml: GH_TOKEN must be scoped to its exact host command, not the job")
    if "secrets.GITHUB_TOKEN" not in host:
        found.append("release.yml: host must explicitly scope its GitHub token")

    # Keyless signing is deliberately limited to the two release artifact
    # builders. A repository-wide OIDC token would let an unrelated job mint a
    # trusted signature under this repository identity.
    for path, source in workflow_sources.items():
        relative = path.relative_to(root)
        if "id-token: write" in source and path != root / ".github" / "workflows" / "release.yml":
            found.append(f"{relative}: id-token: write is restricted to release signing jobs")
    signing_jobs = ("build-local-artifacts", "build-global-artifacts")
    if release.count("id-token: write") != len(signing_jobs):
        found.append("release.yml: id-token: write must appear only in the two release signing jobs")
    for job in signing_jobs:
        match = re.search(rf"^  {re.escape(job)}:\n(?P<body>.*?)(?=^  [a-z][a-z-]+:\n|\Z)", release, re.MULTILINE | re.DOTALL)
        if match is None or match.group("body").count("id-token: write") != 1:
            found.append(f"release.yml: {job} must be the sole scoped OIDC signing job")

    expected_env = f"SIGSTORE_CERTIFICATE_IDENTITY_REGEX: '{SIGSTORE_IDENTITY_REGEX}'"
    if expected_env not in release:
        found.append("release.yml: missing exact release workflow tag identity regex")
    for line in release.splitlines():
        if "cosign verify-blob" in line and '--certificate-identity-regexp "$SIGSTORE_CERTIFICATE_IDENTITY_REGEX"' not in line:
            found.append("release.yml: cosign verification must require exact release workflow tag identity")
    if release.count("cosign verify-blob") != 3:
        found.append("release.yml: every generated checksum bundle and host verification must use cosign verification")

    attestation_blocks = re.findall(
        rf"uses: {re.escape(ATTEST_ACTION)}(?P<body>.*?)(?=^      - |\Z)", release, re.MULTILINE | re.DOTALL
    )
    if len(attestation_blocks) != len(signing_jobs):
        found.append("release.yml: every release artifact phase must keylessly attest shipped artifacts")
    for block in attestation_blocks:
        if "subject-path: ${{ steps.cargo-dist.outputs.artifact-subjects }}" not in block or "METADATA_DIR" in block:
            found.append("release.yml: attest-build-provenance must subject actual produced shipped artifacts, not metadata sidecars")
    for required in ("artifact-subjects<<EOF", "refusing non-distribution artifact subject", "*.sha256|*.sha256sum", "test \"$subject_count\" -gt 0"): 
        if release.count(required) != len(signing_jobs):
            found.append(f"release.yml: artifact attestation subject selection lacks required guard {required!r}")

    for lock_name in ("flake.lock", "devenv.lock"): 
        lock_path = root / lock_name
        try:
            nodes = json.loads(lock_path.read_text(encoding="utf-8"))["nodes"]
        except (OSError, ValueError, KeyError):
            found.append(f"{lock_name}: missing or invalid Nix lock file")
            continue
        for name, node in nodes.items():
            if name == "root":
                continue
            locked = node.get("locked", {})
            if not SHA_RE.fullmatch(locked.get("rev", "")) or not str(locked.get("narHash", "")).startswith("sha256-"):
                found.append(f"{lock_name}: {name} lacks an immutable revision and NAR hash")

    installer = root / "scripts" / "install_cargo_dist.sh"
    if not installer.is_file():
        found.append("scripts/install_cargo_dist.sh: missing checksum-verified cargo-dist installer")
    else:
        installer_source = installer.read_text(encoding="utf-8")
        if "v0.30.4" not in installer_source or "f7bd986e758d0d47c6995aaf92f26d093635c7cd69581ed9e2451b618ea98098" not in installer_source:
            found.append("scripts/install_cargo_dist.sh: cargo-dist version/checksum provenance is incomplete")
        if "curl --fail" not in installer_source or "sha256sum" not in installer_source:
            found.append("scripts/install_cargo_dist.sh: installer must download and verify before extraction")

    allowed_shell_downloads = {
        Path("scripts/install_cargo_dist.sh"),
        Path("scripts/run_workbench_e2e.sh"),  # probes its local chromedriver only
    }
    for path in sorted((root / "scripts").rglob("*.sh")):
        source = path.read_text(encoding="utf-8")
        relative = path.relative_to(root)
        if re.search(r"(?:curl|wget)[^\n|]*\|\s*(?:ba)?sh\b", source):
            found.append(f"{relative}: pipe-to-shell installer is forbidden")
        if "sh.rustup.rs" in source:
            found.append(f"{relative}: rustup bootstrap installer is forbidden")
        if re.search(r"\b(?:curl|wget)\b", source) and relative not in allowed_shell_downloads:
            found.append(f"{relative}: shell download is not an explicitly reviewed source")
        if re.search(r"\bcargo\s+install\b", source):
            found.append(f"{relative}: cargo install is forbidden; use a lock or verified asset")
    e2e_source = (root / "scripts" / "run_workbench_e2e.sh").read_text(encoding="utf-8")
    if "http://127.0.0.1:" not in e2e_source:
        found.append("scripts/run_workbench_e2e.sh: curl must remain a localhost readiness probe")

    # The repository's own ``brain-brew`` identity is valid provenance text;
    # standalone package-manager/token paths remain forbidden.
    if "homebrew" in release.lower() or re.search(r"(?<!brain-)\b(?:brew|tap|pat)\b", release.lower()):
        found.append("release.yml: Homebrew/tap/PAT publication path is forbidden")
    return found


def main() -> int:
    found = issues()
    if found:
        print("release security policy violations:", file=sys.stderr)
        print("\n".join(f"- {issue}" for issue in found), file=sys.stderr)
        return 1
    print("release security policy passed")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
