#!/usr/bin/env python3
"""Fail closed on untriaged production Cargo/npm advisories and licenses.

The policy deliberately consumes raw ``cargo audit --json`` and ``npm audit
--omit=dev --json`` output.  It never turns an audit failure into success: every
observed advisory needs a dated, owned exception in supply-chain-policy.toml.
Development-only findings are recorded separately and do not satisfy or bypass a
production finding with the same advisory ID.
"""

from __future__ import annotations

import argparse
import json
import re
import subprocess
import sys
import tomllib
from datetime import date
from pathlib import Path
from typing import Any

ROOT = Path(__file__).resolve().parent.parent
SPDX_TOKEN = re.compile(r"[A-Za-z0-9.+-]+")


def advisory_id(value: dict[str, Any]) -> str | None:
    advisory = value.get("advisory", value)
    identifier = advisory.get("id") if isinstance(advisory, dict) else None
    if isinstance(identifier, str):
        return identifier
    url = value.get("url")
    if isinstance(url, str) and "/GHSA-" in url:
        return url.rsplit("/", 1)[-1]
    return None


def npm_findings(report: dict[str, Any]) -> list[tuple[str, str]]:
    findings: list[tuple[str, str]] = []
    for package, finding in report.get("vulnerabilities", {}).items():
        if not isinstance(finding, dict) or finding.get("scope") in {"dev", "development"}:
            continue
        for via in finding.get("via", []):
            if isinstance(via, dict):
                identifier = advisory_id(via)
                if identifier:
                    findings.append((identifier, package))
    return findings


def cargo_findings(report: dict[str, Any]) -> list[tuple[str, str]]:
    vulnerabilities = report.get("vulnerabilities", {})
    entries = vulnerabilities.get("list", []) if isinstance(vulnerabilities, dict) else []
    findings: list[tuple[str, str]] = []
    for entry in entries:
        if not isinstance(entry, dict):
            continue
        identifier = advisory_id(entry)
        package = entry.get("package", {}).get("name", "unknown")
        if identifier:
            findings.append((identifier, str(package)))
    return findings


def license_tokens(expression: str) -> set[str]:
    # SPDX operators and the identifier immediately following WITH are not
    # licenses requiring an allow decision (for example LLVM-exception).
    tokens = SPDX_TOKEN.findall(expression)
    return {token for index, token in enumerate(tokens) if token not in {"AND", "OR", "WITH"} and (index == 0 or tokens[index - 1] != "WITH")}


def issues(
    policy_path: Path,
    npm_report: dict[str, Any],
    licenses: dict[str, Any],
    *,
    cargo_report: dict[str, Any] | None = None,
    today: str | None = None,
) -> list[str]:
    policy = tomllib.loads(policy_path.read_text(encoding="utf-8"))
    allowed = set(policy.get("licenses", {}).get("allowed", []))
    exceptions = policy.get("advisory_exceptions", {})
    license_exceptions = policy.get("license_exceptions", {})
    now = date.fromisoformat(today) if today else date.today()
    found: list[str] = []

    observations = [("npm", identifier, package) for identifier, package in npm_findings(npm_report)]
    if cargo_report is not None:
        observations.extend(("Cargo", identifier, package) for identifier, package in cargo_findings(cargo_report))
    for ecosystem, identifier, package in sorted(set(observations)):
        exception = exceptions.get(identifier)
        if not isinstance(exception, dict):
            found.append(f"untriaged production {ecosystem} advisory {identifier} ({package})")
            continue
        missing = [key for key in ("owner", "expires", "rationale") if not exception.get(key)]
        if missing:
            found.append(f"advisory exception {identifier} lacks {', '.join(missing)}")
            continue
        try:
            expiry = date.fromisoformat(str(exception["expires"]))
        except ValueError:
            found.append(f"advisory exception {identifier} has invalid expiry {exception['expires']!r}")
            continue
        if expiry < now:
            found.append(f"expired advisory exception {identifier} ({expiry.isoformat()})")

    for component in licenses.get("packages", []):
        if not isinstance(component, dict) or component.get("scope", "production") in {"dev", "development"}:
            continue
        name, version = component.get("name", "unknown"), component.get("version", "unknown")
        raw_expression = component.get("license")
        expression = raw_expression.get("type") if isinstance(raw_expression, dict) else raw_expression
        # General allowlisted SPDX expressions require no exception, even if a
        # stale exception entry remains while a lock refresh is being reviewed.
        if isinstance(expression, str) and expression.strip() and not (license_tokens(expression) - allowed):
            continue
        exception_id = f"{component.get('ecosystem', 'npm')}:{name}@{version}"
        license_exception = license_exceptions.get(exception_id)
        if license_exception is not None:
            if not isinstance(license_exception, dict) or any(not license_exception.get(key) for key in ("owner", "expires", "rationale")):
                found.append(f"license exception {exception_id} lacks owner, expires, or rationale")
                continue
            try:
                if date.fromisoformat(str(license_exception["expires"])) < now:
                    found.append(f"expired license exception {exception_id} ({license_exception['expires']})")
                    continue
            except ValueError:
                found.append(f"license exception {exception_id} has invalid expiry {license_exception['expires']!r}")
                continue
            if license_exception.get("license") != (expression or "NOASSERTION"):
                found.append(f"license exception {exception_id} does not match observed license")
                continue
            continue
        if not isinstance(expression, str) or not expression.strip():
            found.append(f"production dependency {name}@{version} has no declared license")
            continue
        for token in sorted(license_tokens(expression) - allowed):
            found.append(f"unapproved production license {token} in {name}@{version} ({expression})")
    return found


def cargo_licenses() -> dict[str, Any]:
    metadata = json.loads(subprocess.check_output(["cargo", "metadata", "--locked", "--format-version", "1"], cwd=ROOT, text=True))
    return {"packages": [{"ecosystem": "cargo", "name": package["name"], "version": package["version"], "license": package.get("license"), "scope": "production"} for package in metadata["packages"]]}


def npm_licenses(root: Path) -> dict[str, Any]:
    output = subprocess.check_output(["npm", "ls", "--omit=dev", "--all", "--parseable"], cwd=root, text=True, stderr=subprocess.DEVNULL)
    packages: list[dict[str, str]] = []
    for raw_path in output.splitlines()[1:]:
        manifest = Path(raw_path) / "package.json"
        if not manifest.is_file():
            raise RuntimeError(f"npm production dependency is missing package metadata: {manifest}")
        data = json.loads(manifest.read_text(encoding="utf-8"))
        packages.append({"ecosystem": "npm", "name": data.get("name", manifest.parent.name), "version": data.get("version", "unknown"), "license": data.get("license"), "scope": "production"})
    return {"packages": packages}


def read_json(path: Path) -> dict[str, Any]:
    return json.loads(path.read_text(encoding="utf-8"))


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--policy", type=Path, default=ROOT / "supply-chain-policy.toml")
    parser.add_argument("--npm-audit", type=Path, required=True)
    parser.add_argument("--cargo-audit", type=Path, required=True)
    parser.add_argument("--license-inventory", type=Path)
    parser.add_argument("--write-license-inventory", type=Path)
    parser.add_argument("--npm-root", type=Path, default=ROOT / "documentation")
    args = parser.parse_args()
    try:
        inventory = read_json(args.license_inventory) if args.license_inventory else {"packages": cargo_licenses()["packages"] + npm_licenses(args.npm_root)["packages"]}
        if args.write_license_inventory:
            args.write_license_inventory.parent.mkdir(parents=True, exist_ok=True)
            args.write_license_inventory.write_text(json.dumps(inventory, sort_keys=True, indent=2) + "\n", encoding="utf-8")
        found = issues(args.policy, read_json(args.npm_audit), inventory, cargo_report=read_json(args.cargo_audit))
    except (OSError, ValueError, subprocess.CalledProcessError, tomllib.TOMLDecodeError) as error:
        print(f"dependency policy input failure: {error}", file=sys.stderr)
        return 1
    if found:
        print("dependency policy violations:", file=sys.stderr)
        print("\n".join(f"- {item}" for item in found), file=sys.stderr)
        return 1
    print("dependency advisory and license policy passed")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
