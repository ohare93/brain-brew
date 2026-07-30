#!/usr/bin/env python3
"""Validate release-version references against Cargo's workspace version source."""

from __future__ import annotations

import sys
import tomllib
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
PUBLISHABLE = {"brain-brew-core", "brain-brew-formats", "brainbrew"}
INTERNAL = {"brain-brew-core", "brain-brew-formats"}
CURRENT = "1.0.0-alpha.4"
STALE = "1.0.0-alpha.3"
CURRENT_DOCS = [
    Path("README.md"),
    Path("documentation/docs/getting-started/install.md"),
    Path("documentation/docs/reference/releasing.md"),
]
ALLOWED_STALE_REFERENCES = {
    Path("CHANGELOG.md"),
    Path("audit/13-ultimate-geography.md"),
    Path("audit/14-docs.md"),
    Path("audit/15-release-security.md"),
    Path("audit/16-synthesis.md"),
    Path("crates/brain-brew-cli/tests/registry_planner.rs"),
    Path("crates/brain-brew-formats/src/package_semver.rs"),
    Path("crates/brain-brew-formats/tests/ultimate_geography_fixture.rs"),
    Path("documentation/docs/authoring/packages-locking.md"),
    Path("documentation/docs/reference/ultimate-geography-fixture.md"),
    Path("fixtures/ultimate-geography.lock.json"),
    Path("scripts/tests/test_ug_fixture.py"),
    Path("scripts/ug-fixture-sync/README.md"),
    Path("scripts/ug_fixture.py"),
}
IGNORED_DIRECTORIES = {
    ".agentleman",
    ".frontloop",
    ".jj",
    "build",
    "node_modules",
    "target",
}
IGNORED_FILES = {Path("scripts/check_release_version.py")}


def error(errors: list[str], message: str) -> None:
    errors.append(f"release version check failed: {message}")


def package_versions(lock: dict[str, object]) -> dict[str, str]:
    return {
        package["name"]: package["version"]
        for package in lock["package"]
        if package["name"] in PUBLISHABLE
    }


def main() -> int:
    errors: list[str] = []
    workspace = tomllib.loads((ROOT / "Cargo.toml").read_text())
    workspace_version = workspace["workspace"]["package"]["version"]
    if workspace_version != CURRENT:
        error(errors, f"Cargo.toml workspace.package.version must be {CURRENT}, got {workspace_version}")

    dependencies = workspace["workspace"]["dependencies"]
    for name in sorted(INTERNAL):
        requirement = dependencies[name].get("version")
        if requirement != f"={workspace_version}":
            error(errors, f"workspace dependency {name} must be ={workspace_version}, got {requirement}")

    for manifest in sorted((ROOT / "crates").glob("*/Cargo.toml")):
        package = tomllib.loads(manifest.read_text())["package"]
        if package["name"] in PUBLISHABLE and package.get("version") != {"workspace": True}:
            error(errors, f"{manifest.relative_to(ROOT)} must inherit version.workspace")
        if package["name"] in {"brain-brew-core", "brain-brew-formats"} and "implementation" not in package["description"].lower():
            error(errors, f"{manifest.relative_to(ROOT)} description must identify it as an implementation package")

    lock = tomllib.loads((ROOT / "Cargo.lock").read_text())
    expected_lock_versions = {name: workspace_version for name in PUBLISHABLE}
    if package_versions(lock) != expected_lock_versions:
        error(errors, f"Cargo.lock publishable package versions must be {workspace_version}")

    devenv = (ROOT / "devenv.nix").read_text()
    if 'dist manifest --allow-dirty --tag "v$version"' not in devenv:
        error(errors, "devenv dist:plan must allow the reviewed workflow divergence and derive its tag from Cargo.toml")
    dist = tomllib.loads((ROOT / "dist-workspace.toml").read_text())["dist"]
    if dist.get("installers") != ["shell", "powershell"] or "publish-jobs" in dist:
        error(errors, "cargo-dist must generate only the supported GitHub Release installers")
    release_workflow = (ROOT / ".github/workflows/release.yml").read_text().lower()
    if "homebrew" in release_workflow:
        error(errors, "generated release workflow must not publish an unsupported Homebrew channel")
    flake = (ROOT / "flake.nix").read_text()
    if 'version = workspace.workspace.package.version;' not in flake:
        error(errors, "flake package version must derive from Cargo.toml")

    for document in CURRENT_DOCS:
        source = (ROOT / document).read_text()
        if CURRENT not in source:
            error(errors, f"{document} must name current preview {CURRENT}")
        if STALE in source:
            error(errors, f"{document} retains current-facing stale {STALE} reference")

    changelog = (ROOT / "CHANGELOG.md").read_text()
    history = "`1.0.0-alpha.1` was published with interfaces incompatible with `1.0.0-alpha.2`"
    if history not in changelog:
        error(errors, "CHANGELOG.md must retain the explicit alpha.1/alpha.2 incompatibility record")

    for path in ROOT.rglob("*"):
        if not path.is_file() or any(part in IGNORED_DIRECTORIES for part in path.parts):
            continue
        relative = path.relative_to(ROOT)
        if relative in ALLOWED_STALE_REFERENCES or relative in IGNORED_FILES:
            continue
        try:
            source = path.read_text()
        except UnicodeDecodeError:
            continue
        if STALE in source:
            error(errors, f"stale {STALE} reference in {relative}")

    if errors:
        print("\n".join(errors), file=sys.stderr)
        return 1
    print(f"release version references are synchronized at {workspace_version}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
