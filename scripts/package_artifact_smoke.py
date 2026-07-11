#!/usr/bin/env python3
"""Smoke the CLI built only from Cargo-produced `.crate` archives.

The workspace is permitted to create the archives, but it is never a build or
runtime input after that point.  The CLI is compiled from the safely extracted
archive and its internal dependencies are a checksum-verified directory source
created from the matching produced archives.
"""

from __future__ import annotations

import argparse
import hashlib
import importlib.util
import json
import shutil
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
VERIFY = ROOT / "scripts" / "verify_extracted_crates.py"
spec = importlib.util.spec_from_file_location("verify_extracted_crates", VERIFY)
assert spec and spec.loader
verify = importlib.util.module_from_spec(spec)
spec.loader.exec_module(verify)


def checksum(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as file:
        for block in iter(lambda: file.read(1024 * 1024), b""):
            digest.update(block)
    return digest.hexdigest()


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--evidence-dir", type=Path, default=ROOT / "target" / "release-evidence" / "package-smoke")
    parser.add_argument("--target-sha", required=True, help="immutable commit SHA checked out before this command")
    args = parser.parse_args()
    if len(args.target_sha) != 40 or any(character not in "0123456789abcdef" for character in args.target_sha):
        parser.error("--target-sha must be a lowercase 40-character commit SHA")

    evidence = args.evidence_dir
    evidence.mkdir(parents=True, exist_ok=True)
    report: dict[str, object] = {"target_sha": args.target_sha, "archives": {}, "status": "failed"}
    try:
        with tempfile.TemporaryDirectory(prefix="brainbrew-package-artifact-") as temporary:
            work = Path(temporary)
            version = verify.package_version()
            archives = verify.package_archives(work, version)
            extracted = work / "extracted"
            for label, package in verify.PACKAGES:
                verify.unpack_archive(archives[label], extracted, package, version)
                report["archives"][archives[label].name] = checksum(archives[label])  # type: ignore[index]
            source = verify.staged_directory_source(work, archives, version)
            for label, package in verify.PACKAGES:
                package_root = extracted / f"{package}-{version}"
                verify.write_source_config(package_root, source)
                verify.build_extracted(label, package, version, extracted, offline=True)
            installed = evidence / "install" / "bin"
            installed.mkdir(parents=True, exist_ok=True)
            binary = extracted / "build" / "cli" / "debug" / "brainbrew"
            if not binary.is_file():
                raise verify.VerificationError(f"extracted CLI did not build {binary}")
            installed_binary = installed / "brainbrew"
            shutil.copy2(binary, installed_binary)
            installed_binary.chmod(0o755)
            verify.run([str(ROOT / "scripts" / "release_smoke.sh"), str(installed_binary)], cwd=ROOT)
            report["installed_binary_sha256"] = checksum(installed_binary)
            report["version"] = version
    except Exception as error:
        report["error"] = str(error)
        (evidence / "report.json").write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
        raise
    report["status"] = "passed"
    (evidence / "report.json").write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
    print(f"packaged artifact smoke passed; evidence: {evidence}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
