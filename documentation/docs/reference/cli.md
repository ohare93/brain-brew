---
title: CLI reference
---

# CLI reference

Run `brainbrew --help` or `brainbrew <command> --help` for exact current usage. Unknown commands, unknown options, and unexpected dash-prefixed arguments are rejected instead of ignored.

## Exit and error contract

Commands return exit code `0` on success and a non-zero exit code on failure. Human-readable failures print diagnostics to stderr with empty stdout.

For `validate`, `explain`, `diff`, and `targets`, passing `--json` also opts failures into a machine-readable contract: failures emit a JSON object on stdout, use a non-zero exit code, and leave stderr empty.

```json
{
  "error": {
    "message": "target composition failed",
    "errors": [
      {
        "kind": "ExpectedBaseMismatch",
        "path": "notes.note.france.fields.field.capital",
        "message": "field-level merge may only fill a blank value"
      }
    ],
    "target": "de-standard",
    "base": "deck.yaml",
    "overlay_stack": []
  }
}
```

When validation or composition details are available, `error.errors[]` contains objects with `kind`, `path`, and `message`. Context such as `overlay_stack`, `target`, and `base` is included where the command can report it, such as `explain --json`. `verify` and `export crowdanki` are not part of this JSON error set; their failures continue to use plain-text stderr.

## `targets`

```bash
brainbrew targets --manifest brainbrew.yaml
brainbrew targets --manifest brainbrew.yaml --json
brainbrew targets --package-root ../packages
```

Lists build targets and package metadata.

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
```

Produces a resolved Canonical Deck.

## `export crowdanki`

```bash
brainbrew export crowdanki --manifest brainbrew.yaml --target en-standard
```

Exports a CrowdAnki folder. Without `--out`, manifest-target exports default to `build/crowdanki/<target>` unless the target configures `exports.crowdanki.out`.

## `media hash`

```bash
brainbrew media hash --manifest brainbrew.yaml --all-targets --media-root media/
brainbrew media hash --manifest brainbrew.yaml --target en-standard --media-root media/
```

Computes SHA-256 values for declared media files and writes missing/stale hashes back to deck or overlay source YAML with include-preserving canonical formatting. If a deck uses `media: !include media.yaml`, the command follows the include and writes updated hashes to the included media-map file.

## `import crowdanki`

```bash
brainbrew import crowdanki build/crowdanki/en-standard --out deck.yaml
```

Imports a CrowdAnki folder into Canonical Deck YAML. Import writes a complete deck file and re-inlines the `media:` block; it does not preserve a previously hoisted `media: !include ...` source layout.

## `diff`

```bash
brainbrew diff left.yaml right.yaml
brainbrew diff left.yaml right.yaml --json
brainbrew diff left.yaml right.yaml --as-overlay --id overlay.patch.example --kind patch
```

Compares decks semantically or drafts an overlay.

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
brainbrew verify --manifest brainbrew.yaml --all-targets
brainbrew verify --manifest brainbrew.yaml --all-targets --media-root media/
brainbrew verify --manifest brainbrew.yaml --target legacy-target --skip-content-validation
```

Runs the workspace verification gate. Rendered deck descriptions and card templates are checked as lightweight HTML fragments, and note-type styling is checked for balanced CSS structure; `--skip-content-validation` is the escape hatch for legacy Anki content that renders correctly despite a false positive. Referenced-but-undeclared media is always an error; declared-but-unreferenced media is a warning. With `--media-root`, missing files, empty hashes, and stale hashes are errors. Stale translation records warn by default and fail when the target or command uses strict translation coverage (`translation_coverage: strict` or `--translation-coverage strict`).

## `lock`

> **Experimental:** Lock/package federation works today, but the `brainbrew.lock` format and `brainbrew lock` CLI surface may change incompatibly in any release until a real downstream consumer stabilizes them.

```bash
brainbrew lock update --package upstream.package --path ../upstream
brainbrew lock update --package upstream.package --git https://github.com/owner/repo.git --ref main
brainbrew lock update --package upstream.package --tarball https://example.org/source.tar.gz
brainbrew lock verify
```

Updates or verifies federated package locks.
