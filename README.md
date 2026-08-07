# Brain Brew

Brain Brew is a Rust-based, local-first deck federation tool for shared Anki-compatible decks.

It continues the established Brain Brew project name while replacing the legacy Python recipe pipeline with canonical deck source, overlays, manifests, and reproducible verification.

It helps deck maintainers compose a base deck with translations, extensions, patches, and personal overlays while preserving stable identity through CrowdAnki export and full-deck bootstrap import.

## Current Status

Brain Brew now has a working Rust core, reusable format codecs, and a thin CLI for Canonical Deck validation, overlay composition, CrowdAnki import/export, semantic diffing, media checks, authoring helpers, Federated Deck manifests, package-qualified target composition, and locked package inputs.

The repository includes two tested fixtures:

- `fixtures/ug-style/` — a small Ultimate Geography-style fixture for fast end-to-end checks.
- `fixtures/ultimate-geography/` — the complete pinned Ultimate Geography canonical input and real-media snapshot, including Hardcore Geography.

The full fixture is bound by `fixtures/ultimate-geography.lock.json` to UG
`54b3254...` (descending from migration history rebased on UG `e1fd8518...`)
and Brain Brew `68a8283...`.
`fixtures/ultimate-geography-expected/crowdanki/` contains exactly 100 parsed
`deck.json` oracles (74 main and 26 companion) while media is stored only once.
The default Rust tests compare every output offline and verify real media bytes
strictly. Source refresh, explicit expected-output acceptance, and read-only
checking are intentionally separate; see
[`scripts/ug-fixture-sync/README.md`](scripts/ug-fixture-sync/README.md). Fixture
media retains upstream per-file terms and attribution and is not licensed
wholesale by Brain Brew's root license. The 56 Hardcore images use a separately
pinned attribution supplement without changing the exact UG snapshot; see
[`THIRD_PARTY_ASSETS.md`](THIRD_PARTY_ASSETS.md).

Ultimate Geography is a fixture and case study for the general federation workflow; it is not a special product-specific CLI feature.

## Federated Deck workflow

A Federated Deck workspace contains a base Canonical Deck, overlays, and a `brainbrew.yaml` manifest declaring reproducible build targets.

Common commands:

```bash
brainbrew targets --manifest brainbrew.yaml --json
brainbrew targets --package-root ../anki-geo-packages
brainbrew lock update --package anki-geo.ultimate-geography --path ../ultimate-geography
brainbrew lock verify
brainbrew verify --manifest brainbrew.yaml --all-targets --media-root media/
brainbrew explain --manifest brainbrew.yaml --target de-extended --json
brainbrew compose --manifest brainbrew.yaml --target de-extended --out build/de-extended.yaml
brainbrew export crowdanki --manifest brainbrew.yaml --target de-extended --media-root media/
brainbrew diff deck.yaml edited.yaml --as-overlay --id overlay.patch.capitals --kind patch
```

CrowdAnki import bootstraps a new full-deck workspace after plan/review/apply; it never merges Anki edits into an existing source or overlay stack. Preserve the source, compare deliberately, and treat `diff --as-overlay` output as a review artifact before manually routing accepted changes. See [CrowdAnki bootstrap boundary](documentation/docs/authoring/crowdanki-bootstrap-boundary.md).

See the dedicated documentation site in [`documentation/`](documentation/) for manifest, source variable, translation dictionary, overlay, locking, and example workflows. Lock update/verify uses Rust-native fetching and NAR hashing; Nix is only an optional install/build path.

## Install the CLI

For normal deck users and contributors, install the released `brainbrew` CLI from crates.io or a GitHub Release. You do not need Nix to edit or verify a Federated Deck workspace, and downstream projects such as Ultimate Geography can link to the release version they recommend.

After the alpha.7 release gates pass and the maintainer manually publishes it to crates.io, Rust users can install the current preview crate:

```bash
cargo install brainbrew --version 1.0.0-alpha.7 --locked
brainbrew --version
```

For a no-Rust GitHub Release install, download the versioned installer and its
release checksum before executing it; never pipe a downloaded installer into a
shell. See the [installation guide](documentation/docs/getting-started/install.md)
for the checksum-verified commands.

To test the exact GitHub tag instead of the crates.io package, install directly from the pinned release tag:

```bash
cargo install --git https://github.com/jeprecated/brain-brew --tag v1.0.0-alpha.7 brainbrew --locked
```

Nix remains available as an optional reproducible build/install path for contributors and CI. Pin the same release tag for a reproducible external Nix channel:

```bash
nix run . -- --help
nix build .#brainbrew
nix run github:jeprecated/brain-brew/v1.0.0-alpha.7 -- --help
```

See [`documentation/docs/getting-started/install.md`](documentation/docs/getting-started/install.md) for all install options and an edit/export loop for trying changes against a Federated Deck workspace.

## Workspace

```text
crates/
  brain-brew-core/     Pure domain model, validation, composition, semantic diffing
  brain-brew-formats/  Reusable YAML and CrowdAnki codecs
  brain-brew-cli/      Thin `brainbrew` command-line package
```

## Development

This project uses Devenv. With direnv, `.envrc` loads the environment automatically; otherwise run commands through `devenv shell`:

```bash
devenv shell fmt
devenv shell test
devenv shell clippy
devenv shell ci
devenv test
```

Rust commands in Devenv default to two libtest threads and two Cargo build
jobs. This keeps CPU and thermal load lower at the cost of longer test/build
runtime. Existing `RUST_TEST_THREADS` and `CARGO_BUILD_JOBS` values are
preserved, so higher-throughput one-off runs can opt in explicitly:

```bash
RUST_TEST_THREADS=8 CARGO_BUILD_JOBS=8 devenv shell test
RUST_TEST_THREADS=1 CARGO_BUILD_JOBS=4 devenv shell -- cargo test -p brain-brew-core
devenv shell check:rust-parallelism
```

These settings govern Rust test/compilation parallelism only; they do not limit
Chromium, Trunk, npm, Nix builds, or other non-Cargo tools.

Useful docs:

- Agent guidance: [`AGENTS.md`](AGENTS.md)
- Documentation site source: [`documentation/`](documentation/)
- Start here: [`documentation/docs/intro.md`](documentation/docs/intro.md)
- Domain glossary: [`documentation/docs/reference/glossary.md`](documentation/docs/reference/glossary.md)
- Project scope: [`documentation/docs/reference/project-scope.md`](documentation/docs/reference/project-scope.md)
- Active ADRs: [`documentation/docs/reference/decisions/`](documentation/docs/reference/decisions/)
