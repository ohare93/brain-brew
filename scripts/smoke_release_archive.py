#!/usr/bin/env python3
"""Verify checksum, version, and behavior of a cargo-dist archive artifact."""

from __future__ import annotations

import argparse
import hashlib
import json
import shutil
import subprocess
import importlib.util
import tarfile
import tempfile
import zipfile
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
METADATA = ROOT / "scripts" / "generate_release_metadata.py"
metadata_spec = importlib.util.spec_from_file_location("generate_release_metadata", METADATA)
assert metadata_spec and metadata_spec.loader
release_metadata = importlib.util.module_from_spec(metadata_spec)
metadata_spec.loader.exec_module(release_metadata)


def digest(path: Path) -> str:
    hasher = hashlib.sha256()
    with path.open("rb") as file:
        for block in iter(lambda: file.read(1024 * 1024), b""):
            hasher.update(block)
    return hasher.hexdigest()


def extract(archive: Path, destination: Path) -> None:
    if zipfile.is_zipfile(archive):
        with zipfile.ZipFile(archive) as contents:
            contents.extractall(destination)
        return
    if tarfile.is_tarfile(archive):
        with tarfile.open(archive) as contents:
            contents.extractall(destination, filter="data")
        return
    raise RuntimeError(f"unsupported cargo-dist archive format: {archive.name}")


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--manifest", type=Path, required=True)
    parser.add_argument("--version", required=True)
    parser.add_argument("--target-sha", required=True)
    parser.add_argument("--workflow-run", default="local")
    parser.add_argument("--evidence-dir", type=Path, required=True)
    args = parser.parse_args()
    manifest = json.loads(args.manifest.read_text(encoding="utf-8"))
    candidates = [Path(path) for path in manifest.get("upload_files", [])]
    archives = [path for path in candidates if path.is_file() and "brainbrew" in path.name and (zipfile.is_zipfile(path) or tarfile.is_tarfile(path))]
    if not archives:
        raise RuntimeError("cargo-dist manifest contains no produced brainbrew archive")
    archive = archives[0]
    sidecar = next((path for path in candidates if path.name.startswith(archive.name) and "sha256" in path.name and path.is_file()), None)
    if sidecar is None or digest(archive) not in sidecar.read_text(encoding="utf-8"):
        raise RuntimeError(f"archive {archive.name} lacks a matching produced SHA-256 sidecar")
    with tempfile.TemporaryDirectory(prefix="brainbrew-dist-artifact-") as temporary:
        extracted = Path(temporary)
        extract(archive, extracted)
        binaries = [path for path in extracted.rglob("brainbrew") if path.is_file()]
        if len(binaries) != 1:
            raise RuntimeError("cargo-dist archive must contain exactly one brainbrew binary")
        version = subprocess.check_output([str(binaries[0]), "--version"], text=True).strip()
        if version != f"brainbrew {args.version}":
            raise RuntimeError(f"archive binary version {version!r} does not match {args.version!r}")
        subprocess.run([str(ROOT / "scripts" / "release_smoke.sh"), str(binaries[0])], cwd=ROOT, check=True)
    args.evidence_dir.mkdir(parents=True, exist_ok=True)
    metadata_dir = args.evidence_dir / "dist-archive-metadata"
    release_metadata.generate([archive], metadata_dir, source_sha=args.target_sha, workflow_run=args.workflow_run)
    metadata_issues = release_metadata.verify([archive], metadata_dir, source_sha=args.target_sha, workflow_run=args.workflow_run)
    if metadata_issues:
        raise RuntimeError("; ".join(metadata_issues))
    (args.evidence_dir / "dist-archive.json").write_text(json.dumps({"target_sha": args.target_sha, "archive": archive.name, "archive_sha256": digest(archive), "status": "passed"}, indent=2) + "\n", encoding="utf-8")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
