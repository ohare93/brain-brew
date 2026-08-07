#!/usr/bin/env python3
"""Hermetic Ultimate Geography fixture synchronization and drift checks.

This module deliberately uses only the Python standard library and the already
built, pinned Brain Brew binary. It never invokes Git/Jujutsu, accesses the
network, or writes to the Ultimate Geography checkout.
"""

from __future__ import annotations

import argparse
import copy
import csv
import hashlib
import json
import os
import pathlib
import re
import shutil
import stat
import subprocess
import sys
import tempfile
import unicodedata
import uuid
from collections.abc import Iterable, Mapping, Sequence
from typing import Any


SCHEMA_VERSION = 3
PINNED_UG_REVISION = "54b32544a84d1746403ac8efaa3af0e2250ad4c0"
PINNED_UG_REF = "brainbrew-migration"
PINNED_UG_REPOSITORY = "https://github.com/anki-geo/ultimate-geography.git"
PINNED_HARDCORE_REVISION = "09ce7c3ba665eac6b0794d089a4e0bbafbfc0f46"
PINNED_HARDCORE_REF = "main"
PINNED_HARDCORE_REPOSITORY = "https://github.com/anki-geo/hardcore-geography.git"
PINNED_HARDCORE_README_SHA256 = (
    "ea7da97156e5688e36b8c32eaaf2f5dd805620edf5c3face9ff4d7508fdb7e07"
)
PINNED_HARDCORE_SOURCES_SHA256 = (
    "aa3d9a96a0ae9dd15f6e108891b9a667c70ad9a97f64036045c13a5f4ecf204c"
)
PINNED_HARDCORE_ATTRIBUTION_SHA256 = (
    "aada4219077e5fa77756701e537789c22b2baad29a961c456b3406f3e3629b06"
)
PINNED_ATTRIBUTION_COVERAGE_SHA256 = (
    "13a1d5c1d04a8eacaae3dd3c1c952483128b8f089779dc622f2014055b72351d"
)
PINNED_BRAINBREW_REVISION = "68a828350de4bda46af85b5167bca807edd7d733"
PINNED_BRAINBREW_REF = "rust-brainbrew"
PINNED_BRAINBREW_REPOSITORY = "https://github.com/jeprecated/brain-brew.git"
PINNED_BRAINBREW_VERSION = "1.0.0-alpha.3"
PINNED_BRAINBREW_EXECUTABLE_SHA256 = (
    "0a4963db7bf3e2e8ae019902e5aa98fabd165ba93687811db5ed7cbdd064421f"
)
PINNED_BRAINBREW_EXECUTABLE_BYTES = 15_777_528
PINNED_BRAINBREW_SOURCE_SHA256 = (
    "53b7c7a31848035861115972881dfbd70e04ab27ddae11be88b945c7cabe7a27"
)
PINNED_BRAINBREW_SOURCE_FILES = 69
PINNED_BRAINBREW_SOURCE_BYTES = 2_985_380
PINNED_BRAINBREW_CARGO_LOCK_SHA256 = (
    "ea2858def2a0528b781d992930a8f6067e71b4baa8ef6bf6b298f3b44a120cd1"
)

SOURCE_ROOT_NAME = "ultimate-geography"
EXPECTED_ROOT_PARTS = ("ultimate-geography-expected", "crowdanki")
HARDCORE_ATTRIBUTION_ROOT_PARTS = (
    "ultimate-geography-attribution",
    "hardcore-geography",
)
LOCK_NAME = "ultimate-geography.lock.json"
SOURCE_WHITELIST = (
    "LICENSE.md",
    "brainbrew-hardcore.yaml",
    "brainbrew.yaml",
    "deck-hardcore.yaml",
    "deck.yaml",
    "descriptions",
    "goldens",
    "media",
    "media.yaml",
    "note-types.yaml",
    "overlays",
    "sources.csv",
    "styles",
    "templates",
)
ATTRIBUTION_FILES = ("LICENSE.md", "sources.csv")
HARDCORE_ATTRIBUTION_FILES = ("README.md", "sources.csv")
UG_NOTICE_MEDIA_FILES = (
    "_ug-interactive_map_config.js",
    "_ug-interactive_map_init.js",
    "_ug-jsvectormap.js",
    "_ug-jsvectormap.min.css",
    "_ug-world.js",
)
ATTRIBUTION_CSV_COLUMNS = ("File", "Source", "License", "Modifications")
BRAINBREW_GENERATOR_SOURCE_PATHS = (
    "Cargo.lock",
    "Cargo.toml",
    "crates/brain-brew-core/Cargo.toml",
    "crates/brain-brew-core/src",
    "crates/brain-brew-formats/Cargo.toml",
    "crates/brain-brew-formats/src",
    "crates/brain-brew-cli/Cargo.toml",
    "crates/brain-brew-cli/assets",
    "crates/brain-brew-cli/src",
)
MANIFEST_SPECS = (
    ("main", "brainbrew.yaml", 74),
    ("companion", "brainbrew-hardcore.yaml", 26),
)
TOTAL_TARGETS = 100
TREE_ALGORITHM = "sha256-path-length-content-v1"
JSON_TREE_ALGORITHM = "sha256-path-canonical-json-v1"
TREE_DOMAIN = b"brainbrew-tree-sha256-v1\0"
JSON_TREE_DOMAIN = b"brainbrew-json-tree-sha256-v1\0"
GENERATOR_SOURCE_DOMAIN = b"brainbrew-generator-source-sha256-v1\0"
GENERATOR_SOURCE_ALGORITHM = "sha256-generator-path-length-content-v1"
GENERATOR_IDENTITY_ALGORITHM = "sha256-canonical-json-v1"
ATTRIBUTION_COVERAGE_ALGORITHM = "sha256-normalized-filename-owner-v1"
ATTRIBUTION_FILENAME_NORMALIZATION = "unicode-nfc-posix-basename-v1"
ATTRIBUTION_COVERAGE_DOMAIN = b"brainbrew-attribution-coverage-sha256-v1\0"
TARGET_RE = re.compile(r"^[a-z0-9][a-z0-9.-]*$")
REVISION_RE = re.compile(r"^[0-9a-f]{40}$")
TOP_LEVEL_TARGET_RE = re.compile(r"^  ([A-Za-z0-9][A-Za-z0-9._-]*):\s*(?:#.*)?$")


class FixtureError(RuntimeError):
    """A fail-closed fixture contract violation."""


def _update_framed(hasher: Any, relative: str, content: bytes) -> None:
    hasher.update(relative.encode("utf-8"))
    hasher.update(b"\0")
    hasher.update(str(len(content)).encode("ascii"))
    hasher.update(b"\0")
    hasher.update(content)


def _regular_files(root: pathlib.Path) -> list[tuple[str, pathlib.Path]]:
    if not root.is_dir():
        raise FixtureError(f"missing directory: {root}")
    files: list[tuple[str, pathlib.Path]] = []
    for current, dir_names, file_names in os.walk(root, topdown=True, followlinks=False):
        current_path = pathlib.Path(current)
        for name in sorted(dir_names):
            path = current_path / name
            mode = path.lstat().st_mode
            if stat.S_ISLNK(mode):
                raise FixtureError(f"symlink is forbidden in fixture trees: {path}")
            if not stat.S_ISDIR(mode):
                raise FixtureError(f"special directory entry is forbidden in fixture trees: {path}")
        dir_names.sort()
        for name in sorted(file_names):
            path = current_path / name
            mode = path.lstat().st_mode
            if stat.S_ISLNK(mode):
                raise FixtureError(f"symlink is forbidden in fixture trees: {path}")
            if not stat.S_ISREG(mode):
                raise FixtureError(f"special file is forbidden in fixture trees: {path}")
            files.append((path.relative_to(root).as_posix(), path))
    files.sort(key=lambda item: item[0])
    return files


def _metadata_for_files(
    files: Iterable[tuple[str, pathlib.Path]], *, domain: bytes, algorithm: str
) -> dict[str, Any]:
    hasher = hashlib.sha256()
    hasher.update(domain)
    file_count = 0
    byte_count = 0
    for relative, path in sorted(files, key=lambda item: item[0]):
        content = path.read_bytes()
        _update_framed(hasher, relative, content)
        file_count += 1
        byte_count += len(content)
    return {
        "algorithm": algorithm,
        "file_count": file_count,
        "byte_count": byte_count,
        "sha256": hasher.hexdigest(),
    }


def tree_metadata(root: pathlib.Path) -> dict[str, Any]:
    """Return deterministic path/content metadata for every regular file below root."""

    return _metadata_for_files(
        _regular_files(root), domain=TREE_DOMAIN, algorithm=TREE_ALGORITHM
    )


def _selected_files(
    root: pathlib.Path, selected_paths: Sequence[str], label: str
) -> list[tuple[str, pathlib.Path]]:
    files: list[tuple[str, pathlib.Path]] = []
    for relative in selected_paths:
        path = root / relative
        try:
            mode = path.lstat().st_mode
        except FileNotFoundError as error:
            raise FixtureError(f"missing {label} path: {path}") from error
        if stat.S_ISLNK(mode):
            raise FixtureError(f"symlink is forbidden in {label}: {path}")
        if stat.S_ISREG(mode):
            files.append((relative, path))
        elif stat.S_ISDIR(mode):
            for child_relative, child in _regular_files(path):
                files.append((f"{relative}/{child_relative}", child))
        else:
            raise FixtureError(f"special filesystem entry is forbidden in {label}: {path}")
    files.sort(key=lambda item: item[0])
    return files


def selected_tree_metadata(
    root: pathlib.Path,
    selected_paths: Sequence[str],
    *,
    domain: bytes = TREE_DOMAIN,
    algorithm: str = TREE_ALGORITHM,
    label: str,
) -> dict[str, Any]:
    return _metadata_for_files(
        _selected_files(root, selected_paths, label), domain=domain, algorithm=algorithm
    )


def _file_metadata(path: pathlib.Path) -> dict[str, Any]:
    try:
        mode = path.lstat().st_mode
    except FileNotFoundError as error:
        raise FixtureError(f"missing file: {path}") from error
    if stat.S_ISLNK(mode) or not stat.S_ISREG(mode):
        raise FixtureError(f"expected a regular file: {path}")
    content = path.read_bytes()
    return {
        "algorithm": "sha256",
        "byte_count": len(content),
        "sha256": hashlib.sha256(content).hexdigest(),
    }


def require_tree_metadata(
    root: pathlib.Path, recorded: Mapping[str, Any], label: str
) -> dict[str, Any]:
    actual = tree_metadata(root)
    if actual["sha256"] != recorded.get("sha256"):
        raise FixtureError(
            f"{label} digest drift: expected {recorded.get('sha256')}, actual {actual['sha256']}"
        )
    for field in ("algorithm", "file_count", "byte_count"):
        if actual[field] != recorded.get(field):
            raise FixtureError(
                f"{label} {field} drift: expected {recorded.get(field)!r}, actual {actual[field]!r}"
            )
    return actual


def _canonical_json_bytes(value: Any) -> bytes:
    try:
        text = json.dumps(
            value,
            ensure_ascii=False,
            allow_nan=False,
            sort_keys=True,
            separators=(",", ":"),
        )
    except (TypeError, ValueError) as error:
        raise FixtureError(f"JSON value cannot be canonicalized: {error}") from error
    return text.encode("utf-8")


def normalize_attribution_filename(value: str, label: str) -> str:
    """Return the one accepted filename key or fail on normalization drift."""

    if not isinstance(value, str) or not value:
        raise FixtureError(f"{label} must be a non-empty filename")
    if value != value.strip():
        raise FixtureError(f"{label} has leading or trailing whitespace: {value!r}")
    if "\0" in value or "/" in value or "\\" in value:
        raise FixtureError(f"{label} must be a POSIX basename: {value!r}")
    normalized = unicodedata.normalize("NFC", value)
    if normalized != value:
        raise FixtureError(f"{label} is not in canonical Unicode NFC form: {value!r}")
    if pathlib.PurePosixPath(normalized).name != normalized or normalized in {".", ".."}:
        raise FixtureError(f"{label} must be a POSIX basename: {value!r}")
    return normalized


def _attribution_csv_entries(
    path: pathlib.Path, label: str
) -> dict[str, dict[str, str]]:
    try:
        handle = path.open("r", encoding="utf-8", newline="")
    except (OSError, UnicodeError) as error:
        raise FixtureError(f"cannot read {label} {path}: {error}") from error
    with handle:
        try:
            reader = csv.DictReader(handle, strict=True)
            if reader.fieldnames != list(ATTRIBUTION_CSV_COLUMNS):
                raise FixtureError(
                    f"{label} columns drifted: expected {list(ATTRIBUTION_CSV_COLUMNS)!r}, "
                    f"actual {reader.fieldnames!r}"
                )
            entries: dict[str, dict[str, str]] = {}
            for row_number, row in enumerate(reader, start=2):
                if (
                    None in row
                    or set(row) != set(ATTRIBUTION_CSV_COLUMNS)
                    or any(value is None for value in row.values())
                ):
                    raise FixtureError(f"{label} row {row_number} is malformed")
                if not row["Source"] or not row["License"]:
                    raise FixtureError(
                        f"{label} row {row_number} must identify both source and license"
                    )
                filename = normalize_attribution_filename(
                    row["File"], f"{label} row {row_number} File"
                )
                if filename in entries:
                    raise FixtureError(
                        f"ambiguous attribution: {label} repeats normalized filename {filename!r}"
                    )
                entries[filename] = row
        except (csv.Error, UnicodeError) as error:
            raise FixtureError(f"cannot parse {label} {path}: {error}") from error
    return entries


def _normalized_media_filenames(media_root: pathlib.Path) -> dict[str, str]:
    media: dict[str, str] = {}
    for relative, _ in _regular_files(media_root):
        filename = normalize_attribution_filename(relative, "vendored media filename")
        if filename in media:
            raise FixtureError(
                "ambiguous attribution: vendored media filenames collide after "
                f"{ATTRIBUTION_FILENAME_NORMALIZATION}: {media[filename]!r}, {relative!r}"
            )
        media[filename] = relative
    return media


def _attribution_owner_digest(owners: Mapping[str, str]) -> str:
    hasher = hashlib.sha256()
    hasher.update(ATTRIBUTION_COVERAGE_DOMAIN)
    for filename, owner in sorted(owners.items()):
        _update_framed(hasher, filename, owner.encode("utf-8"))
    return hasher.hexdigest()


def _attribution_category_counts(entries: Mapping[str, Any]) -> tuple[int, int]:
    return (
        sum(filename.startswith("ug-flag-") for filename in entries),
        sum(filename.startswith("ug-map-") for filename in entries),
    )


def attribution_coverage_metadata(
    media_root: pathlib.Path,
    ug_sources_csv: pathlib.Path,
    hardcore_sources_csv: pathlib.Path,
    *,
    ug_notice_files: Sequence[str] = UG_NOTICE_MEDIA_FILES,
    enforce_release_counts: bool = True,
) -> dict[str, Any]:
    """Prove one unambiguous attribution owner for every vendored media file."""

    media = _normalized_media_filenames(media_root)
    ug_entries = _attribution_csv_entries(ug_sources_csv, "UG sources.csv")
    hardcore_entries = _attribution_csv_entries(
        hardcore_sources_csv, "Hardcore Geography sources.csv"
    )
    overlap = sorted(set(ug_entries) & set(hardcore_entries))
    if overlap:
        raise FixtureError(
            "ambiguous attribution: normalized filenames occur in both UG and Hardcore "
            f"sources.csv: {overlap!r}"
        )

    notice_files: set[str] = set()
    for raw_filename in ug_notice_files:
        filename = normalize_attribution_filename(
            raw_filename, "UG LICENSE.md media notice filename"
        )
        if filename in notice_files or filename in ug_entries or filename in hardcore_entries:
            raise FixtureError(
                f"ambiguous attribution: normalized filename {filename!r} has multiple owners"
            )
        notice_files.add(filename)

    owners = {
        filename: "ultimate-geography:sources.csv" for filename in ug_entries
    }
    owners.update(
        {
            filename: "hardcore-geography:sources.csv"
            for filename in hardcore_entries
        }
    )
    owners.update(
        {
            filename: "ultimate-geography:LICENSE.md"
            for filename in notice_files
        }
    )

    media_names = set(media)
    owner_names = set(owners)
    missing = sorted(media_names - owner_names)
    extra = sorted(owner_names - media_names)
    if missing:
        raise FixtureError(
            "unattributed media after normalized filename matching: "
            f"{missing!r}"
        )
    if extra:
        raise FixtureError(
            "attribution entries have no vendored media after normalized filename matching: "
            f"{extra!r}"
        )

    image_files = {
        filename
        for filename in media_names
        if pathlib.PurePosixPath(filename).suffix in {".png", ".svg"}
    }
    csv_files = set(ug_entries) | set(hardcore_entries)
    if csv_files != image_files:
        raise FixtureError(
            "image attribution inventory drift: "
            f"missing={sorted(image_files - csv_files)!r}, "
            f"non_images={sorted(csv_files - image_files)!r}"
        )
    non_image_files = media_names - image_files
    if notice_files != non_image_files:
        raise FixtureError(
            "UG LICENSE.md media notice inventory drift: "
            f"missing={sorted(non_image_files - notice_files)!r}, "
            f"extra={sorted(notice_files - non_image_files)!r}"
        )

    ug_flags, ug_maps = _attribution_category_counts(ug_entries)
    hardcore_flags, hardcore_maps = _attribution_category_counts(hardcore_entries)
    coverage = {
        "algorithm": ATTRIBUTION_COVERAGE_ALGORITHM,
        "filename_normalization": ATTRIBUTION_FILENAME_NORMALIZATION,
        "media_file_count": len(media_names),
        "image_file_count": len(image_files),
        "unattributed_file_count": 0,
        "ambiguous_file_count": 0,
        "ultimate_geography": {
            "sources_csv_file_count": len(ug_entries),
            "license_notice_file_count": len(notice_files),
            "attributed_file_count": len(ug_entries) + len(notice_files),
            "flag_file_count": ug_flags,
            "map_file_count": ug_maps,
        },
        "hardcore_geography": {
            "sources_csv_file_count": len(hardcore_entries),
            "attributed_file_count": len(hardcore_entries),
            "flag_file_count": hardcore_flags,
            "map_file_count": hardcore_maps,
        },
        "sha256": _attribution_owner_digest(owners),
    }
    if enforce_release_counts and coverage != _reviewed_attribution_coverage():
        raise FixtureError(
            "media attribution coverage drifted from the reviewed 609-file inventory: "
            f"actual={coverage!r}"
        )
    return coverage


def _expected_files(expected_root: pathlib.Path) -> list[tuple[str, pathlib.Path]]:
    if not expected_root.is_dir():
        raise FixtureError(f"missing expected-output directory: {expected_root}")
    files: list[tuple[str, pathlib.Path]] = []
    for entry in sorted(expected_root.iterdir(), key=lambda path: path.name):
        mode = entry.lstat().st_mode
        if stat.S_ISLNK(mode) or not stat.S_ISDIR(mode):
            raise FixtureError(
                f"expected output root may contain target directories only: {entry}"
            )
        if not TARGET_RE.fullmatch(entry.name):
            raise FixtureError(f"invalid expected target directory name: {entry.name!r}")
        children = sorted(entry.iterdir(), key=lambda path: path.name)
        if [child.name for child in children] != ["deck.json"]:
            raise FixtureError(
                f"expected target {entry.name} must contain exactly deck.json; found "
                f"{[child.name for child in children]!r}"
            )
        deck_json = children[0]
        mode = deck_json.lstat().st_mode
        if stat.S_ISLNK(mode) or not stat.S_ISREG(mode):
            raise FixtureError(f"expected output must be a regular file: {deck_json}")
        files.append((f"{entry.name}/deck.json", deck_json))
    return files


def json_tree_metadata(expected_root: pathlib.Path) -> dict[str, Any]:
    """Hash expected outputs as parsed, canonical JSON rather than serialization bytes."""

    hasher = hashlib.sha256()
    hasher.update(JSON_TREE_DOMAIN)
    canonical_byte_count = 0
    file_count = 0
    for relative, path in _expected_files(expected_root):
        try:
            value = json.loads(path.read_text(encoding="utf-8"))
        except (OSError, UnicodeError, json.JSONDecodeError) as error:
            raise FixtureError(f"invalid expected JSON {path}: {error}") from error
        canonical = _canonical_json_bytes(value)
        _update_framed(hasher, relative, canonical)
        canonical_byte_count += len(canonical)
        file_count += 1
    return {
        "algorithm": JSON_TREE_ALGORITHM,
        "file_count": file_count,
        "canonical_byte_count": canonical_byte_count,
        "sha256": hasher.hexdigest(),
    }


def _expected_target_set(expected_root: pathlib.Path) -> set[str]:
    return {relative.split("/", 1)[0] for relative, _ in _expected_files(expected_root)}


def validate_expected_tree(
    expected_root: pathlib.Path,
    manifest_mapping: Mapping[str, Sequence[str]],
    recorded: Mapping[str, Any],
    *,
    enforce_release_counts: bool = True,
) -> dict[str, Any]:
    all_targets: list[str] = []
    for manifest_path, targets in sorted(manifest_mapping.items()):
        if list(targets) != sorted(targets):
            raise FixtureError(f"target mapping for {manifest_path} is not sorted")
        all_targets.extend(targets)
    if len(all_targets) != len(set(all_targets)):
        raise FixtureError("a target is assigned to more than one fixture manifest")
    expected_targets = _expected_target_set(expected_root)
    mapped_targets = set(all_targets)
    if expected_targets != mapped_targets:
        missing = sorted(mapped_targets - expected_targets)
        extra = sorted(expected_targets - mapped_targets)
        raise FixtureError(
            f"expected target set drift: missing={missing!r}, extra={extra!r}"
        )
    if enforce_release_counts:
        counts = {path: len(targets) for path, targets in manifest_mapping.items()}
        required = {path: count for _, path, count in MANIFEST_SPECS}
        if counts != required or len(all_targets) != TOTAL_TARGETS:
            raise FixtureError(
                f"fixture target count drift: required {required!r} / {TOTAL_TARGETS} total, "
                f"actual {counts!r} / {len(all_targets)} total"
            )
    actual = json_tree_metadata(expected_root)
    if actual["sha256"] != recorded.get("sha256"):
        raise FixtureError(
            "expected JSON semantic digest drift: "
            f"expected {recorded.get('sha256')}, actual {actual['sha256']}"
        )
    for field in ("algorithm", "file_count", "canonical_byte_count"):
        if actual[field] != recorded.get(field):
            raise FixtureError(
                f"expected JSON {field} drift: expected {recorded.get(field)!r}, "
                f"actual {actual[field]!r}"
            )
    return actual


def manifest_targets(path: pathlib.Path) -> list[str]:
    """Extract keys from the one top-level `targets:` map without a YAML dependency."""

    try:
        lines = path.read_text(encoding="utf-8").splitlines()
    except (OSError, UnicodeError) as error:
        raise FixtureError(f"cannot read manifest {path}: {error}") from error
    in_targets = False
    found_targets = False
    targets: list[str] = []
    for line in lines:
        if not in_targets:
            if line == "targets:":
                if found_targets:
                    raise FixtureError(f"duplicate top-level targets map in {path}")
                found_targets = True
                in_targets = True
            continue
        if line and not line[0].isspace() and not line.lstrip().startswith("#"):
            break
        match = TOP_LEVEL_TARGET_RE.fullmatch(line)
        if match:
            target = match.group(1)
            if target in targets:
                raise FixtureError(f"duplicate target {target!r} in {path}")
            targets.append(target)
    if not found_targets:
        raise FixtureError(f"manifest has no top-level targets map: {path}")
    if not targets:
        raise FixtureError(f"manifest has an empty top-level targets map: {path}")
    return sorted(targets)


def _source_files(root: pathlib.Path, *, require_exact_top_level: bool) -> list[tuple[str, pathlib.Path]]:
    if not root.is_dir():
        raise FixtureError(f"Ultimate Geography source directory is missing: {root}")
    expected = set(SOURCE_WHITELIST)
    if require_exact_top_level:
        actual = {entry.name for entry in root.iterdir()}
        if actual != expected:
            raise FixtureError(
                "source fixture top-level whitelist drift: "
                f"missing={sorted(expected - actual)!r}, extra={sorted(actual - expected)!r}"
            )
    files: list[tuple[str, pathlib.Path]] = []
    for relative in SOURCE_WHITELIST:
        path = root / relative
        try:
            mode = path.lstat().st_mode
        except FileNotFoundError as error:
            raise FixtureError(f"missing whitelisted UG source path: {path}") from error
        if stat.S_ISLNK(mode):
            raise FixtureError(f"symlink is forbidden in whitelisted UG source: {path}")
        if stat.S_ISREG(mode):
            files.append((relative, path))
        elif stat.S_ISDIR(mode):
            for child_relative, child in _regular_files(path):
                files.append((f"{relative}/{child_relative}", child))
        else:
            raise FixtureError(f"special whitelisted UG source path is forbidden: {path}")
    files.sort(key=lambda item: item[0])
    return files


def source_tree_metadata(root: pathlib.Path, *, exact: bool) -> dict[str, Any]:
    return _metadata_for_files(
        _source_files(root, require_exact_top_level=exact),
        domain=TREE_DOMAIN,
        algorithm=TREE_ALGORITHM,
    )


def _require_source_metadata(
    fixture_root: pathlib.Path, recorded: Mapping[str, Any]
) -> dict[str, Any]:
    actual = source_tree_metadata(fixture_root, exact=True)
    if actual["sha256"] != recorded.get("sha256"):
        raise FixtureError(
            "source snapshot digest drift: "
            f"expected {recorded.get('sha256')}, actual {actual['sha256']}"
        )
    for field in ("algorithm", "file_count", "byte_count"):
        if actual[field] != recorded.get(field):
            raise FixtureError(
                f"source snapshot {field} drift: expected {recorded.get(field)!r}, "
                f"actual {actual[field]!r}"
            )
    return actual


def _manifest_records(source_root: pathlib.Path) -> list[dict[str, Any]]:
    records = []
    total = 0
    for role, manifest_path, required_count in MANIFEST_SPECS:
        targets = manifest_targets(source_root / manifest_path)
        if len(targets) != required_count:
            raise FixtureError(
                f"refusing target-count change for {manifest_path}: required {required_count}, "
                f"found {len(targets)}; explicit fixture-contract review is required"
            )
        records.append(
            {
                "role": role,
                "path": manifest_path,
                "target_count": len(targets),
                "targets": targets,
            }
        )
        total += len(targets)
    if total != TOTAL_TARGETS:
        raise FixtureError(
            f"refusing target total {total}; fixture contract requires {TOTAL_TARGETS}"
        )
    target_names = [target for record in records for target in record["targets"]]
    if len(target_names) != len(set(target_names)):
        raise FixtureError("main and companion fixture manifests contain duplicate target names")
    return records


def _mapping_from_records(records: Sequence[Mapping[str, Any]]) -> dict[str, list[str]]:
    mapping: dict[str, list[str]] = {}
    for record in records:
        path = record.get("path")
        targets = record.get("targets")
        if not isinstance(path, str) or not isinstance(targets, list) or not all(
            isinstance(target, str) for target in targets
        ):
            raise FixtureError("lock manifest target mapping is malformed")
        if path in mapping:
            raise FixtureError(f"duplicate manifest mapping in lock: {path}")
        mapping[path] = list(targets)
    return mapping


def _tree_record(root: pathlib.Path) -> dict[str, Any]:
    return tree_metadata(root)


def _load_lock(path: pathlib.Path) -> dict[str, Any]:
    try:
        value = json.loads(path.read_text(encoding="utf-8"))
    except FileNotFoundError as error:
        raise FixtureError(f"fixture lock is missing: {path}") from error
    except (OSError, UnicodeError, json.JSONDecodeError) as error:
        raise FixtureError(f"cannot parse fixture lock {path}: {error}") from error
    if not isinstance(value, dict):
        raise FixtureError(f"fixture lock root must be an object: {path}")
    return value


def _write_json_temp(path: pathlib.Path, value: Mapping[str, Any]) -> pathlib.Path:
    path.parent.mkdir(parents=True, exist_ok=True)
    handle, temp_name = tempfile.mkstemp(prefix=f".{path.name}.", dir=path.parent)
    temp_path = pathlib.Path(temp_name)
    try:
        with os.fdopen(handle, "w", encoding="utf-8", newline="\n") as output:
            json.dump(value, output, ensure_ascii=False, indent=2, sort_keys=True)
            output.write("\n")
            output.flush()
            os.fsync(output.fileno())
        return temp_path
    except BaseException:
        temp_path.unlink(missing_ok=True)
        raise


def _reviewed_generator_source() -> dict[str, Any]:
    return {
        "algorithm": GENERATOR_SOURCE_ALGORITHM,
        "file_count": PINNED_BRAINBREW_SOURCE_FILES,
        "byte_count": PINNED_BRAINBREW_SOURCE_BYTES,
        "sha256": PINNED_BRAINBREW_SOURCE_SHA256,
        "paths": list(BRAINBREW_GENERATOR_SOURCE_PATHS),
    }


def _reviewed_generator_build() -> dict[str, Any]:
    return {
        "cargo_command": (
            "CARGO_INCREMENTAL=0 "
            "RUSTFLAGS='-C debuginfo=0 --remap-path-prefix=<source-root>=/brainbrew' "
            "cargo build --locked --offline --release -p brainbrew --bin brainbrew"
        ),
        "cargo_lock_sha256": PINNED_BRAINBREW_CARGO_LOCK_SHA256,
        "profile": "release",
        "features": ["default", "workbench-write-dev"],
        "rustflags": [
            "-C",
            "debuginfo=0",
            "--remap-path-prefix=<source-root>=/brainbrew",
        ],
        "target": "x86_64-unknown-linux-gnu",
        "rustc": "rustc 1.95.0 (59807616e 2026-04-14)",
        "cargo": "cargo 1.95.0 (f2d3ce0bd 2026-03-21)",
    }


def _reviewed_generator() -> dict[str, Any]:
    source = _reviewed_generator_source()
    build = _reviewed_generator_build()
    identity_input = {"source": source, "build": build}
    return {
        **copy.deepcopy(_provenance()["brain_brew"]),
        "executable": {
            "algorithm": "sha256",
            "byte_count": PINNED_BRAINBREW_EXECUTABLE_BYTES,
            "sha256": PINNED_BRAINBREW_EXECUTABLE_SHA256,
        },
        "source": source,
        "build": build,
        "identity": {
            "algorithm": GENERATOR_IDENTITY_ALGORITHM,
            "sha256": hashlib.sha256(_canonical_json_bytes(identity_input)).hexdigest(),
        },
    }


def brainbrew_source_metadata(source_root: pathlib.Path) -> dict[str, Any]:
    metadata = selected_tree_metadata(
        source_root.resolve(),
        BRAINBREW_GENERATOR_SOURCE_PATHS,
        domain=GENERATOR_SOURCE_DOMAIN,
        algorithm=GENERATOR_SOURCE_ALGORITHM,
        label="Brain Brew generator source",
    )
    metadata["paths"] = list(BRAINBREW_GENERATOR_SOURCE_PATHS)
    return metadata


def _validate_brainbrew_source(source_root: pathlib.Path) -> dict[str, Any]:
    actual = brainbrew_source_metadata(source_root)
    reviewed = _reviewed_generator_source()
    if actual != reviewed:
        raise FixtureError(
            "Brain Brew generator source identity mismatch: "
            f"expected {reviewed['sha256']} ({reviewed['file_count']} files), "
            f"actual {actual['sha256']} ({actual['file_count']} files)"
        )
    cargo_lock = _file_metadata(source_root.resolve() / "Cargo.lock")
    if cargo_lock["sha256"] != PINNED_BRAINBREW_CARGO_LOCK_SHA256:
        raise FixtureError(
            "Brain Brew generator Cargo.lock digest mismatch: "
            f"expected {PINNED_BRAINBREW_CARGO_LOCK_SHA256}, actual {cargo_lock['sha256']}"
        )
    return actual


def _validate_generator_record(record: Mapping[str, Any]) -> None:
    if record != _reviewed_generator():
        raise FixtureError(
            "expected-output generator identity drifted from the reviewed executable/source/build pin"
        )


def _hardcore_provenance() -> dict[str, Any]:
    return {
        "repository": PINNED_HARDCORE_REPOSITORY,
        "ref": PINNED_HARDCORE_REF,
        "revision": PINNED_HARDCORE_REVISION,
    }


def _reviewed_attribution_coverage() -> dict[str, Any]:
    return {
        "algorithm": ATTRIBUTION_COVERAGE_ALGORITHM,
        "filename_normalization": ATTRIBUTION_FILENAME_NORMALIZATION,
        "media_file_count": 609,
        "image_file_count": 604,
        "unattributed_file_count": 0,
        "ambiguous_file_count": 0,
        "ultimate_geography": {
            "sources_csv_file_count": 548,
            "license_notice_file_count": 5,
            "attributed_file_count": 553,
            "flag_file_count": 227,
            "map_file_count": 321,
        },
        "hardcore_geography": {
            "sources_csv_file_count": 56,
            "attributed_file_count": 56,
            "flag_file_count": 39,
            "map_file_count": 17,
        },
        "sha256": PINNED_ATTRIBUTION_COVERAGE_SHA256,
    }


def _reviewed_hardcore_attribution_supplement() -> dict[str, Any]:
    return {
        "root": "/".join(HARDCORE_ATTRIBUTION_ROOT_PARTS),
        "provenance": _hardcore_provenance(),
        "algorithm": TREE_ALGORITHM,
        "file_count": 2,
        "byte_count": 6_126,
        "sha256": PINNED_HARDCORE_ATTRIBUTION_SHA256,
        "paths": list(HARDCORE_ATTRIBUTION_FILES),
        "files": [
            {
                "path": "README.md",
                "algorithm": "sha256",
                "byte_count": 42,
                "sha256": PINNED_HARDCORE_README_SHA256,
            },
            {
                "path": "sources.csv",
                "algorithm": "sha256",
                "byte_count": 6_084,
                "sha256": PINNED_HARDCORE_SOURCES_SHA256,
            },
        ],
    }


def _reviewed_attribution() -> dict[str, Any]:
    return {
        "coverage": _reviewed_attribution_coverage(),
        "supplements": {
            "hardcore_geography": _reviewed_hardcore_attribution_supplement()
        },
    }


def _provenance() -> dict[str, Any]:
    return {
        "ultimate_geography": {
            "repository": PINNED_UG_REPOSITORY,
            "ref": PINNED_UG_REF,
            "revision": PINNED_UG_REVISION,
        },
        "hardcore_geography": _hardcore_provenance(),
        "brain_brew": {
            "repository": PINNED_BRAINBREW_REPOSITORY,
            "ref": PINNED_BRAINBREW_REF,
            "revision": PINNED_BRAINBREW_REVISION,
            "version": PINNED_BRAINBREW_VERSION,
        },
    }


def _new_lock() -> dict[str, Any]:
    return {
        "schema_version": SCHEMA_VERSION,
        "provenance": _provenance(),
        "attribution": _reviewed_attribution(),
        "source": {},
        "expected": {
            "root": "/".join(EXPECTED_ROOT_PARTS),
            "accepted_from_source_sha256": None,
            "generated_by": _reviewed_generator(),
            "algorithm": JSON_TREE_ALGORITHM,
            "file_count": 0,
            "canonical_byte_count": 0,
            "sha256": None,
            "manifests": [],
        },
    }


def _validate_provenance(lock: Mapping[str, Any]) -> None:
    if lock.get("schema_version") != SCHEMA_VERSION:
        raise FixtureError(
            f"unsupported fixture lock schema {lock.get('schema_version')!r}; expected {SCHEMA_VERSION}"
        )
    if lock.get("provenance") != _provenance():
        raise FixtureError(
            "fixture provenance drifted from the reviewed UG/Hardcore/Brain Brew pins"
        )


def _intentional_exclusions() -> list[dict[str, Any]]:
    return [
        {
            "paths": [
                ".git/",
                ".gitignore",
                ".jj/",
                ".github/",
                ".frontloop/",
                ".agentleman/",
                ".editorconfig",
                ".pi-scratch.md",
                "build/",
                "scratch/",
            ],
            "reason": "VCS, CI, planning, cache, and generated-output state are not Brain Brew canonical inputs.",
        },
        {
            "paths": [
                "CONTRIBUTING.md",
                "README.md",
                "doc/",
                "docs/",
                "scripts/",
                "src/",
                "*.xlsx",
            ],
            "reason": "Repository documentation, legacy/build tooling, and spreadsheet maintainer sources are not read by the pinned Brain Brew manifests; LICENSE.md and sources.csv are retained for asset licensing and attribution.",
        },
    ]


def _validate_source_record_contract(source: Mapping[str, Any]) -> None:
    if source.get("root") != SOURCE_ROOT_NAME:
        raise FixtureError("fixture lock source root is invalid")
    if source.get("whitelist") != list(SOURCE_WHITELIST):
        raise FixtureError("fixture source whitelist drifted from the reviewed contract")
    if source.get("intentional_exclusions") != _intentional_exclusions():
        raise FixtureError(
            "fixture source intentional-exclusion provenance drifted from the reviewed contract"
        )


def _source_lock_record(source_root: pathlib.Path) -> dict[str, Any]:
    metadata = source_tree_metadata(source_root, exact=True)
    metadata.update(
        {
            "root": SOURCE_ROOT_NAME,
            "whitelist": list(SOURCE_WHITELIST),
            "intentional_exclusions": _intentional_exclusions(),
            "media": _tree_record(source_root / "media"),
            "goldens": _tree_record(source_root / "goldens"),
            "third_party_attribution": {
                **selected_tree_metadata(
                    source_root,
                    ATTRIBUTION_FILES,
                    label="UG third-party attribution",
                ),
                "paths": list(ATTRIBUTION_FILES),
            },
            "manifests": _manifest_records(source_root),
        }
    )
    return metadata


def _copy_source_snapshot(checkout: pathlib.Path, destination: pathlib.Path) -> None:
    _source_files(checkout, require_exact_top_level=False)
    destination.mkdir(parents=True)
    for relative in SOURCE_WHITELIST:
        source = checkout / relative
        target = destination / relative
        if source.is_dir():
            shutil.copytree(source, target, symlinks=False)
        else:
            shutil.copy2(source, target)
    # Re-scan the copy so a race or unusual filesystem object fails before publish.
    _source_files(destination, require_exact_top_level=True)


def _replace_directory_and_lock(
    destination: pathlib.Path,
    staged: pathlib.Path,
    lock_path: pathlib.Path,
    staged_lock: pathlib.Path,
) -> None:
    backup = destination.parent / f".{destination.name}.backup-{uuid.uuid4().hex}"
    had_destination = destination.exists()
    if had_destination:
        os.replace(destination, backup)
    try:
        os.replace(staged, destination)
        os.replace(staged_lock, lock_path)
    except BaseException:
        if destination.exists():
            shutil.rmtree(destination)
        if had_destination and backup.exists():
            os.replace(backup, destination)
        raise
    finally:
        staged_lock.unlink(missing_ok=True)
    if backup.exists():
        shutil.rmtree(backup)


def sync_source(
    repo_root: pathlib.Path, checkout: pathlib.Path, ug_revision: str
) -> dict[str, Any]:
    if ug_revision != PINNED_UG_REVISION:
        raise FixtureError(
            f"this reviewed fixture pins UG {PINNED_UG_REVISION}; got {ug_revision}. "
            "Changing the pin requires an explicit fixture-contract change."
        )
    repo_root = repo_root.resolve()
    checkout = checkout.resolve()
    fixture_root = repo_root / "fixtures" / SOURCE_ROOT_NAME
    lock_path = repo_root / "fixtures" / LOCK_NAME
    fixtures_dir = fixture_root.parent
    fixtures_dir.mkdir(parents=True, exist_ok=True)
    stage_parent = pathlib.Path(
        tempfile.mkdtemp(prefix=f".{SOURCE_ROOT_NAME}.sync-", dir=fixtures_dir)
    )
    staged_source = stage_parent / SOURCE_ROOT_NAME
    try:
        _copy_source_snapshot(checkout, staged_source)
        lock = _load_lock(lock_path) if lock_path.exists() else _new_lock()
        if lock_path.exists():
            _validate_provenance(lock)
            if lock.get("attribution") != _reviewed_attribution():
                raise FixtureError(
                    "ordinary UG source sync cannot change or remove the separately pinned "
                    "Hardcore Geography attribution supplement contract"
                )
        lock["schema_version"] = SCHEMA_VERSION
        lock["provenance"] = _provenance()
        lock["source"] = _source_lock_record(staged_source)
        staged_lock = _write_json_temp(lock_path, lock)
        _replace_directory_and_lock(
            fixture_root, staged_source, lock_path, staged_lock
        )
    finally:
        if stage_parent.exists():
            shutil.rmtree(stage_parent)
    return lock


def _validate_recorded_tree(
    root: pathlib.Path, record: Mapping[str, Any], label: str
) -> None:
    require_tree_metadata(root, record, label)


def _validate_attribution_record(
    source_root: pathlib.Path, record: Mapping[str, Any]
) -> None:
    if record.get("paths") != list(ATTRIBUTION_FILES):
        raise FixtureError("UG third-party attribution path contract drifted")
    actual = selected_tree_metadata(
        source_root, ATTRIBUTION_FILES, label="UG third-party attribution"
    )
    for field in ("algorithm", "file_count", "byte_count", "sha256"):
        if actual[field] != record.get(field):
            raise FixtureError(
                f"UG third-party attribution {field} drift: "
                f"expected {record.get(field)!r}, actual {actual[field]!r}"
            )


def _hardcore_attribution_root(repo_root: pathlib.Path) -> pathlib.Path:
    root = repo_root.resolve() / "fixtures"
    for component in HARDCORE_ATTRIBUTION_ROOT_PARTS:
        root /= component
    return root


def _validate_hardcore_attribution_supplement(
    repo_root: pathlib.Path, record: Mapping[str, Any]
) -> None:
    reviewed = _reviewed_hardcore_attribution_supplement()
    if record != reviewed:
        raise FixtureError(
            "Hardcore Geography attribution supplement lock drifted from the reviewed pin"
        )
    supplement_root = _hardcore_attribution_root(repo_root)
    actual_paths = [relative for relative, _ in _regular_files(supplement_root)]
    if actual_paths != list(HARDCORE_ATTRIBUTION_FILES):
        raise FixtureError(
            "Hardcore Geography attribution supplement inventory drift: "
            f"expected {list(HARDCORE_ATTRIBUTION_FILES)!r}, actual {actual_paths!r}"
        )
    actual_tree = tree_metadata(supplement_root)
    for field in ("algorithm", "file_count", "byte_count", "sha256"):
        if actual_tree[field] != record.get(field):
            raise FixtureError(
                f"Hardcore Geography attribution supplement {field} drift: "
                f"expected {record.get(field)!r}, actual {actual_tree[field]!r}"
            )
    file_records = record.get("files")
    if not isinstance(file_records, list):
        raise FixtureError(
            "Hardcore Geography attribution supplement file records are missing"
        )
    for file_record in file_records:
        if not isinstance(file_record, dict) or not isinstance(
            file_record.get("path"), str
        ):
            raise FixtureError(
                "Hardcore Geography attribution supplement file record is malformed"
            )
        path = supplement_root / file_record["path"]
        actual_file = _file_metadata(path)
        for field in ("algorithm", "byte_count", "sha256"):
            if actual_file[field] != file_record.get(field):
                raise FixtureError(
                    f"Hardcore Geography attribution {file_record['path']} {field} drift: "
                    f"expected {file_record.get(field)!r}, actual {actual_file[field]!r}"
                )


def _validate_attribution_state(
    repo_root: pathlib.Path,
    fixture_root: pathlib.Path,
    record: Mapping[str, Any],
) -> None:
    if record != _reviewed_attribution():
        raise FixtureError("fixture attribution lock drifted from the reviewed contract")
    supplements = record.get("supplements")
    coverage_record = record.get("coverage")
    if not isinstance(supplements, dict) or not isinstance(coverage_record, dict):
        raise FixtureError("fixture attribution supplement/coverage records are missing")
    hardcore_record = supplements.get("hardcore_geography")
    if not isinstance(hardcore_record, dict):
        raise FixtureError("Hardcore Geography attribution supplement record is missing")
    _validate_hardcore_attribution_supplement(repo_root, hardcore_record)
    actual_coverage = attribution_coverage_metadata(
        fixture_root / "media",
        fixture_root / "sources.csv",
        _hardcore_attribution_root(repo_root) / "sources.csv",
    )
    if actual_coverage != coverage_record:
        raise FixtureError(
            "fixture media attribution coverage differs from the reviewed lock record"
        )


def validate_fixture_state(repo_root: pathlib.Path) -> dict[str, Any]:
    repo_root = repo_root.resolve()
    fixture_root = repo_root / "fixtures" / SOURCE_ROOT_NAME
    expected_root = repo_root / "fixtures"
    for component in EXPECTED_ROOT_PARTS:
        expected_root /= component
    lock_path = repo_root / "fixtures" / LOCK_NAME
    lock = _load_lock(lock_path)
    _validate_provenance(lock)
    source = lock.get("source")
    expected = lock.get("expected")
    attribution = lock.get("attribution")
    if (
        not isinstance(source, dict)
        or not isinstance(expected, dict)
        or not isinstance(attribution, dict)
    ):
        raise FixtureError("fixture lock must contain source, expected, and attribution objects")
    _validate_source_record_contract(source)
    _require_source_metadata(fixture_root, source)
    media_record = source.get("media")
    golden_record = source.get("goldens")
    attribution_record = source.get("third_party_attribution")
    if (
        not isinstance(media_record, dict)
        or not isinstance(golden_record, dict)
        or not isinstance(attribution_record, dict)
    ):
        raise FixtureError("fixture lock media/goldens/attribution records are missing")
    _validate_recorded_tree(fixture_root / "media", media_record, "source media tree")
    _validate_recorded_tree(fixture_root / "goldens", golden_record, "source UG goldens tree")
    _validate_attribution_record(fixture_root, attribution_record)
    _validate_attribution_state(repo_root, fixture_root, attribution)

    actual_records = _manifest_records(fixture_root)
    if source.get("manifests") != actual_records:
        raise FixtureError("source manifest target mapping drifted from the fixture lock")
    source_mapping = _mapping_from_records(actual_records)

    if expected.get("root") != "/".join(EXPECTED_ROOT_PARTS):
        raise FixtureError("fixture lock expected-output root is invalid")
    if expected.get("accepted_from_source_sha256") != source.get("sha256"):
        raise FixtureError(
            "expected outputs were not accepted from the current source snapshot; "
            "run the explicit --accept-expected boundary"
        )
    generated_by = expected.get("generated_by")
    if not isinstance(generated_by, dict):
        raise FixtureError("expected-output generator provenance is missing")
    _validate_generator_record(generated_by)
    expected_records = expected.get("manifests")
    if expected_records != actual_records:
        raise FixtureError("expected target-to-manifest mapping drifted from current source")
    expected_mapping = _mapping_from_records(expected_records)
    if expected_mapping != source_mapping:
        raise FixtureError("source and expected target mappings disagree")
    validate_expected_tree(expected_root, expected_mapping, expected)
    return lock


def compare_source_checkout(
    repo_root: pathlib.Path,
    checkout: pathlib.Path,
    ug_revision: str,
    lock: Mapping[str, Any],
) -> None:
    if ug_revision != PINNED_UG_REVISION:
        raise FixtureError(
            f"UG revision attestation mismatch: expected {PINNED_UG_REVISION}, got {ug_revision}"
        )
    checkout = checkout.resolve()
    fixture_root = repo_root.resolve() / "fixtures" / SOURCE_ROOT_NAME
    checkout_files = _source_files(checkout, require_exact_top_level=False)
    fixture_files = _source_files(fixture_root, require_exact_top_level=True)
    checkout_names = [relative for relative, _ in checkout_files]
    fixture_names = [relative for relative, _ in fixture_files]
    if checkout_names != fixture_names:
        raise FixtureError("UG checkout whitelist inventory differs from the vendored source snapshot")
    for (relative, checkout_path), (_, fixture_path) in zip(
        checkout_files, fixture_files, strict=True
    ):
        if checkout_path.read_bytes() != fixture_path.read_bytes():
            raise FixtureError(
                f"UG checkout source drift at {relative}: vendored bytes do not match"
            )
    checkout_metadata = source_tree_metadata(checkout, exact=False)
    source_record = lock.get("source", {})
    if checkout_metadata.get("sha256") != source_record.get("sha256"):
        raise FixtureError("UG checkout digest differs from the pinned source record")


def _brainbrew_binary(repo_root: pathlib.Path, explicit: str | None) -> pathlib.Path:
    raw = explicit or os.environ.get("BRAINBREW_BIN")
    path = pathlib.Path(raw) if raw else repo_root / "target" / "debug" / "brainbrew"
    path = path.expanduser().resolve()
    if not path.is_file() or not os.access(path, os.X_OK):
        raise FixtureError(
            f"pinned Brain Brew binary is not executable: {path}. "
            "Build it offline first or pass --brainbrew-bin/BRAINBREW_BIN."
        )
    return path


def _verify_brainbrew_binary(
    binary: pathlib.Path,
    brainbrew_revision: str,
    source_root: pathlib.Path,
) -> None:
    if brainbrew_revision != PINNED_BRAINBREW_REVISION:
        raise FixtureError(
            "Brain Brew revision attestation mismatch: "
            f"expected {PINNED_BRAINBREW_REVISION}, got {brainbrew_revision}"
        )
    executable = _file_metadata(binary)
    expected_executable = _reviewed_generator()["executable"]
    if executable != expected_executable:
        raise FixtureError(
            "Brain Brew executable digest mismatch: "
            f"expected {expected_executable['sha256']} ({expected_executable['byte_count']} bytes), "
            f"actual {executable['sha256']} ({executable['byte_count']} bytes)"
        )
    _validate_brainbrew_source(source_root)
    result = subprocess.run(
        [str(binary), "--version"],
        check=False,
        capture_output=True,
        text=True,
        timeout=30,
        env={**os.environ, "BRAINBREW_COLOR": "never", "NO_COLOR": "1"},
    )
    expected = f"brainbrew {PINNED_BRAINBREW_VERSION}"
    if result.returncode != 0 or result.stdout.strip() != expected:
        raise FixtureError(
            f"Brain Brew binary version mismatch: expected {expected!r}, "
            f"got stdout={result.stdout.strip()!r}, stderr={result.stderr.strip()!r}"
        )


def _generate_outputs(
    source_root: pathlib.Path,
    records: Sequence[Mapping[str, Any]],
    binary: pathlib.Path,
) -> tuple[pathlib.Path, tempfile.TemporaryDirectory[str], dict[str, Any]]:
    temporary = tempfile.TemporaryDirectory(prefix="brainbrew-ug-expected-")
    temp_root = pathlib.Path(temporary.name)
    workspace = temp_root / "workspace"
    shutil.copytree(source_root, workspace)
    generated = temp_root / "expected" / "crowdanki"
    generated.mkdir(parents=True)
    values: dict[str, Any] = {}
    target_rows = [
        (str(record["path"]), str(target))
        for record in records
        for target in record["targets"]
    ]
    environment = {
        **os.environ,
        "BRAINBREW_COLOR": "never",
        "NO_COLOR": "1",
        "CARGO_NET_OFFLINE": "true",
    }
    for index, (manifest_path, target) in enumerate(target_rows, start=1):
        output = workspace / "build" / "fixture-expected" / target
        command = [
            str(binary),
            "export",
            "crowdanki",
            "--manifest",
            str(workspace / manifest_path),
            "--target",
            target,
            "--out",
            str(output),
            "--media-mode",
            "reference-only",
        ]
        result = subprocess.run(
            command,
            cwd=workspace,
            check=False,
            capture_output=True,
            text=True,
            timeout=180,
            env=environment,
        )
        if result.returncode != 0:
            raise FixtureError(
                f"Brain Brew failed while generating {manifest_path}:{target} "
                f"(exit {result.returncode}):\n{result.stdout}\n{result.stderr}"
            )
        children = sorted(path.name for path in output.iterdir()) if output.is_dir() else []
        if children != ["deck.json"]:
            raise FixtureError(
                f"reference-only export for {target} produced {children!r}, expected only deck.json"
            )
        source_json = output / "deck.json"
        try:
            value = json.loads(source_json.read_text(encoding="utf-8"))
        except (OSError, UnicodeError, json.JSONDecodeError) as error:
            raise FixtureError(f"generated output for {target} is invalid JSON: {error}") from error
        target_dir = generated / target
        target_dir.mkdir()
        shutil.copy2(source_json, target_dir / "deck.json")
        values[target] = value
        if index % 10 == 0 or index == len(target_rows):
            print(
                f"generated {index}/{len(target_rows)} pinned UG expected outputs",
                file=sys.stderr,
            )
    if len(values) != TOTAL_TARGETS:
        raise FixtureError(
            f"generator produced {len(values)} distinct targets, expected {TOTAL_TARGETS}"
        )
    return generated, temporary, values


def accept_expected(
    repo_root: pathlib.Path,
    binary_path: str | None,
    brainbrew_revision: str,
    brainbrew_source_root: pathlib.Path,
) -> dict[str, Any]:
    repo_root = repo_root.resolve()
    lock = validate_source_only(repo_root)
    binary = _brainbrew_binary(repo_root, binary_path)
    _verify_brainbrew_binary(binary, brainbrew_revision, brainbrew_source_root)
    source_root = repo_root / "fixtures" / SOURCE_ROOT_NAME
    source_records = lock["source"]["manifests"]
    generated, temporary, _ = _generate_outputs(source_root, source_records, binary)
    try:
        mapping = _mapping_from_records(source_records)
        metadata = json_tree_metadata(generated)
        # Validate the complete generated tree before changing either destination.
        validate_expected_tree(generated, mapping, metadata)
        new_lock = copy.deepcopy(lock)
        new_lock["expected"] = {
            "root": "/".join(EXPECTED_ROOT_PARTS),
            "accepted_from_source_sha256": lock["source"]["sha256"],
            "generated_by": _reviewed_generator(),
            **metadata,
            "manifests": copy.deepcopy(source_records),
        }
        expected_root = repo_root / "fixtures"
        for component in EXPECTED_ROOT_PARTS:
            expected_root /= component
        expected_root.parent.mkdir(parents=True, exist_ok=True)
        publish_stage_parent = pathlib.Path(
            tempfile.mkdtemp(
                prefix=".crowdanki.accept-", dir=expected_root.parent
            )
        )
        staged_expected = publish_stage_parent / "crowdanki"
        try:
            # Generation uses the system temporary directory. Copy once to a
            # same-filesystem staging directory so publication remains an
            # atomic rename even when /tmp is a different filesystem.
            shutil.copytree(generated, staged_expected)
            staged_lock = _write_json_temp(repo_root / "fixtures" / LOCK_NAME, new_lock)
            _replace_directory_and_lock(
                expected_root,
                staged_expected,
                repo_root / "fixtures" / LOCK_NAME,
                staged_lock,
            )
        finally:
            if publish_stage_parent.exists():
                shutil.rmtree(publish_stage_parent)
    finally:
        temporary.cleanup()
    return new_lock


def validate_source_only(repo_root: pathlib.Path) -> dict[str, Any]:
    repo_root = repo_root.resolve()
    lock_path = repo_root / "fixtures" / LOCK_NAME
    lock = _load_lock(lock_path)
    _validate_provenance(lock)
    source = lock.get("source")
    attribution = lock.get("attribution")
    if not isinstance(source, dict) or not isinstance(attribution, dict):
        raise FixtureError("fixture lock source/attribution records are missing")
    _validate_source_record_contract(source)
    fixture_root = repo_root / "fixtures" / SOURCE_ROOT_NAME
    _require_source_metadata(fixture_root, source)
    records = _manifest_records(fixture_root)
    if source.get("manifests") != records:
        raise FixtureError("source manifest target mapping drifted from fixture lock")
    media_record = source.get("media")
    golden_record = source.get("goldens")
    attribution_record = source.get("third_party_attribution")
    if (
        not isinstance(media_record, dict)
        or not isinstance(golden_record, dict)
        or not isinstance(attribution_record, dict)
    ):
        raise FixtureError("fixture lock media/goldens/attribution records are missing")
    _validate_recorded_tree(fixture_root / "media", media_record, "source media tree")
    _validate_recorded_tree(fixture_root / "goldens", golden_record, "source UG goldens tree")
    _validate_attribution_record(fixture_root, attribution_record)
    _validate_attribution_state(repo_root, fixture_root, attribution)
    return lock


def check_fixture(
    repo_root: pathlib.Path,
    binary_path: str | None,
    brainbrew_revision: str,
    brainbrew_source_root: pathlib.Path,
    checkout: pathlib.Path | None,
    ug_revision: str | None,
) -> dict[str, Any]:
    repo_root = repo_root.resolve()
    lock = validate_fixture_state(repo_root)
    if checkout is not None:
        if ug_revision is None:
            raise FixtureError("--check with --ug-checkout requires --ug-revision")
        compare_source_checkout(repo_root, checkout, ug_revision, lock)
    elif ug_revision is not None:
        raise FixtureError("--ug-revision requires --ug-checkout in --check mode")
    binary = _brainbrew_binary(repo_root, binary_path)
    _verify_brainbrew_binary(binary, brainbrew_revision, brainbrew_source_root)
    source_root = repo_root / "fixtures" / SOURCE_ROOT_NAME
    records = lock["source"]["manifests"]
    generated, temporary, generated_values = _generate_outputs(source_root, records, binary)
    try:
        expected_root = repo_root / "fixtures"
        for component in EXPECTED_ROOT_PARTS:
            expected_root /= component
        for target in sorted(generated_values):
            expected_path = expected_root / target / "deck.json"
            expected_value = json.loads(expected_path.read_text(encoding="utf-8"))
            if generated_values[target] != expected_value:
                raise FixtureError(
                    f"generated output drift for {target}: parsed deck.json differs from accepted output"
                )
        generated_metadata = json_tree_metadata(generated)
        expected_record = lock["expected"]
        if generated_metadata != {
            field: expected_record[field]
            for field in ("algorithm", "file_count", "canonical_byte_count", "sha256")
        }:
            raise FixtureError(
                "generated output semantic digest differs from the accepted expected-output digest"
            )
    finally:
        temporary.cleanup()
    return lock


def _repo_root_from_script() -> pathlib.Path:
    return pathlib.Path(__file__).resolve().parent.parent


def _revision(value: str) -> str:
    if not REVISION_RE.fullmatch(value):
        raise argparse.ArgumentTypeError("revision must be exactly 40 lowercase hexadecimal characters")
    return value


def _parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(
        description="Synchronize, explicitly accept, or read-only check the pinned Ultimate Geography fixture."
    )
    modes = parser.add_mutually_exclusive_group()
    modes.add_argument(
        "--sync-source",
        action="store_true",
        help="copy only the exact whitelisted UG source snapshot (default mode)",
    )
    modes.add_argument(
        "--accept-expected",
        action="store_true",
        help="explicitly regenerate and accept all 100 parsed expected deck.json outputs",
    )
    modes.add_argument(
        "--check",
        action="store_true",
        help="read-only source/target/expected/generated-output drift check",
    )
    parser.add_argument(
        "checkout",
        nargs="?",
        type=pathlib.Path,
        help="legacy positional spelling for --ug-checkout",
    )
    parser.add_argument("--ug-checkout", type=pathlib.Path)
    parser.add_argument("--ug-revision", type=_revision)
    parser.add_argument("--brainbrew-bin")
    parser.add_argument("--brainbrew-revision", type=_revision)
    parser.add_argument(
        "--brainbrew-source-root",
        type=pathlib.Path,
        help="checkout/root containing the reviewed generator source inputs",
    )
    parser.add_argument(
        "--repo-root",
        type=pathlib.Path,
        default=_repo_root_from_script(),
        help=argparse.SUPPRESS,
    )
    return parser


def main(argv: Sequence[str] | None = None) -> int:
    args = _parser().parse_args(argv)
    checkout = args.ug_checkout or args.checkout
    if args.ug_checkout is not None and args.checkout is not None:
        raise FixtureError("pass the UG checkout either positionally or with --ug-checkout, not both")
    if args.accept_expected:
        if checkout is not None or args.ug_revision is not None:
            raise FixtureError("--accept-expected operates on the pinned vendored source; do not pass UG checkout options")
        if args.brainbrew_revision is None or args.brainbrew_source_root is None:
            raise FixtureError(
                "--accept-expected requires --brainbrew-revision and --brainbrew-source-root"
            )
        lock = accept_expected(
            args.repo_root,
            args.brainbrew_bin,
            args.brainbrew_revision,
            args.brainbrew_source_root,
        )
        print(
            "Accepted 100 pinned UG expected CrowdAnki outputs\n"
            f"source sha256: {lock['source']['sha256']}\n"
            f"expected semantic sha256: {lock['expected']['sha256']}"
        )
        return 0
    if args.check:
        if args.brainbrew_revision is None or args.brainbrew_source_root is None:
            raise FixtureError(
                "--check requires --brainbrew-revision and --brainbrew-source-root"
            )
        lock = check_fixture(
            args.repo_root,
            args.brainbrew_bin,
            args.brainbrew_revision,
            args.brainbrew_source_root,
            checkout,
            args.ug_revision,
        )
        print(
            "Pinned UG fixture check passed (read-only)\n"
            f"source files: {lock['source']['file_count']}\n"
            f"source sha256: {lock['source']['sha256']}\n"
            f"expected files: {lock['expected']['file_count']}\n"
            f"expected semantic sha256: {lock['expected']['sha256']}"
        )
        return 0
    if (
        args.brainbrew_revision is not None
        or args.brainbrew_bin is not None
        or args.brainbrew_source_root is not None
    ):
        raise FixtureError("Brain Brew options apply only to --accept-expected or --check")
    if checkout is None or args.ug_revision is None:
        raise FixtureError("source sync requires --ug-checkout (or positional checkout) and --ug-revision")
    lock = sync_source(args.repo_root, checkout, args.ug_revision)
    stale = lock.get("expected", {}).get("accepted_from_source_sha256") != lock["source"]["sha256"]
    print(
        "Refreshed the pinned UG source snapshot only; expected outputs were not changed\n"
        f"source files: {lock['source']['file_count']}\n"
        f"source sha256: {lock['source']['sha256']}\n"
        f"expected outputs stale: {'yes' if stale else 'no'}"
    )
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except FixtureError as error:
        print(f"error: {error}", file=sys.stderr)
        raise SystemExit(1)
