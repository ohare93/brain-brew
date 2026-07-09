# Release, Supply-Chain, and Filesystem/Network Security Audit

## Review

### Correct

- Cargo forbids unsafe Rust workspace-wide (`Cargo.toml:23-24`), and the committed `Cargo.lock` contains only checksum-backed crates.io registry sources (364 sourced packages; no Git dependencies were found).
- Both Nix inputs are locked by revision and NAR hash (`flake.lock`; `devenv.lock`), and the separate cargo-dist Nix input is also revision-pinned (`devenv.yaml:4-5`). `nix flake check --no-build` evaluated successfully for the host system and `nix flake show --all-systems` exposed the four declared Nix systems.
- Normal CI explicitly limits `GITHUB_TOKEN` to `contents: read` (`.github/workflows/ci.yml:7-8`), runs formatting, Rust tests, clippy with warnings denied, embedded-asset freshness, and browser E2E (`devenv.nix:108-115`).
- Embedded Workbench filenames are content-hashed and `index.html` has SRI for CSS, JS, and WASM (`crates/brain-brew-cli/assets/workbench/index.html:7-9`). `devenv shell workbench-ui-embed-check` rebuilt the release bundle and reported `Workbench embedded release assets are fresh.` The CLI crate package list includes all four assets.
- The release smoke command installs with the lockfile and exercises version, validate, compose, CrowdAnki export, and verify (`devenv.nix:100-106`; `scripts/release_smoke.sh:1-31`). It passed locally.
- The Workbench binds only to IPv4 loopback (`crates/brain-brew-cli/src/commands/workbench.rs:162-170`), media paths reject lexical parent traversal (`crates/brain-brew-cli/src/media_assets.rs:67-77`), and browser launch passes the fixed local URL as a process argument rather than interpolating it into a shell (`crates/brain-brew-cli/src/commands/workbench.rs:4962-4983`).
- A tracked-file scan found no private-key markers, GitHub PAT-like values, AWS access-key IDs, or tracked `.env`/credential files. The largest intentional embedded asset is the 1,078,375-byte WASM file.

## Severity-ranked findings

### Blocker — the current workspace version is already immutable on crates.io and the dependent package no longer builds against that published version

**Refs:** `Cargo.toml:11-21`; `scripts/publish_crates.sh:53-60,106-122`; `.github/workflows/package-smoke.yml:14-21`; `devenv.nix:99-105`; `documentation/docs/getting-started/install.md:11-30`.

All publishable crates still declare `1.0.0-alpha.1`, with exact internal requirements `=1.0.0-alpha.1`. The crates.io API and `cargo search` confirm that `brain-brew-core`, `brain-brew-formats`, and `brainbrew` at that version already exist and are not yanked. Those immutable published sources predate current workspace APIs.

**Concrete validation:** `devenv shell release:crates` failed. `brain-brew-core` dry-run warned that `brain-brew-core@1.0.0-alpha.1 already exists`; then `brain-brew-formats` package verification downloaded that published core and failed with 40 compile errors, including missing `FieldImageReference`, `MessageComponent`, `StaleTranslation`, `StructuredMessage`, and `TargetAdaptation`. Current path-based workspace tests cannot expose this. The job named “Install packaged CLI” also does not install a `.crate`; it executes `cargo install --path ...` (`.github/workflows/package-smoke.yml:16-19`) and therefore uses current local path dependencies.

**Required resolution:** bump all three publishable crates and exact internal requirements to a new, unpublished version; update lockfile, changelog, docs, and the hard-coded dist plan tag; then verify actual packaged artifacts in dependency order against the registry state. The package smoke gate must install/test extracted `.crate` contents (or a staging registry), not only workspace paths. No release should proceed while this gate fails.

### High — Workbench trusts arbitrary Host/Origin values, leaving its unauthenticated read/write API open to DNS rebinding

**Refs:** `crates/brain-brew-cli/src/commands/workbench.rs:162-188,191-222`.

Loopback binding alone does not establish a browser security boundary. The router exposes workspace/source reads and mutation endpoints (`new-language`, `apply-preview`, and `apply`) with no session token, Host allowlist, or Origin validation. A malicious origin that DNS-rebinds its hostname to `127.0.0.1` remains same-origin in the browser and can reach these endpoints; the server currently accepts the attacker-controlled Host.

**Concrete validation:** against a live Workbench on an ephemeral loopback port, both a normal request and this request returned HTTP 200:

```text
Host: attacker.example
Origin: https://attacker.example
GET /api/health
```

The attacker-host response disclosed `{"manifest":"fixtures/ug-style/brainbrew.yaml","status":"ok"}`. The same router hosts file-mutating POST handlers.

**Required resolution:** generate an unguessable per-process capability and require it for every API route (especially mutations), validate Host against the exact loopback authority/port, reject foreign Origin/Referer values, and add DNS-rebinding tests for both reads and writes. Keep loopback binding as defense in depth.

### High — release jobs execute mutable/unverified third-party code with publication credentials

**Refs:** `.github/workflows/release.yml:17-18,56-72,112-145,222-279,286-325`; `.github/workflows/ci.yml:16-24`; `.github/workflows/package-smoke.yml:12-18`.

Every GitHub Action is referenced by a mutable major tag (`actions/checkout@v6`, artifact actions, Cachix actions, and `dtolnay/rust-toolchain@stable`) rather than a commit SHA. The release workflow grants `contents: write` at workflow scope, places `GH_TOKEN` in build/plan job environments, downloads the cargo-dist installer with `curl | sh` without checking a digest/signature, may install rustup the same way, and executes `matrix.install_dist.run`/`matrix.packages_install` generated by that tool. The Homebrew job checks out with a long-lived tap token and also invokes mutable actions.

A compromise or retag of any of these upstreams can replace release artifacts, create GitHub releases, or steal the tap token. Pinning `cargo-dist-version = "0.30.4"` (`dist-workspace.toml:5-7`) identifies a version but does not authenticate the downloaded installer bytes.

**Required resolution:** pin every action to a reviewed full commit SHA; verify cargo-dist/rustup installers against published hashes or signatures (prefer a prebuilt, pinned toolchain image/Nix derivation); move `contents: write` to the host job only; keep build jobs read-only and token-free; protect publication with a GitHub Environment and approval; use a narrowly scoped/rotated Homebrew credential.

### High — a tag can publish without the repository quality gates, crate publication, or any pre-tag macOS/Windows build

**Refs:** `.github/workflows/release.yml:41-55,90-166,215-279,281-343`; `.github/workflows/ci.yml:3-33`; `.github/workflows/package-smoke.yml:3-21`; `dist-workspace.toml:10-18`; `CHANGELOG.md:20-21`.

The release workflow is independent of CI and Package Smoke. Its host job depends only on cargo-dist plan/build jobs, so a tag release can be created while normal CI fails or is still running. The workflow contains no Rust test, clippy, embedded freshness, E2E, release smoke, crates metadata, or crate dry-run invocation. It publishes GitHub artifacts and Homebrew only; `scripts/publish_crates.sh` is not called, despite the changelog claiming crates.io publication.

`dist manifest` reported `pr_run_mode=plan`, so release pull requests do not build artifacts. The macOS ARM, macOS Intel, Windows, and Linux release binaries are therefore first built only after the publication tag is pushed. Normal CI and Package Smoke both run only on Ubuntu. The configured release matrix also omits `aarch64-unknown-linux-*`, although the Nix flake advertises `aarch64-linux` (`flake.nix:11-16`; `dist-workspace.toml:12-13`).

**Required resolution:** make release call a reusable, passing quality workflow before host/publish; build and smoke-test every release target on pull requests or an explicit release-candidate workflow; include crate package verification/publication in the versioned release transaction (with restart/idempotency design); and document or add ARM Linux support. Add artifact signature/provenance/SBOM generation before publication.

### High — the advertised Nix build/install path is broken because the package check runs browser E2E without its harness

**Refs:** `flake.nix:25-41,72-75`; `devenv.nix:53-82`; `documentation/docs/getting-started/install.md:80-110`.

The Nix derivation sets `cargoTestFlags = [ "--workspace" "--all-targets" ]`, which includes `brain-brew-workbench-e2e`, but does not build the expected debug CLI/UI or provide Chromium/chromedriver and E2E environment variables. `checks` is merely an alias of this same package derivation, not a separate quality check.

**Concrete validation:** `nix build .#checks.x86_64-linux.brainbrew -L --no-link` failed all 13 Workbench E2E tests with `brainbrew test binary not found at /build/source/target/debug/brainbrew; run devenv shell e2e`. Because `nix build .#brainbrew`, `nix run`, and profile install use the same derivation, this contradicts the documented install commands.

**Required resolution:** exclude the E2E crate from the normal Nix package check as the documented Cargo gates do, and define a separate Nix E2E check with all browser/UI/runtime inputs. Add explicit fmt/clippy/unit checks rather than aliasing the package derivation. Build all four declared systems in CI or a trusted binary cache workflow.

### High — fetched package archives preserve symlinks that can escape the hashed cache and package root

**Refs:** `crates/brain-brew-cli/src/commands/lock.rs:43-49,481-555,639-651,786-813`; subsequent locked-manifest join at `:94-108` and `:457-476`.

Tarballs are unpacked, copied, NAR-hashed, and cached, but `copy_dir_contents` deliberately recreates symlinks. Later file access joins manifest/base/include/media paths lexically and follows those symlinks. The NAR hash authenticates the symlink text, not confinement of the dereferenced target. A fetched package can therefore cause Brain Brew to read host files outside its source tree (and filesystem-facing Workbench/export flows may serve or copy such content).

**Concrete validation:** a temporary tarball contained `pkg/brainbrew.yaml` as an absolute symlink to a manifest outside the archive. This command succeeded:

```text
brainbrew lock update --package audit.tar-symlink --tarball file://.../pkg.tar
exit=0
```

The cache retained `.../sources/<hash>/brainbrew.yaml -> /tmp/.../outside.yaml`, and the external package metadata was accepted into the lock.

**Required resolution:** reject symlinks in fetched package sources, or canonicalize every dereferenced source path and require it to stay beneath the canonical package root. Apply containment to manifests, bases, overlays, scalar/media includes, media reads, and Workbench writes. Add tarball tests for absolute links, relative `../` links, link chains, and links swapped between validation and use.

### Medium — lock fetching has no transport policy, timeout/size budget, or decompression budget

**Refs:** `crates/brain-brew-cli/src/commands/lock.rs:493-537,604-651,675-686`.

Arbitrary tarball URLs flow directly to `ureq`; HTTP is not rejected, and even GitHub repository parsing explicitly accepts `http://`. Responses are read completely into an unbounded `Vec`, GitHub JSON into an unbounded `String`, and gzip/tar extraction has no entry-count, per-file, expanded-size, or compression-ratio limit. The first `lock update` computes its hash only after download and extraction, so hash locking does not protect that initial fetch from insecure transport or resource exhaustion.

**Required resolution:** allow HTTPS network URLs only (retain an explicit local-file mode), configure connect/read/overall timeouts and redirect policy, enforce compressed and expanded byte/entry limits while streaming, reject special archive entries, and fail before cache mutation. Tests should use a local server for slow responses, redirect loops, oversized `Content-Length`, chunked over-limit bodies, and gzip bombs.

### Medium — no dependency/advisory/license gate exists, and the committed documentation graph currently has a high advisory

**Refs:** `devenv.nix:84-86,108-122`; `.github/workflows/ci.yml:23-26`; `documentation/package.json:11-20`; `documentation/package-lock.json:7080,7391,10268-10275,16284-16291`; `Cargo.lock:2364-2367`.

The quality gate has no `cargo audit`, `cargo deny`, OSV, `npm audit`, license allowlist, dependency updater, SBOM, or provenance check. Documentation installation uses `npm install`, not deterministic `npm ci`, and docs are not built in CI. The lock contains 369 Cargo packages (364 registry sources) and 1,281 npm package entries, so manual review is not a realistic substitute.

**Concrete validation:** `npm --prefix documentation audit --omit=dev --json` reported 26 vulnerabilities: 1 high, 24 moderate, and 1 low. The high finding is `serialize-javascript` 6.0.2 (`GHSA-5c6j-r48x-rmvq`); the lock also contains vulnerable `js-yaml` 4.1.1 according to the audit. `cargo-audit` and `cargo-deny` were not installed and no CI references were found. Rust also directly uses the deprecated `serde_yaml 0.9.34+deprecated`, although this audit did not establish a RustSec vulnerability for it.

**Required resolution:** use `npm ci`, build docs, remediate or explicitly time-bound advisory exceptions, and gate npm/Cargo advisories and licenses in CI. Add automated dependency updates and generate an SBOM plus GitHub artifact attestations for releases.

### Low — published crate archives omit the project license text and README/package-discovery metadata

**Refs:** `Cargo.toml:11-17`; each publishable crate manifest `crates/brain-brew-{core,formats,cli}/Cargo.toml:1-11`; root `LICENSE`; `scripts/check_cratesio_metadata.py:31-66`.

The SPDX expression and repository metadata are present, but `cargo package --list` showed no `LICENSE` or `README` in any of the three publishable archives. Cargo metadata also reported `readme=None`, empty keywords, and empty categories for all three. The metadata checker only requires description/license/repository and therefore passes this incomplete packaging.

**Required resolution:** include the canonical license text and an appropriate README in every `.crate`; add useful keywords/categories; and make the metadata/package-content check assert those files before publishing.

## Notes and residual risks

- `plan.md` and `progress.md` were requested but absent at the supplied paths, so no plan/progress-specific assumptions could be checked.
- No Rust advisory result is claimed: `cargo-audit` was unavailable and is not configured in the repository. The absence of a result is itself covered by the missing-gate finding.
- `nix flake check --no-build` validates evaluation only. The subsequent real Nix build was run and failed as documented above.
- Package archive lists were inspected, but dependent `.crate` verification cannot currently pass until the version blocker is fixed.
- No source, test, workflow, dependency, or configuration file was edited. This report is the only file created by this audit.
- Review gate: **failed / blockers present**. Do not publish from the current version or workflow state.

## Validation summary

| Command/probe | Result | Evidence |
|---|---:|---|
| `jj status` / `jj diff --summary` | Passed | Existing uncommitted audit reports only before this report; no source/config changes and Jujutsu has no staged index. |
| `devenv shell crates:metadata-check` | Passed | Metadata checker reported all three crates at `1.0.0-alpha.1`. |
| `devenv shell release:smoke` | Passed | Locked path install completed; version, validate, compose, export, and verify succeeded. |
| `devenv shell release:crates` | **Failed** | Dependent crate verification compiled against immutable published core and produced 40 API mismatch errors. |
| `cargo package -p <crate> --list --allow-dirty` | Passed with finding | 17/23/37 files; embedded assets present; no LICENSE/README in any publishable crate. |
| `devenv shell workbench-ui-embed-check` | Passed | Release Trunk output matched checked-in assets byte-for-byte. |
| `nix flake check --no-build` | Passed with limitation | Host evaluation succeeded; incompatible systems were omitted from builds. |
| `nix flake show --all-systems --json` | Passed | Four declared package/app/check systems evaluated. |
| `nix build .#checks.x86_64-linux.brainbrew -L --no-link` | **Failed** | 13/13 E2E tests failed because the Nix derivation lacks the required debug CLI/E2E harness. |
| `devenv shell dist:plan` | Passed with findings | Release `v1.0.0-alpha.1`; PR mode is `plan`; four release targets build only in publication mode. |
| `npm --prefix documentation audit --omit=dev --json` | **Failed policy-wise** | 1 high, 24 moderate, 1 low advisory. |
| `bash -n scripts/*.sh` and Python compile check | Passed | Release/helper scripts parsed. Temporary Python bytecode was removed immediately and is not a repository change. |
| lock tarball symlink probe | **Security reproduction succeeded** | External symlinked manifest was read and retained in the hash-addressed cache. |
| Workbench Host/Origin probe | **Security reproduction succeeded** | Foreign Host and Origin received HTTP 200 and workspace manifest metadata. |
| tracked secret-marker scan | Passed | No private key, PAT-like token, or AWS access-key marker found. |

```acceptance-report
{
  "criteriaSatisfied": [
    {
      "id": "criterion-1",
      "status": "satisfied",
      "evidence": "Created only audit/15-release-security.md and reviewed Cargo/devenv/Nix, dependency features and locks, all GitHub workflows, cargo-dist and crate publishing scripts, package archives/metadata/licensing, embedded assets, and filesystem/network boundaries without changing implementation files."
    },
    {
      "id": "criterion-2",
      "status": "satisfied",
      "evidence": "Each finding cites exact files/lines and includes concrete command output or a reproducible probe; validation includes the failing crate dry-run, failing real Nix build, npm advisory counts, Host/Origin acceptance, and tarball symlink escape."
    }
  ],
  "changedFiles": [
    "audit/15-release-security.md"
  ],
  "testsAddedOrUpdated": [],
  "commandsRun": [
    {
      "command": "devenv shell crates:metadata-check && devenv shell release:smoke",
      "result": "passed",
      "summary": "Metadata passed and the locked path-installed CLI completed version/validate/compose/export/verify smoke coverage."
    },
    {
      "command": "devenv shell release:crates",
      "result": "failed",
      "summary": "Confirmed the release blocker: formats verification downloaded already-published core 1.0.0-alpha.1 and failed with 40 API mismatch errors."
    },
    {
      "command": "cargo package -p <core|formats|brainbrew> --list --allow-dirty",
      "result": "passed",
      "summary": "Inspected exact crate payloads; CLI assets are present, but all publishable archives omit LICENSE and README."
    },
    {
      "command": "devenv shell workbench-ui-embed-check",
      "result": "passed",
      "summary": "Checked-in release JS/WASM/CSS/HTML matched a fresh release build byte-for-byte."
    },
    {
      "command": "nix flake check --no-build && nix flake show --all-systems --json",
      "result": "passed",
      "summary": "Flake evaluated and exposed all four declared systems."
    },
    {
      "command": "nix build .#checks.x86_64-linux.brainbrew -L --no-link",
      "result": "failed",
      "summary": "Real Nix package/check build failed all 13 browser tests because the derivation does not provision the E2E harness."
    },
    {
      "command": "devenv shell dist:plan",
      "result": "passed",
      "summary": "Generated a 14-artifact plan and confirmed PR mode is plan-only with four publication targets."
    },
    {
      "command": "npm --prefix documentation audit --omit=dev --json",
      "result": "failed",
      "summary": "Reported 26 advisories: 1 high, 24 moderate, and 1 low."
    },
    {
      "command": "temporary tarball symlink lock-update probe",
      "result": "passed",
      "summary": "Reproduced the vulnerability: lock update accepted an external absolute symlink manifest and preserved it in cache."
    },
    {
      "command": "temporary live Workbench foreign Host/Origin probe",
      "result": "passed",
      "summary": "Reproduced DNS-rebinding exposure: attacker.example Host/Origin received HTTP 200 and manifest metadata."
    },
    {
      "command": "bash -n scripts/*.sh; Python syntax compile; cargo metadata --locked",
      "result": "passed",
      "summary": "Shell/Python helper syntax and locked Cargo metadata validated; generated bytecode was cleaned immediately."
    },
    {
      "command": "jj status && jj diff --summary",
      "result": "passed",
      "summary": "Confirmed no staged index and no implementation/config changes; only requested audit report files are uncommitted."
    }
  ],
  "validationOutput": [
    "release:crates: 40 compilation errors against already-published brain-brew-core 1.0.0-alpha.1.",
    "Nix build: 0 passed / 13 failed Workbench E2E tests due to missing target/debug/brainbrew.",
    "Workbench probe: normal=200, host_attacker=200; response disclosed the manifest path.",
    "Tarball probe: exit=0 and cached brainbrew.yaml remained an absolute symlink outside the package/cache.",
    "npm audit: high=1, moderate=24, low=1, total=26.",
    "Embedded assets: Workbench embedded release assets are fresh.",
    "Tracked secret-marker scan: no matches."
  ],
  "residualRisks": [
    "plan.md and progress.md were absent at the requested paths.",
    "No RustSec result is available because cargo-audit is neither installed nor configured in CI.",
    "Non-host Nix systems were evaluated but not built, and release macOS/Windows artifacts were not available for local smoke testing.",
    "Actual dependent crate artifact verification remains blocked until the immutable-version collision is resolved."
  ],
  "noStagedFiles": true,
  "notes": "Review only: no implementation changes or tests were added. Review gate fails because a release blocker and multiple high-severity security/release issues are verified."
}
```
