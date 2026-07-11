#!/usr/bin/env python3
"""Create and verify artifact-derived checksums, SBOMs, and provenance records.

This intentionally reads the files that will be uploaded, never workspace source
paths. GitHub's keyless build-provenance action attests the generated provenance
records; this script supplies the deterministic offline verification layer for
artifact bytes, source SHA, and workflow-run binding.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import sys
import tarfile
import tomllib
from pathlib import Path
from typing import Any


def digest(path: Path) -> str:
    hasher = hashlib.sha256()
    with path.open("rb") as file:
        for block in iter(lambda: file.read(1024 * 1024), b""):
            hasher.update(block)
    return hasher.hexdigest()


def component_for(path: Path) -> dict[str, Any]:
    component: dict[str, Any] = {"type": "file", "name": path.name, "hashes": [{"alg": "SHA-256", "content": digest(path)}]}
    if path.suffix == ".crate":
        with tarfile.open(path, "r:gz") as archive:
            manifest = next((item for item in archive.getmembers() if item.name.endswith("/Cargo.toml")), None)
            if manifest is None:
                raise ValueError(f"crate archive {path} has no Cargo.toml")
            source = archive.extractfile(manifest)
            if source is None:
                raise ValueError(f"cannot read {manifest.name}")
            package = tomllib.loads(source.read().decode("utf-8"))["package"]
        component.update({"type": "library", "name": package["name"], "version": package["version"], "licenses": [{"license": {"id": package.get("license", "NOASSERTION")}}]})
    return component


def names(path: Path) -> tuple[str, str]:
    safe = path.name.replace("/", "_")
    return f"{safe}.sbom.cdx.json", f"{safe}.provenance.json"


def generate(artifacts: list[Path], output: Path, *, source_sha: str, workflow_run: str) -> None:
    if len(source_sha) != 40 or any(character not in "0123456789abcdef" for character in source_sha):
        raise ValueError("source SHA must be a lowercase 40-character Git commit")
    if not workflow_run:
        raise ValueError("workflow run must be non-empty")
    output.mkdir(parents=True, exist_ok=True)
    checksums: list[str] = []
    for artifact in sorted(artifacts):
        if not artifact.is_file():
            raise ValueError(f"artifact does not exist: {artifact}")
        sha256 = digest(artifact)
        sbom_name, provenance_name = names(artifact)
        component = component_for(artifact)
        sbom = {
            "$schema": "https://cyclonedx.org/schema/bom-1.5.schema.json",
            "bomFormat": "CycloneDX",
            "specVersion": "1.5",
            "version": 1,
            "metadata": {"component": component, "properties": [{"name": "brainbrew:artifact-sha256", "value": sha256}]},
            "components": [component],
        }
        provenance = {
            "_type": "https://in-toto.io/Statement/v1",
            "subject": [{"name": artifact.name, "digest": {"sha256": sha256}}],
            "predicateType": "https://slsa.dev/provenance/v1",
            "predicate": {"buildDefinition": {"externalParameters": {"source_sha": source_sha, "workflow_run": workflow_run}}, "runDetails": {"builder": {"id": "https://github.com/actions/attest-build-provenance"}}},
        }
        (output / sbom_name).write_text(json.dumps(sbom, sort_keys=True, indent=2) + "\n", encoding="utf-8")
        (output / provenance_name).write_text(json.dumps(provenance, sort_keys=True, indent=2) + "\n", encoding="utf-8")
        checksums.append(f"{sha256}  {artifact.name}")
    (output / "SHA256SUMS").write_text("\n".join(checksums) + "\n", encoding="utf-8")


def verify(artifacts: list[Path], output: Path, *, source_sha: str, workflow_run: str) -> list[str]:
    found: list[str] = []
    checksums_path = output / "SHA256SUMS"
    if not checksums_path.is_file():
        return ["missing SHA256SUMS"]
    expected = {}
    for line in checksums_path.read_text(encoding="utf-8").splitlines():
        parts = line.split(maxsplit=1)
        if len(parts) != 2 or len(parts[0]) != 64:
            found.append("invalid SHA256SUMS entry")
            continue
        expected[parts[1].lstrip("*")] = parts[0]
    for artifact in artifacts:
        actual = digest(artifact) if artifact.is_file() else None
        if expected.get(artifact.name) != actual:
            found.append(f"checksum mismatch for {artifact.name}")
        sbom_name, provenance_name = names(artifact)
        try:
            sbom = json.loads((output / sbom_name).read_text(encoding="utf-8"))
            sbom_hash = sbom["metadata"]["properties"][0]["value"]
            if sbom_hash != actual:
                found.append(f"SBOM hash mismatch for {artifact.name}")
        except (OSError, ValueError, KeyError, IndexError, TypeError):
            found.append(f"missing or invalid SBOM for {artifact.name}")
        try:
            provenance = json.loads((output / provenance_name).read_text(encoding="utf-8"))
            subject = provenance["subject"]
            parameters = provenance["predicate"]["buildDefinition"]["externalParameters"]
            if subject != [{"name": artifact.name, "digest": {"sha256": actual}}]:
                found.append(f"provenance artifact hash mismatch for {artifact.name}")
            if parameters.get("source_sha") != source_sha:
                found.append(f"provenance source SHA mismatch for {artifact.name}")
            if str(parameters.get("workflow_run")) != str(workflow_run):
                found.append(f"provenance workflow-run mismatch for {artifact.name}")
        except (OSError, ValueError, KeyError, IndexError, TypeError):
            found.append(f"missing or invalid provenance for {artifact.name}")
    return found


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--artifact", type=Path, action="append", required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--source-sha", required=True)
    parser.add_argument("--workflow-run", required=True)
    parser.add_argument("--verify", action="store_true")
    args = parser.parse_args()
    try:
        if args.verify:
            found = verify(args.artifact, args.output, source_sha=args.source_sha, workflow_run=args.workflow_run)
            if found:
                print("release metadata verification failed:\n" + "\n".join(f"- {item}" for item in found), file=sys.stderr)
                return 1
        else:
            generate(args.artifact, args.output, source_sha=args.source_sha, workflow_run=args.workflow_run)
    except (OSError, ValueError, tarfile.TarError, tomllib.TOMLDecodeError) as error:
        print(f"release metadata failure: {error}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
