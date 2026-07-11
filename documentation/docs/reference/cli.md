---
title: CLI reference
---

# CLI reference

Run `brainbrew --help` or `brainbrew <command> --help` for exact current usage. Unknown commands, unknown options, and unexpected dash-prefixed arguments are rejected instead of ignored.

## Exit and error contract

Commands return exit code `0` on success and exit code `1` on operational or usage failure. Human-readable failures print diagnostics to stderr with empty stdout. `diff --exit-code` additionally returns `2` when a valid comparison contains semantic differences; it still returns `1` for an operational error.

Whenever `--json` is selected, including `translations` and every existing JSON route, failures emit the versioned machine envelope below on stdout and leave stderr empty. Success output remains the command's documented JSON schema.

```json
{
  "error": {
    "schema_version": 1,
    "command": "validate",
    "context": null,
    "code": "validation_failed",
    "category": "validation",
    "path": "notes.note.france.note_type_id",
    "message": "invalid deck",
    "diagnostics": [{
      "code": "validation_failed",
      "category": "validation",
      "source": "deck.main",
      "children": [{
        "code": "missing_note_type",
        "category": "validation",
        "path": "notes.note.france.note_type_id",
        "message": "note references missing note type note-type.country"
      }]
    }],
    "details": {}
  }
}
```

See [Diagnostic and error contracts](diagnostics.md) for stable codes, typed metadata, ordering, versioning, and migration. `message` is supplemental and must not be parsed.

## `targets`

```bash
brainbrew targets --manifest brainbrew.yaml
brainbrew targets --manifest brainbrew.yaml --json
brainbrew targets --package-root ../packages
```

Lists build targets and package metadata. With `--json`, the `discovery` object reports inspected roots/entries/directories/files/manifests and built-in/configured prune counts.

### Package-root discovery options

Every command whose usage accepts `--package-root` also accepts the same repeatable/validated discovery options:

```text
--package-ignore <safe-relative-pattern>
--discovery-max-depth <1..=256>
--discovery-max-entries <1..=10000000>
--discovery-max-manifests <1..=100000>
```

Defaults are depth 32, 100,000 inspected entries, and 1,000 manifests. `*`/`?` match within one path component and a complete `**` component matches any number of components. See [Packages and lock files](../authoring/packages-locking.md#bounded-package-root-discovery) for pruning, precedence, diagnostics, and rationale.

## `fmt`

```bash
brainbrew fmt deck.yaml
brainbrew fmt media.yaml
brainbrew fmt overlays/languages/de.yaml
brainbrew fmt brainbrew.yaml
brainbrew fmt brainbrew.lock
```

Canonicalizes supported source files. Standalone media-map files used by `media: !include media.yaml` are formatted as root media mappings; formatting a deck with a hoisted media map preserves the `media: !include ...` line.

## `validate`

```bash
brainbrew validate deck.yaml
brainbrew validate deck.yaml --json
brainbrew validate --manifest brainbrew.yaml --target en-standard
brainbrew validate --manifest brainbrew.yaml --target en-standard --json
```

Validates source or composed target semantics.

## `compose`

```bash
brainbrew compose --manifest brainbrew.yaml --target en-standard --out build/en-standard.yaml
brainbrew compose --manifest brainbrew.yaml --target en-standard --out build/en-standard.yaml --force
```

Produces a resolved Canonical Deck. Missing output parents below the selected output ancestor are created. Existing files, directories, and symlinks are refused by default; `--force` may replace an existing regular file through the recoverable workspace transaction. Parent creation and ordinary failed commits leave no empty output directories.

## `export crowdanki`

```bash
brainbrew export crowdanki --manifest brainbrew.yaml --target en-standard --media-root media/
brainbrew export crowdanki --manifest brainbrew.yaml --target en-standard --media-mode reference-only
brainbrew export crowdanki --manifest brainbrew.yaml --target en-standard --media-root media/ --force --json
```

Exports a CrowdAnki folder. Media targets are strict by default and require all package owner roots, canonical hashes, and matching bytes. `--media-mode reference-only` is the explicit development-only path for producing `deck.json` without byte copy; it still validates all references/collisions and reports `NOT RELEASE-READY`. A missing root never selects it. `--json` returns structured `media.mode`, `media.release_ready`, copy counts, and warnings. Without `--out`, manifest-target exports default to `build/crowdanki/<target>` unless the target configures `exports.crowdanki.out`. Existing output is refused unless `--force` is explicit. Brain Brew validates everything before it stages the complete clean tree privately and publishes it by directory rename. Forced replacement first renames the old complete tree to a recovery backup; ordinary failure restores it, and interruption leaves a sibling recovery journal rather than a mixed tree.

## `media hash`

```bash
brainbrew media hash --manifest brainbrew.yaml --all-targets --media-root media/
brainbrew media hash --manifest brainbrew.yaml --target en-standard --media-root media/
```

Computes SHA-256 values for declared media files and writes missing/stale hashes back to deck or overlay source YAML with include-preserving canonical formatting. If a deck uses `media: !include media.yaml`, the command follows the include and writes updated hashes to the included media-map file.

## `import crowdanki`

```bash
brainbrew import crowdanki build/crowdanki/en-standard --accept-suggested-ids --out deck.yaml
brainbrew import crowdanki build/crowdanki/en-standard --accept-suggested-ids --force --out deck.yaml
```

Imports a CrowdAnki folder into typed, canonical Deck YAML. `--accept-suggested-ids` accepts the current deterministic automatic suggestions; no suggested-ID override file or selective override argument exists. Imported note suggestions NFC-normalize the first field, keep a unique ASCII-readable `note.<slug>`, and otherwise use a GUID-assisted SHA-256 suffix for repeated, blank, or non-Latin first fields. The original CrowdAnki GUID remains independently stored as `crowdanki:guid`. GUID identity itself is opaque exact non-empty text (no whitespace or Unicode normalization), and duplicate diagnostics name every `$.notes[index].guid` source location. Standard-model template ordinals must be zero-based contiguous array positions (`tmpls[index].ord == index`); malformed order is rejected rather than sorted or renumbered. Import has no JSON route, so these typed codec diagnostics are rendered to stderr with the input `deck.json` path. See [Import CrowdAnki](../authoring/importing-crowdanki.md) for the exact suffix, normalization, and identity contract. By default import creates a new output and refuses an existing file, directory, or symlink. `--force` may replace an existing regular file; Brain Brew fingerprints and backs up that file and commits the replacement through one recoverable workspace transaction. Import writes a complete deck file and re-inlines the `media:` block; it does not preserve a previously hoisted `media: !include ...` source layout.

## `diff`

```bash
brainbrew diff left.yaml right.yaml
brainbrew diff left.yaml right.yaml --json
brainbrew diff left.yaml right.yaml --exit-code
brainbrew diff left.yaml right.yaml --as-overlay --id overlay.patch.example --kind patch
```

Compares decks semantically or drafts an overlay. Default report mode exits `0` even when changes are present. With `--exit-code`, no differences exit `0`, differences exit `2` after the same human/JSON report, and parse/filesystem/usage errors exit `1`.

## `explain`

```bash
brainbrew explain --manifest brainbrew.yaml --target en-standard
brainbrew explain --manifest brainbrew.yaml --target en-standard --json
```

Shows expanded overlay stack and resulting changes.

## `translations` / `translate`

```bash
brainbrew translations --manifest brainbrew.yaml --target da-standard
brainbrew translations --manifest brainbrew.yaml --target da-standard --context
brainbrew translations --manifest brainbrew.yaml --all-targets --summary
```

Reports translation coverage, shows terminal note/card context, summarizes translation state, or applies reviewed translation stubs back to canonical translation overlay YAML.

## `workbench`

```bash
brainbrew workbench serve --manifest brainbrew.yaml
brainbrew workbench serve --manifest brainbrew.yaml --port 0 --no-open
brainbrew workbench serve --manifest brainbrew.yaml --dev-assets target/workbench-ui
brainbrew workbench serve --manifest brainbrew.yaml --media-root media/
```

Starts the local Deck Workbench server on `127.0.0.1`, serving the browser UI plus JSON APIs for health and workspace metadata. Normal and distributed builds are read-only: browse, compare, preview, media, and browser-local drafts work, while direct HTTP requests to state-changing routes return HTTP 403. `/api/workspace` exposes the authoritative `write_capability` shown by the UI.

Write-path development requires both `cargo build -p brainbrew --features workbench-write-dev` and the explicit runtime flag `--enable-write`. Normal binaries reject that flag, and no environment variable enables writes. This unsafe development mode is visibly marked and is not a supported release workflow.

Release builds serve embedded Leptos/WASM assets from the `brainbrew` binary; during UI development run `devenv shell workbench-ui-watch` and pass `--dev-assets target/workbench-ui`. Use `devenv shell workbench-ui-build` for a one-shot development WASM asset build, or `devenv shell workbench-ui-embed` to refresh the release assets checked into the CLI crate. Use `--media-root` when declared media files live outside the manifest root. See [Deck Workbench](workbench.md) for risks, draft retention, and exact removal conditions.

## `verify`

```bash
brainbrew verify --manifest brainbrew.yaml --all-targets --media-root media/
brainbrew verify --manifest brainbrew.yaml --all-targets --media-mode reference-only
brainbrew verify --manifest brainbrew.yaml --all-targets --media-root media/ --json
brainbrew verify --manifest brainbrew.yaml --target legacy-target --skip-content-validation
```

Runs the workspace verification gate. Rendered deck descriptions and card templates are checked as lightweight HTML fragments, and note-type styling is checked for balanced CSS structure; `--skip-content-validation` is the escape hatch for legacy Anki content that renders correctly despite a false positive. Referenced-but-undeclared media is always an error and unused media warns. Any media target is strict by default: all owner roots, canonical non-empty hashes, and matching bytes are required. `--media-mode reference-only` is explicit development mode, still checks references/collisions/path and present-hash syntax, and reports `media.release_ready: false` under `--json`; it cannot be combined with roots. Targets without media are unaffected. Stale translation records warn by default and fail when the target or command uses strict translation coverage (`translation_coverage: strict` or `--translation-coverage strict`).

## `lock`

> **Experimental:** Lock/package federation works today, but the `brainbrew.lock` format and `brainbrew lock` CLI surface may change incompatibly in any release until a real downstream consumer stabilizes them.

```bash
brainbrew lock update --package upstream.package --path ../upstream
brainbrew lock update --package upstream.package --git https://github.com/owner/repo.git --ref main
brainbrew lock update --package upstream.package --tarball https://example.org/source.tar.gz
brainbrew lock verify
```

Updates or verifies federated package locks.
