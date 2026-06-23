---
title: CLI reference
---

# CLI reference

Run `brainbrew --help` or `brainbrew <command> --help` for exact current usage.

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
brainbrew fmt overlays/languages/de.yaml
brainbrew fmt brainbrew.yaml
brainbrew fmt brainbrew.lock
```

Canonicalizes supported source files.

## `validate`

```bash
brainbrew validate deck.yaml
brainbrew validate --manifest brainbrew.yaml --target en-standard
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

## `import crowdanki`

```bash
brainbrew import crowdanki build/crowdanki/en-standard --out deck.yaml
```

Imports a CrowdAnki folder into Canonical Deck YAML.

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
brainbrew workbench serve --manifest brainbrew.yaml --dev-assets build/workbench
```

Starts the local Deck Workbench server on `127.0.0.1`, serving the browser UI plus JSON APIs for health and workspace metadata.

## `verify`

```bash
brainbrew verify --manifest brainbrew.yaml --all-targets
brainbrew verify --manifest brainbrew.yaml --all-targets --media-root media/
```

Runs the workspace verification gate.

## `lock`

```bash
brainbrew lock update --package upstream.package --path ../upstream
brainbrew lock update --package upstream.package --git https://github.com/owner/repo.git --ref main
brainbrew lock update --package upstream.package --tarball https://example.org/source.tar.gz
brainbrew lock verify
```

Updates or verifies federated package locks.
