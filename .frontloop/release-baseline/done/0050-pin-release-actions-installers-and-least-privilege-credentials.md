---
title: Pin release actions installers and least-privilege credentials
priority: high
---

## Goal

Remove mutable third-party execution and broad publication credentials from the release path.

## Acceptance Criteria

- GitHub Actions are pinned to reviewed commit SHAs
- Downloaded installers are pinned and checksum-verified or built from a locked dependency
- Job permissions are minimized and publication tokens are isolated to host jobs
- Homebrew writes cannot run on untrusted pull-request code
- Renovation instructions exist for intentional pin updates

## Implementation Notes

Apply after the release workflow dependency graph is explicit.


## Completion Summary

- Pinned all external GitHub Actions to reviewed full commit SHAs with version/provenance comments
- Replaced mutable cargo-dist/rustup bootstrap behavior with checksum-verified local cargo-dist installer
- Set default contents:read permissions, isolated contents:write and GH_TOKEN to final host commands, and kept PR/reusable quality tokenless/read-only
- Removed Homebrew/tap/PAT release paths and enforced their absence
- Added fail-closed release security policy checker and regression tests covering actions, installer, Nix locks, workflow privileges, and banned patterns
- Added Dependabot configuration and documented manual pin review/update process
- Passed security-policy tests, full tests, fmt/clippy, docs, Nix package/smoke, 13 E2E scenarios, parallelism check, and Claude judgment

### Files Changed

- .github/workflows/release.yml
- .github/workflows/reusable-quality.yml
- .github/dependabot.yml
- scripts/install_cargo_dist.sh
- scripts/check_release_security.py
- scripts/tests/test_release_security_policy.py
- devenv.nix
- README.md
- documentation/docs/reference/release-security.md
- documentation/docs/reference/releasing.md
- documentation/docs/getting-started/install.md
