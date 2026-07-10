---
title: Diff and explain
---

# Diff and explain

Use `explain` to understand a target. Use `diff` to compare two decks or draft an overlay.

## Explain a target

```bash
brainbrew explain --manifest brainbrew.yaml --target de-extended
```

Human output shows the expanded overlay stack and semantic changes.

For tools and UIs:

```bash
brainbrew explain --manifest brainbrew.yaml --target de-extended --json
```

If `explain --json` fails during target composition, it writes the version-1 `{ "error": ... }` envelope to stdout with a non-zero exit and empty stderr. Structured failures and target/base/overlay context live in `error.details` (and are mirrored directly during the compatibility window).

## Semantic diff

```bash
brainbrew diff deck.yaml edited.yaml
```

Example output:

```text
1 semantic change

~ notes.note.finland.fields.field.capital
  - Helsinki
  + Helsingfors
```

The path uses stable IDs, not row numbers or raw YAML positions.

## JSON diff

```bash
brainbrew diff deck.yaml edited.yaml --json
```

Use JSON when another tool needs to render or inspect changes. If `diff --json` fails, it follows the CLI JSON error contract: exit `1`, empty stderr, and a versioned `{ "error": ... }` envelope on stdout.

For a conventional CI gate, opt into change-sensitive status without changing the report:

```bash
brainbrew diff deck.yaml edited.yaml --json --exit-code
```

This exits `0` for no differences, `2` for semantic differences, and `1` for usage, parse, or filesystem errors.

## Draft an overlay

```bash
brainbrew diff deck.yaml edited.yaml \
  --as-overlay \
  --id overlay.patch.capitals \
  --kind patch > overlays/patches/capitals.yaml
```

Review the generated overlay before committing. Destructive changes include `expected_base` values.
