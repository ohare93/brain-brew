# Changelog

## Unreleased

The next preview is `1.0.0-alpha.8`; it is pending the documented release gates and manual crates.io publication.

- Card templates can select `#question` and `#answer` fragments from one combined include file.
- Media declarations can separate package-relative source locations from CrowdAnki output names while retaining authored SHA-256 integrity checks and hash-refresh tooling.
- CSV translation descriptors now own a validated subset of resolved note-type fields, allowing one shared translation source across Standard and extension-added fields while ordinary CSV note sources remain complete.
- Typed CSV image fields may split one cell into an ordered structured-image sequence with an exact descriptor delimiter.
- Canonical field definitions now preserve CrowdAnki right-to-left direction through YAML, overlays, composition, fingerprints, import, and export; omitted `rtl` remains `false`.

## v1.0.0-alpha.5

- Adds ADR-022 composable read-only CSV authoring with explicit descriptors, joins, and localized columns; translation and sparse-field sources; exact incremental CSV-to-YAML ownership transfer; Workbench provenance and read-only enforcement; source-graph fingerprint and lock integration; and maintained all-CSV-versus-migrated certification.
- Distributed Workbench binaries now include the write implementation by default while retaining the explicit runtime `--enable-write` opt-in; release smoke tests reject artifacts that omit the capability.
- Recursive package-root discovery is now deterministic, no-follow, and registry-owned, with exact built-in VCS/build/output/cache pruning, authorized component-glob ignores, explicit depth/entry/manifest budgets, structured actionable errors, and JSON visit/prune statistics shared by every package-root command.
- Media-bearing `verify` and manifest/ad-hoc CrowdAnki export are now release-strict by default: every final package owner needs an explicit media root, hashes must be canonical lowercase SHA-256, and bytes must exist and match before clean-tree publication. Hashless/source-only fixtures must migrate to explicit `--media-mode reference-only`, whose human/JSON status is always marked not release-ready when media is present.
- Structured `!image` rendering percent-encodes hostile filenames and HTML-attribute escapes output without changing declaration/CrowdAnki filenames; media paths now reject control, URL/platform, bidi/format-control, and reserved-name ambiguity, while scanners decode normalized HTML/URL references before comparison.
- Package federation now validates the complete root/include/package-root/lock registry before planning: package versions and exact pins use SemVer, explicit base compatibility ranges are enforced, dependency cycles include full deterministic edge traces, and overlay catalog IDs/kinds must match decoded sources.
- CrowdAnki import now fails closed on additional unsupported or ambiguous data, including `bafmt`/`bqfmt`/`did` fields and media stable-ID collisions, while preserving media-file parity.
- Canonical YAML emission is safer for block scalars, CRLF-sensitive content, and map keys, with validated/quoted keys and hard failures for un-emittable key text.
- Overlay field operations are fill-blank-only. Full `note:`/`note_type:` bodies require explicit `add`, or fingerprint-protected `replace`/`override`; merge remains sparse.
- Complete note, note-type, field-definition, card-template, and media destructive operations now compare stable `sha256:v1` canonical entity fingerprints. Presence-only expected bases are rejected, `diff --as-overlay` generates fingerprints, and explain JSON exposes typed expected/actual precondition details.
- Package verification detects drift in local path sources, and path locks are stored as portable paths relative to `brainbrew.lock`.
- Translation coverage now surfaces untranslated structured-message `format` glue strings, and stale-translation resolution removes shadowed stale records cleanly.
- `validate --json`, `explain --json`, `diff --json`, and `targets --json` now emit machine-parseable failure envelopes on stdout with non-zero exits, empty stderr, and structured `error.errors[]` details where available.
- The Ultimate Geography regression fixture is now an exact pinned source/media snapshot with a provenance lock and mandatory parsed `deck.json` oracles for all 74 main plus 26 companion targets; source sync, explicit expected-output acceptance, and read-only drift checking are separate hermetic boundaries.

## v1.0.0-alpha.1

Initial Rust-based Brain Brew preview release.

- Adds the `brainbrew` CLI for Canonical Deck validation, formatting, composition, semantic diffing, and CrowdAnki import/export.
- Adds Federated Deck manifests, named targets, package-qualified composition, package locks, and CI-friendly `verify` checks.
- Adds media reference validation and release-oracle comparison support for parity reviews.
- Includes Ultimate Geography-style fixtures used to validate translations, variants, and Hardcore Geography extension overlays.
- Ships prebuilt release archives, shell and PowerShell installers, and a Homebrew formula via `cargo-dist`.
- `1.0.0-alpha.1` was published with interfaces incompatible with `1.0.0-alpha.2`; use the current preview rather than treating alpha.1 as compatible with this source.
