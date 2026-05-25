# Brain Brew

Brain Brew is a Rust-based, local-first deck federation and round-trip engine for shared Anki-compatible decks.

It continues the established Brain Brew project name while replacing the legacy Python recipe pipeline with canonical deck source, overlays, manifests, and reproducible verification.

It aims to help deck maintainers compose a base deck with translations, extensions, patches, and personal overlays while preserving stable identity for Anki/CrowdAnki round trips.

## Current Status

Brain Brew now has a working Rust core, reusable format codecs, and a thin CLI for Canonical Deck validation, overlay composition, CrowdAnki import/export, semantic diffing, media checks, authoring helpers, Federated Deck manifests, package-qualified target composition, and locked package inputs.

The repository includes two tested fixtures:

- `fixtures/ug-style/` — a small Ultimate Geography-style fixture for fast end-to-end checks.
- `fixtures/ultimate-geography/` — a full Ultimate Geography canonical workspace used as a large parity case study, including Hardcore Geography as an extension overlay.

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

See the dedicated documentation site in [`documentation/`](documentation/) for manifest, source variable, translation dictionary, overlay, locking, and example workflows. Lock update/verify uses Rust-native fetching and NAR hashing; Nix is only an optional install/build path.

## Install the CLI with Nix

Run the CLI directly from this flake:

```bash
nix run . -- --help
```

Build a local binary:

```bash
nix build .#brainbrew
./result/bin/brainbrew --help
```

Install into your user profile:

```bash
nix profile install .#brainbrew
brainbrew --help
```

See [`documentation/docs/getting-started/install.md`](documentation/docs/getting-started/install.md) for install options and an edit/export loop for trying changes against a Federated Deck workspace.

## Workspace

```text
crates/
  brain-brew-core/     Pure domain model, validation, composition, semantic diffing
  brain-brew-formats/  Reusable YAML and CrowdAnki codecs
  brain-brew-cli/      Thin `brainbrew` command-line interface
```

## Development

This project uses Devbox:

```bash
devbox run fmt
devbox run test
devbox run clippy
devbox run ci
```

Useful docs:

- Agent guidance: [`AGENTS.md`](AGENTS.md)
- Documentation site source: [`documentation/`](documentation/)
- Start here: [`documentation/docs/intro.md`](documentation/docs/intro.md)
- Domain glossary: [`documentation/docs/reference/glossary.md`](documentation/docs/reference/glossary.md)
- Project scope: [`documentation/docs/reference/project-scope.md`](documentation/docs/reference/project-scope.md)
- Active ADRs: [`documentation/docs/reference/decisions/`](documentation/docs/reference/decisions/)
