#!/usr/bin/env python3
"""Check workspace package metadata needed for crates.io publishing."""

from __future__ import annotations

import json
import subprocess
import sys

EXPECTED_PACKAGES = {"brain-brew-core", "brain-brew-formats", "brainbrew"}
INTERNAL_DEPS = {"brain-brew-core", "brain-brew-formats"}


def main() -> int:
    metadata = json.loads(
        subprocess.check_output(
            ["cargo", "metadata", "--no-deps", "--format-version", "1"], text=True
        )
    )
    workspace_packages = {
        package["name"]: package
        for package in metadata["packages"]
        if package["id"] in metadata["workspace_members"]
    }
    errors: list[str] = []

    missing = EXPECTED_PACKAGES - workspace_packages.keys()
    if missing:
        errors.append(f"missing workspace packages: {', '.join(sorted(missing))}")

    workspace_version = metadata["workspace_default_members"] and next(
        package["version"] for package in workspace_packages.values()
    )

    for name in sorted(EXPECTED_PACKAGES & workspace_packages.keys()):
        package = workspace_packages[name]
        if package.get("publish") is not None:
            errors.append(f"{name}: package.publish must allow crates.io publishing")
        for field in ["description", "license", "repository", "readme"]:
            if not package.get(field):
                errors.append(f"{name}: missing package.{field}")
        if package["version"] != workspace_version:
            errors.append(
                f"{name}: version {package['version']} does not match workspace version {workspace_version}"
            )

        for dep in package["dependencies"]:
            if dep["name"] not in INTERNAL_DEPS:
                continue
            expected_req = f"={workspace_version}"
            if dep["req"] != expected_req:
                errors.append(
                    f"{name}: dependency {dep['name']} must use exact req {expected_req}, got {dep['req']}"
                )
            if not dep.get("path"):
                errors.append(
                    f"{name}: dependency {dep['name']} should keep a local path for workspace development"
                )

    if errors:
        for error in errors:
            print(f"crates.io metadata check failed: {error}", file=sys.stderr)
        return 1

    print(
        f"crates.io metadata ok for {', '.join(sorted(EXPECTED_PACKAGES))} at {workspace_version}"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
