#!/usr/bin/env python3
"""Fetch Ultimate Geography release CrowdAnki deck.json files as a parity oracle.

This is intentionally a small, project-specific helper for the UG migration proof.
It is not a general Brain Brew package downloader.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import sys
import urllib.request
import zipfile
from pathlib import Path
from tempfile import TemporaryDirectory

REPO = "anki-geo/ultimate-geography"
DEFAULT_TAG = "v5.3"
LANGUAGES = [
    "cs",
    "da",
    "de",
    "en",
    "es",
    "fr",
    "it",
    "nb",
    "nl",
    "pl",
    "pt",
    "ru",
    "sv",
    "zh",
    "zh-tw",
]
VARIANTS = ["standard", "extended"]


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--tag", default=DEFAULT_TAG, help="UG release tag to fetch")
    parser.add_argument(
        "--out",
        type=Path,
        help="Output directory (default: .cache/brainbrew/ug-release-oracle/<tag>)",
    )
    parser.add_argument(
        "--target",
        action="append",
        help="Target to fetch, e.g. en-standard. Repeatable. Defaults to all release targets.",
    )
    parser.add_argument(
        "--force",
        action="store_true",
        help="Download even when the target deck.json already exists.",
    )
    args = parser.parse_args()

    out = args.out or Path(".cache/brainbrew/ug-release-oracle") / args.tag
    targets = args.target or all_targets()
    validate_targets(targets)

    records = {}
    crowdanki_root = out / "crowdanki"
    crowdanki_root.mkdir(parents=True, exist_ok=True)

    with TemporaryDirectory(prefix="brainbrew-ug-oracle-") as temp:
        temp_dir = Path(temp)
        for target in targets:
            spec = target_spec(args.tag, target)
            deck_json_path = crowdanki_root / spec["deck_folder"] / "deck.json"
            if deck_json_path.exists() and not args.force:
                deck_bytes = deck_json_path.read_bytes()
                records[target] = {
                    **spec,
                    "asset_sha256": None,
                    "deck_json_sha256": sha256_bytes(deck_bytes),
                    "deck_json": str(deck_json_path.relative_to(out)),
                    "downloaded": False,
                }
                print(f"already present {target}: {deck_json_path}")
                continue

            zip_path = temp_dir / spec["asset"]
            print(f"downloading {target}: {spec['url']}")
            urllib.request.urlretrieve(spec["url"], zip_path)
            asset_bytes = zip_path.read_bytes()
            asset_sha256 = sha256_bytes(asset_bytes)

            with zipfile.ZipFile(zip_path) as archive:
                member = f"{spec['deck_folder']}/deck.json"
                try:
                    deck_bytes = archive.read(member)
                except KeyError as error:
                    raise SystemExit(f"{spec['asset']} does not contain {member}") from error

            deck_json_path.parent.mkdir(parents=True, exist_ok=True)
            deck_json_path.write_bytes(deck_bytes)
            records[target] = {
                **spec,
                "asset_sha256": asset_sha256,
                "deck_json_sha256": sha256_bytes(deck_bytes),
                "deck_json": str(deck_json_path.relative_to(out)),
                "downloaded": True,
            }
            print(f"extracted {target}: {deck_json_path}")

    manifest = {
        "repo": REPO,
        "tag": args.tag,
        "targets": records,
    }
    manifest_path = out / "oracle-manifest.json"
    manifest_path.write_text(json.dumps(manifest, indent=2, sort_keys=True) + "\n")
    print(f"wrote {manifest_path}")
    return 0


def all_targets() -> list[str]:
    return [f"{language}-{variant}" for language in LANGUAGES for variant in VARIANTS]


def validate_targets(targets: list[str]) -> None:
    valid = set(all_targets())
    invalid = sorted(set(targets) - valid)
    if invalid:
        raise SystemExit(f"unknown target(s): {', '.join(invalid)}")


def target_spec(tag: str, target: str) -> dict[str, str]:
    language, variant = target_language_and_variant(target)
    release_code = language.upper()
    variant_suffix = "_EXTENDED" if variant == "extended" else ""
    folder_suffix = " [Extended]" if variant == "extended" else ""
    asset = f"Ultimate_Geography_{tag}_{release_code}{variant_suffix}.zip"
    deck_folder = f"Ultimate Geography [{release_code}]{folder_suffix}"
    return {
        "asset": asset,
        "url": f"https://github.com/{REPO}/releases/download/{tag}/{asset}",
        "deck_folder": deck_folder,
    }


def target_language_and_variant(target: str) -> tuple[str, str]:
    if target.startswith("zh-tw-"):
        return "zh-tw", target.removeprefix("zh-tw-")
    language, variant = target.split("-", 1)
    return language, variant


def sha256_bytes(value: bytes) -> str:
    return hashlib.sha256(value).hexdigest()


if __name__ == "__main__":
    sys.exit(main())
