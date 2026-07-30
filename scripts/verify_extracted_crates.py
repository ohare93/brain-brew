#!/usr/bin/env python3
"""Test Cargo's packaged crates outside this workspace in publication order.

``pre-publish`` is intentionally offline after packaging. It replaces crates.io
with a Cargo directory source made from the exact extracted archives and vendored
third-party sources, then runs each extracted package's tests so repository-only
tests or missing packaged data cannot escape the release gate. Formats and the
CLI resolve the staged current-version implementation crates without a network
upload. ``indexed`` does *not* install that replacement: it verifies the
extracted dependents against real crates.io once each predecessor is published
and indexed.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import re
import shutil
import subprocess
import sys
import tarfile
import tempfile
import tomllib
from datetime import UTC, datetime
from pathlib import Path, PurePosixPath
from typing import Any

ROOT = Path(__file__).resolve().parent.parent
PACKAGES = (
    ("core", "brain-brew-core"),
    ("formats", "brain-brew-formats"),
    ("cli", "brainbrew"),
)
REQUIRED_ARCHIVE_FILES = {"LICENSE", "README.md"}
REPOSITORY_ONLY_PACKAGE_FILES = {
    "brain-brew-formats": {
        "tests/crowdanki_import_plan.rs",
        "tests/ultimate_geography_fixture.rs",
    },
    "brainbrew": {
        "tests/cli.rs",
        "tests/crowdanki_import_media_cli.rs",
        "tests/crowdanki_import_plan_cli.rs",
        "tests/release_media_integrity.rs",
        "tests/safe_paths.rs",
        "tests/ug_style_fixture.rs",
    },
}


class VerificationError(RuntimeError):
    """An artifact cannot provide release evidence."""


def run(command: list[str], *, cwd: Path | None = None, env: dict[str, str] | None = None) -> None:
    print("+", " ".join(command), flush=True)
    subprocess.run(command, cwd=cwd, env=env, check=True)


def output(command: list[str], *, cwd: Path | None = None) -> str:
    return subprocess.check_output(command, cwd=cwd, text=True)


def package_version() -> str:
    metadata = json.loads(output(["cargo", "metadata", "--no-deps", "--format-version", "1"], cwd=ROOT))
    versions = {
        package["version"]
        for package in metadata["packages"]
        if package["name"] in {name for _, name in PACKAGES}
    }
    if len(versions) != 1:
        raise VerificationError(f"publishable packages do not share one version: {sorted(versions)}")
    return versions.pop()


def package_archives(work: Path, version: str) -> dict[str, Path]:
    """Ask Cargo to package in a disposable workspace with staged predecessors.

    Cargo validates normalized dependencies while creating a package, so its
    package command itself cannot create formats while the current-version core
    is absent from crates.io. The disposable copy changes only that *packaging input* to
    point at the predecessor archive already extracted above. The resulting
    normalized archive drops the path; all verification after this function is
    exclusively against those archives, never this workspace copy.
    """
    package_workspace = work / "package-workspace"
    shutil.copytree(ROOT / "crates", package_workspace / "crates", ignore=shutil.ignore_patterns("target"))
    shutil.copy2(ROOT / "Cargo.toml", package_workspace / "Cargo.toml")
    shutil.copy2(ROOT / "LICENSE", package_workspace / "LICENSE")
    shutil.copy2(ROOT / "Cargo.lock", package_workspace / "Cargo.lock")
    package_target = work / "package-target"
    packaging_stage = work / "packaging-stage"
    archives: dict[str, Path] = {}
    staged: dict[str, Path] = {}
    for label, package in PACKAGES:
        command = ["cargo"]
        if staged:
            config = work / "package-config.toml"
            patches = "\n".join(
                f'{name} = {{ path = {json.dumps(str(path))} }}' for name, path in staged.items()
            )
            config.write_text(f"[patch.crates-io]\n{patches}\n", encoding="utf-8")
            command.extend(["--config", str(config)])
        command.extend(
            [
                "package",
                "--allow-dirty",
                "--no-verify",
                "-p",
                package,
                "--target-dir",
                str(package_target),
            ]
        )
        run(command, cwd=package_workspace)
        archive = package_target / "package" / f"{package}-{version}.crate"
        if not archive.is_file():
            raise VerificationError(f"Cargo did not create expected archive {archive}")
        copied = work / "archives" / archive.name
        copied.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(archive, copied)
        archives[label] = copied
        unpack_archive(copied, packaging_stage, package, version)
        if package != "brainbrew":
            # Change only the next packaging invocation. Cargo removes these paths
            # when it writes the archive's normalized manifest.
            workspace_manifest = package_workspace / "Cargo.toml"
            source = workspace_manifest.read_text(encoding="utf-8")
            old = f'path = "crates/{package}"'
            new = f'path = {json.dumps(str(packaging_stage / f"{package}-{version}"))}'
            member = f'    "crates/{package}",\n'
            if old not in source or member not in source:
                raise VerificationError(f"cannot stage internal dependency {package} for Cargo packaging")
            workspace_manifest.write_text(source.replace(old, new).replace(member, ""), encoding="utf-8")
            staged[package] = packaging_stage / f"{package}-{version}"
    return archives


def safe_member_path(member: tarfile.TarInfo, expected_root: str) -> PurePosixPath:
    path = PurePosixPath(member.name)
    if path.is_absolute() or ".." in path.parts or not path.parts or path.parts[0] != expected_root:
        raise VerificationError(f"unsafe archive member {member.name!r}")
    return path


def safe_link_target(member: tarfile.TarInfo) -> None:
    target = PurePosixPath(member.linkname)
    if target.is_absolute() or ".." in target.parts:
        raise VerificationError(f"unsafe archive symlink {member.name!r} -> {member.linkname!r}")


def unpack_archive(archive: Path, destination: Path, package: str, version: str) -> dict[str, Any]:
    expected_root = f"{package}-{version}"
    files: list[str] = []
    symlinks: list[str] = []
    with tarfile.open(archive, "r:gz") as tar:
        members = tar.getmembers()
        for member in members:
            relative = safe_member_path(member, expected_root)
            target = destination / relative
            if member.isdir():
                target.mkdir(parents=True, exist_ok=True)
            elif member.isreg():
                target.parent.mkdir(parents=True, exist_ok=True)
                source = tar.extractfile(member)
                if source is None:
                    raise VerificationError(f"cannot read archive member {member.name!r}")
                with source, target.open("wb") as out:
                    shutil.copyfileobj(source, out)
                files.append(str(relative))
            elif member.issym():
                safe_link_target(member)
                target.parent.mkdir(parents=True, exist_ok=True)
                target.symlink_to(member.linkname)
                symlinks.append(f"{relative} -> {member.linkname}")
            else:
                raise VerificationError(f"unsupported archive member type for {member.name!r}")
    extracted = destination / expected_root
    if not extracted.is_dir():
        raise VerificationError(f"archive {archive.name} did not contain {expected_root}/")
    names = {str(PurePosixPath(name).relative_to(expected_root)) for name in files}
    missing = REQUIRED_ARCHIVE_FILES - names
    if missing:
        raise VerificationError(f"{archive.name} is missing required archive material: {', '.join(sorted(missing))}")
    leaked_repository_tests = REPOSITORY_ONLY_PACKAGE_FILES.get(package, set()) & names
    if leaked_repository_tests:
        raise VerificationError(
            f"{archive.name} contains repository-only tests without their repository-root data: "
            f"{', '.join(sorted(leaked_repository_tests))}"
        )
    readme = extracted / "README.md"
    readme_text = readme.read_text(encoding="utf-8")
    if not readme_text.strip():
        raise VerificationError(f"{archive.name} has an empty README.md")
    readme_links = re.findall(r"\[[^]]*\]\(([^)]+)\)", readme_text)
    if not readme_links:
        raise VerificationError(f"{archive.name} README.md has no package/help link")
    for link in readme_links:
        if "://" not in link and not (extracted / link.split("#", 1)[0]).is_file():
            raise VerificationError(f"{archive.name} README.md links to missing packaged file {link!r}")
    manifest = tomllib.loads((extracted / "Cargo.toml").read_text(encoding="utf-8"))
    for section in ("dependencies", "dev-dependencies", "build-dependencies"):
        for dependency, specification in manifest.get(section, {}).items():
            if dependency.startswith("brain-brew-") and isinstance(specification, dict) and "path" in specification:
                raise VerificationError(f"{archive.name} retained an internal path dependency")
    return {"archive": archive.name, "files": sorted(files), "symlinks": sorted(symlinks), "readme_links": sorted(readme_links)}


def file_checksum(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as file:
        for block in iter(lambda: file.read(1024 * 1024), b""):
            digest.update(block)
    return digest.hexdigest()


def directory_checksum_manifest(directory: Path, archive: Path) -> None:
    """Make an extracted .crate a checked Cargo directory-source package."""
    checksums: dict[str, str] = {}
    for path in sorted(directory.rglob("*")):
        if path.is_file() and path.name != ".cargo-checksum.json":
            checksums[str(path.relative_to(directory)).replace(os.sep, "/")] = file_checksum(path)
        elif path.is_symlink():
            raise VerificationError(f"directory source cannot safely represent symlink {path}")
    (directory / ".cargo-checksum.json").write_text(
        json.dumps({"files": checksums, "package": file_checksum(archive)}, sort_keys=True), encoding="utf-8"
    )


def staged_directory_source(work: Path, archives: dict[str, Path], version: str) -> Path:
    vendor = work / "staged-registry"
    # This only obtains third-party sources. Every internal crate is replaced below
    # from its just-produced .crate archive before Cargo compiles anything.
    run(["cargo", "vendor", "--locked", "--versioned-dirs", str(vendor)], cwd=ROOT)
    extracted = work / "extracted"
    for _label, package in PACKAGES:
        unpacked = extracted / f"{package}-{version}"
        destination = vendor / unpacked.name
        if destination.exists():
            shutil.rmtree(destination)
        shutil.copytree(unpacked, destination, symlinks=True)
        directory_checksum_manifest(destination, archives[next(label for label, name in PACKAGES if name == package)])
    return vendor


def write_source_config(package_root: Path, source: Path) -> None:
    config = package_root / ".cargo" / "config.toml"
    config.parent.mkdir(exist_ok=True)
    config.write_text(
        "[source.crates-io]\nreplace-with = \"staged-artifacts\"\n\n"
        "[source.staged-artifacts]\n"
        f"directory = {json.dumps(str(source))}\n",
        encoding="utf-8",
    )


def build_extracted(label: str, package: str, version: str, extracted: Path, *, offline: bool) -> None:
    manifest = extracted / f"{package}-{version}" / "Cargo.toml"
    command = ["cargo", "build", "--manifest-path", str(manifest), "--target-dir", str(extracted / "build" / label)]
    if offline:
        command.append("--offline")
    run(command, cwd=manifest.parent)


def test_extracted(label: str, package: str, version: str, extracted: Path) -> None:
    manifest = extracted / f"{package}-{version}" / "Cargo.toml"
    env = os.environ.copy()
    env["BRAINBREW_COLOR"] = "never"
    run(
        [
            "cargo",
            "test",
            "--manifest-path",
            str(manifest),
            "--target-dir",
            str(extracted / "test" / label),
            "--offline",
        ],
        cwd=manifest.parent,
        env=env,
    )


def explain_index_failure(package: str, version: str, error: subprocess.CalledProcessError) -> VerificationError:
    if package in {"brain-brew-formats", "brainbrew"}:
        predecessor = "brain-brew-core" if package == "brain-brew-formats" else "brain-brew-formats"
        return VerificationError(
            f"BLOCKED indexed verification of {package}: {predecessor} {version} was not resolved from crates.io. "
            f"The immutable {predecessor} 1.0.0-alpha.1 is intentionally incompatible; publish and wait for "
            f"{predecessor} {version} to be indexed, then rerun this mode."
        )
    return VerificationError(f"indexed verification of {package} failed: {error}")


def verify_pre_publish(work: Path, archives: dict[str, Path], version: str, report: dict[str, Any]) -> None:
    extracted = work / "extracted"
    for label, package in PACKAGES:
        report["archives"][label] = unpack_archive(archives[label], extracted, package, version)
    vendor = staged_directory_source(work, archives, version)
    for label, package in PACKAGES:
        package_root = extracted / f"{package}-{version}"
        write_source_config(package_root, vendor)
        test_extracted(label, package, version, extracted)
        report["tests"].append(
            {
                "package": package,
                "dependency_source": "staged extracted directory source",
                "network": "offline",
                "status": "passed",
            }
        )


def verify_indexed(work: Path, archives: dict[str, Path], version: str, through: str, report: dict[str, Any]) -> None:
    extracted = work / "extracted"
    limit = [label for label, _ in PACKAGES].index(through) + 1
    for label, package in PACKAGES[:limit]:
        report["archives"][label] = unpack_archive(archives[label], extracted, package, version)
        try:
            build_extracted(label, package, version, extracted, offline=False)
        except subprocess.CalledProcessError as error:
            raise explain_index_failure(package, version, error) from error
        report["builds"].append({"package": package, "dependency_source": "crates.io", "status": "passed"})


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("mode", choices=("pre-publish", "indexed"))
    parser.add_argument("--through", choices=[label for label, _ in PACKAGES], default="cli")
    parser.add_argument("--evidence-dir", type=Path, help="directory for the JSON archive/build evidence")
    args = parser.parse_args()

    version = package_version()
    started = datetime.now(UTC).strftime("%Y%m%dT%H%M%SZ")
    evidence = args.evidence_dir or ROOT / "target" / "release-evidence" / f"extracted-{args.mode}-{started}"
    evidence.mkdir(parents=True, exist_ok=True)
    report: dict[str, Any] = {
        "mode": args.mode,
        "version": version,
        "archives": {},
        "builds": [],
        "tests": [],
    }
    try:
        with tempfile.TemporaryDirectory(prefix="brainbrew-extracted-") as temporary:
            work = Path(temporary)
            archives = package_archives(work, version)
            if args.mode == "pre-publish":
                verify_pre_publish(work, archives, version, report)
            else:
                verify_indexed(work, archives, version, args.through, report)
    except (VerificationError, subprocess.CalledProcessError) as error:
        report["status"] = "failed"
        report["error"] = str(error)
        (evidence / "report.json").write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
        print(f"extracted crate verification failed; evidence: {evidence}", file=sys.stderr)
        print(error, file=sys.stderr)
        return 1
    report["status"] = "passed"
    (evidence / "report.json").write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
    print(f"extracted crate verification passed ({args.mode}); evidence: {evidence}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
