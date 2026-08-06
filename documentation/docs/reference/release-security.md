---
title: Release supply-chain security
---

# Release supply-chain security

Release automation executes only reviewed, immutable inputs. Run the policy
checker before changing a workflow, release installer, Nix input, or publication
path:

```bash
python3 scripts/check_release_security.py
python3 -m unittest scripts.tests.test_release_security_policy
```

The checker deliberately has a small explicit pin map rather than a broad
allowlist. It scans every workflow YAML file and every shell script, rejects
mutable action tags, containers, pipe-to-shell installers, Rustup bootstrap,
`pull_request_target`, broad permissions, Homebrew/tap/PAT paths, and credentials
outside the release host. It also checks both Nix locks for a full revision and
NAR hash.

## Reviewed executable pins

| Input | Reviewed version | Immutable pin / integrity evidence | Provenance |
| --- | --- | --- | --- |
| `actions/checkout` | v6 | `df4cb1c069e1874edd31b4311f1884172cec0e10` | [upstream v6 ref](https://github.com/actions/checkout/tree/v6) |
| `actions/upload-artifact` | v6 | `b7c566a772e6b6bfb58ed0dc250532a479d7789f` | [upstream v6 ref](https://github.com/actions/upload-artifact/tree/v6) |
| `actions/download-artifact` | v7 | `37930b1c2abaa49bbe596cd826c3c89aef350131` | [upstream v7 ref](https://github.com/actions/download-artifact/tree/v7) |
| `cachix/install-nix-action` | v31 | `a49548c11d9846ad46ecc0115273879b045f001c` | [upstream v31 ref](https://github.com/cachix/install-nix-action/tree/v31) |
| cargo-dist | v0.30.4 | platform-specific SHA-256 values in `scripts/install_cargo_dist.sh` | [upstream v0.30.4 release](https://github.com/axodotdev/cargo-dist/releases/tag/v0.30.4) |
| Nix inputs | locked | full revisions plus NAR hashes in `flake.lock` and `devenv.lock` | checked-in locks |
| documentation npm dependencies | lockfile | `documentation/package-lock.json`; CI uses `npm ci`, audits production dependencies, and records installed license metadata | checked-in lock |
| GitHub artifact attestation | v3 | `977bb373ede98d70efdf65b84cb5f73e068dcc2a` | [upstream v3 ref](https://github.com/actions/attest-build-provenance/tree/v3) |
| cosign installer | v3.10.0 | `d7543c93d881b35a8faa02e8e3605f69b7a1ce62`; installs cosign v2.5.3 | [upstream v3.10.0 ref](https://github.com/sigstore/cosign-installer/tree/v3.10.0) |

`cachix/install-nix-action` is an annotated tag; its documented v31 tag was
resolved through GitHub's tag object to the commit above. The action pins and
cargo-dist release checksums were independently checked against the linked
upstream release/ref on 2026-07-11. Do not replace a SHA with a version tag.

The cargo-dist helper downloads one exact upstream archive for the runner
platform, verifies its hard-coded SHA-256 before extraction, and installs only
the verified `dist` executable. It replaces the generated curl installer and
Rustup bootstrap. Runner images must already provide Rust; failure is safer than
downloading a bootstrap toolchain. Generated cargo-dist matrix container and
package-installer expressions are deliberately not executed.

## External execution inventory

- Workflows execute only the six action repositories in the table, the
  repository's local reusable workflow, checked-out repository scripts, locked
  Nix/devenv packages, locked Cargo dependencies, and `npm ci` from the
  documentation lockfile. There are no Docker/container actions.
- `scripts/install_cargo_dist.sh` is the sole shell download of executable code;
  it is checksum-verified before extraction. `scripts/run_workbench_e2e.sh`
  uses curl only to probe its own `127.0.0.1` chromedriver process.
- `scripts/verify_representative_consumer.py` downloads evidence bytes only after
  callers provide an HTTPS URL and exact SHA-256; it parses the verified JSON but
  never executes it. `scripts/fetch_ug_release_oracle.py` downloads public UG
  ZIP data for an optional parity oracle, hashes it, and extracts `deck.json`; it
  never executes archive contents. It is not a release installer or gate.
- `scripts/publish_crates.sh` invokes `cargo publish` only after an explicit
  local `publish ... --yes`; it is never called from GitHub Actions. Cargo's
  workspace dependency graph is in `Cargo.lock`; registry publication remains a
  human credential boundary.

## Dependency policy, SBOM, and provenance

`supply-chain-policy.toml` is the single Cargo and production npm advisory/license
policy. `devenv shell supply-chain:check` runs `cargo audit --json`, `npm ci`,
`npm audit --omit=dev --json`, and a checked installed-package license inventory.
It fails closed for a missing report, an unknown production advisory/license, or
an exception without an exact ID, owner, expiry, and rationale. Development-only
findings are reported separately and cannot suppress production findings.

A lock refresh keeps the documentation build on Docusaurus 3.10.1 while selecting
patched compatible transitive releases. The remaining npm audit findings are the
three explicitly listed Docusaurus/Webpack advisory exceptions in that policy.
They are not suppressed: each is a `release-maintainers`-owned, 2026-08-15 review
item. The high `GHSA-5c6j-r48x-rmvq` remains blocked by that dated exception and
must not be renewed without reviewing an upstream-compatible Docusaurus update.
Legacy npm license metadata exceptions are equally exact and time-bound. Cargo
currently has no listed advisory or license exception.

Release smoke creates CycloneDX 1.5 SBOMs from the produced `.crate`, cargo-dist,
and Nix binary bytes. Every record carries the observed SHA-256. Release builders
create `SHA256SUMS` and in-toto/SLSA provenance records binding each uploaded
artifact SHA to the immutable source SHA and GitHub workflow run, verify those
records before upload, then use the pinned keyless GitHub attestation action on
the actual cargo-dist archive, binary, and installer upload paths. SBOMs,
checksums, provenance JSON, and Sigstore bundles are deliberately not attestation
subjects. Subject paths are required to be existing `target/distrib/` files and
sidecar patterns are excluded before the action receives them.

The pinned cosign client additionally keylessly signs each `SHA256SUMS` as a
Sigstore bundle sidecar and verifies it offline before upload. The certificate
identity must exactly match
`https://github.com/jeprecated/brain-brew/.github/workflows/release.yml@refs/tags/<tag>`;
other workflows, branches, and repositories cannot satisfy the tag-scoped
regular expression. Only the two artifact-building release jobs receive OIDC
`id-token: write`. The host re-verifies checksum/SBOM/provenance and Sigstore
bundle bytes offline before it can publish, and uploads all checksum, SBOM,
provenance, and signature sidecars.
There are no repository signing keys or long-lived signing secrets.

### Operator update and recovery

1. Run `devenv shell supply-chain:check`. Fix an advisory or license first; do
   not delete an audit finding or add a broad ignore. For an unavoidable finding,
   add only its exact advisory/package ID with an accountable owner, short expiry,
   and a concrete upgrade/review rationale to `supply-chain-policy.toml`.
2. Regenerate the lock with the package manager, rerun the check, and inspect the
   evidence under `target/release-evidence/dependency-policy/`. An expired entry
   intentionally blocks recovery until it is reviewed or removed.
3. If a release build fails after metadata generation, discard its artifacts and
   rerun the release workflow from the same immutable tag. Never copy sidecars
   between runs: provenance binds `github.run_id`. If host verification reports a
   checksum, SBOM, source SHA, or workflow mismatch, treat the artifacts as
   unpublishable and rebuild; do not override the gate.
4. Consumers can validate published bytes offline with `sha256sum -c SHA256SUMS`.
   GitHub's keyless provenance attests the shipped artifact itself (not its
   sidecar); after downloading a release archive or binary, verify it with:

   ```bash
   gh attestation verify ./brainbrew-<archive-or-binary> --repo jeprecated/brain-brew
   ```

   GitHub verifies the artifact digest and repository identity; the release
   workflow's tag-scoped OIDC identity is independently enforced for the
   accompanying offline Sigstore checksum bundle.

## Credential and trigger boundary

Every workflow defaults to `contents: read`. CI and the reusable quality workflow
have no publication credentials and can run untrusted pull-request code. They
check out the immutable PR head with `persist-credentials: false`; there is no
`pull_request_target` workflow.

Only `release.yml`'s `host` job has `contents: write`, and it depends on all
quality and artifact jobs. `GH_TOKEN` is passed only to the `dist host` upload
command and the final `gh release create` command. The repository does not place
a crates.io token in Actions: crates.io publication remains a manual, separately
reviewed operation. GitHub's token cannot be made more granular than the job's
`contents: write` permission, so the host job remains a residual GitHub-platform
trust boundary; branch/tag protection and review are required controls.

Homebrew is not a supported channel. No workflow, token, tap, or PAT path may
publish a formula.

## Intentional pin updates

Dependabot proposes GitHub Actions, Cargo, and npm updates but must not be
configured to auto-merge release-input changes. For each pin update:

1. Review the upstream release notes, owner, commit/tag relationship, and any
   security advisories; record the new version, full SHA, and provenance URL in
   the table above.
2. For actions, resolve the upstream signed/annotated tag through GitHub's API or
   trusted release metadata to its full commit SHA. Update the inline workflow
   comment and the exact `ACTION_PINS` entry together.
3. For cargo-dist, obtain the platform asset checksums from the upstream release,
   review them, and update every affected mapping in
   `scripts/install_cargo_dist.sh`; never reinstate `curl | sh`.
4. Update Nix only through the checked-in lockfiles and review both revision and
   NAR hash. Keep `nix profile add --inputs-from .` so CI uses that lock.
5. Run the checker and its regression tests, then the normal release gates
   documented in [Releasing Brain Brew](./releasing.md). A reviewer must inspect
   the workflow diff and provenance before merging.

An unfamiliar action, container image, installer, package registry bootstrap, or
credential placement is a policy failure until this document, the narrow checker,
and tests are deliberately extended with its exact evidence.
