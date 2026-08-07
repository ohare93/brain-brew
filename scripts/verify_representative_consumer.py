#!/usr/bin/env python3
"""Verify an explicitly supplied live-consumer evidence contract.

A checked-in fixture is deliberately not a representative consumer.  Release
callers must supply a separately produced evidence JSON and the SHA-256 that
binds the downloaded bytes.  Until the live Ultimate Geography integration
exists, `--required` fails closed while ordinary PR callers record `blocked`.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import sys
from pathlib import Path
from urllib.request import urlopen


class ContractError(RuntimeError):
    pass


def sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as file:
        for block in iter(lambda: file.read(1024 * 1024), b""):
            digest.update(block)
    return digest.hexdigest()


def required_text(value: object, name: str) -> str:
    if not isinstance(value, str) or not value.strip():
        raise ContractError(f"representative-consumer evidence requires non-empty {name}")
    return value


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--target-sha", required=True)
    parser.add_argument("--url", default="")
    parser.add_argument("--sha256", default="")
    parser.add_argument("--required", action="store_true")
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    result: dict[str, str] = {"target_sha": args.target_sha, "status": "blocked"}
    try:
        if not args.url or not args.sha256:
            raise ContractError("live representative-consumer evidence URL and SHA-256 have not been configured")
        if len(args.sha256) != 64 or any(character not in "0123456789abcdef" for character in args.sha256):
            raise ContractError("representative-consumer evidence SHA-256 must be 64 lowercase hexadecimal characters")
        if not args.url.startswith("https://"):
            raise ContractError("representative-consumer evidence URL must use HTTPS")
        with urlopen(args.url, timeout=30) as response:
            args.output.write_bytes(response.read())
        actual = sha256(args.output)
        if actual != args.sha256:
            raise ContractError("representative-consumer evidence SHA-256 does not match supplied bytes")
        evidence = json.loads(args.output.read_text(encoding="utf-8"))
        if not isinstance(evidence, dict):
            raise ContractError("representative-consumer evidence must be a JSON object")
        if evidence.get("schema_version") != 1 or evidence.get("status") != "passed":
            raise ContractError("representative-consumer evidence is not a passing schema version 1 result")
        if evidence.get("target_sha") != args.target_sha:
            raise ContractError("representative-consumer evidence is bound to a different commit SHA")
        if evidence.get("consumer") != "ultimate-geography-live":
            raise ContractError("representative-consumer evidence must name ultimate-geography-live, not a fixture")
        required_text(evidence.get("artifact_sha256"), "artifact_sha256")
        required_text(evidence.get("commands"), "commands")
        result.update({"status": "passed", "evidence_sha256": actual})
    except (ContractError, OSError, ValueError) as error:
        result["reason"] = str(error)
        args.output.write_text(json.dumps(result, indent=2) + "\n", encoding="utf-8")
        print(f"representative-consumer: blocked: {error}", file=sys.stderr)
        if args.required:
            return 1
    print(json.dumps(result, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
