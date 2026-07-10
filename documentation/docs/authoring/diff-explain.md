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

If `explain --json` fails during target composition, it writes the version-1 `{ "error": ... }` envelope to stdout with a non-zero exit and empty stderr. Structured failures and target/base/overlay context live in `error.details` (and are mirrored directly during the compatibility window). Precondition entries include `code`, `category`, `deck_path`, `entity_kind`, `intent`, `overlay`, `expected`, and `actual`.

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

The path uses stable IDs, not row numbers or raw YAML positions. Added and removed entities include a deterministic typed summary in `after` or `before`; modified values include both.

### Exact semantic laws

The default diff is the exact canonical equivalence oracle. An empty diff means the two `CanonicalDeck` values are equal under only these normalizations:

- map insertion order is nonsemantic;
- set insertion order is nonsemantic;
- field-definition, card-template, structured-image, and positional message-component order is semantic;
- stable IDs participate whether stored as entity values or collection keys;
- scalar, structured-image, and structured-message field representations remain distinct even when they lower to identical adapter text.

Changes are sorted by canonical path and kind, so repeated runs are deterministic. Reversing operands swaps added/removed direction and before/after values; modifications keep their kind. Comparison uses typed domain values, never YAML, JSON, `Debug`, or another presentation serialization.

### Lossy CrowdAnki equivalence

CrowdAnki round trips use the explicit `crowdanki-export-import-v1` projection before applying this exact oracle. The projection documents and applies only adapter losses: source variables are rendered, structured fields are lowered, media hashes and typed tombstones are not stored, unsupported adapter IDs are discarded, and stable IDs are regenerated from adapter-visible content. Suggested-ID collisions are reported as an unrepresentable adapter loss rather than treated as exact equality. The unprojected semantic diff is never weakened.

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

Review the generated overlay before committing. Sparse destructive changes include exact typed `expected_base.value` values. Complete replacements/removals include tooling-generated `expected_base.fingerprint` values and reapply only to the exact input deck. See [Canonical entity fingerprints](../reference/entity-fingerprints.md).
