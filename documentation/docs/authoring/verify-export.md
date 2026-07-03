---
title: Verify and export
---

# Verify and export

Verification is the CI gate for a Federated Deck workspace.

## Verify one target

```bash
brainbrew verify --manifest brainbrew.yaml --target de-standard
```

## Verify every target

```bash
brainbrew verify --manifest brainbrew.yaml --all-targets
```

Verification checks:

1. manifest parsing and formatting;
2. base deck parsing and formatting;
3. overlay parsing and formatting;
4. lock-file package resolution and hashes;
5. dependency expansion;
6. target composition;
7. Canonical Deck validation;
8. translation coverage policy, when configured or passed with `--translation-coverage`;
9. stale translations, warning by default and failing under strict translation coverage;
10. media references always, plus media file existence and SHA-256 hashes when `--media-root` is passed;
11. configured CrowdAnki golden checks.

> **Experimental:** Lock/package federation works today, but the `brainbrew.lock` format and `brainbrew lock` CLI surface may change incompatibly in any release until a real downstream consumer stabilizes them.

## Verify media

```bash
brainbrew verify --manifest brainbrew.yaml --all-targets --media-root media/
```

Without `--media-root`, Brain Brew checks rendered field/template media references (`<img src>`, `[sound:]`, and related URL forms) against declarations. Referenced-but-undeclared media is an error; declared-but-unreferenced media is a warning.

With `--media-root`, Brain Brew also checks that every declared media file exists and that every declaration has a non-empty SHA-256 matching the file. Refresh hashes after intentional media edits with:

```bash
brainbrew media hash --manifest brainbrew.yaml --all-targets --media-root media/
```

## Export CrowdAnki

```bash
brainbrew export crowdanki \
  --manifest brainbrew.yaml \
  --target de-standard \
  --out build/crowdanki/de-standard
```

If a target contains `stale_translations`, export applies the recorded target text and prints a stale-review warning to stderr. Use `translation_coverage: strict` on release targets (or `--translation-coverage strict`) when stale translations should block release.

With media copied into the CrowdAnki folder's `media/` subdirectory:

```bash
brainbrew export crowdanki \
  --manifest brainbrew.yaml \
  --target de-standard \
  --media-root media/ \
  --out build/crowdanki/de-standard
```

Export copies the declared media set itself, so release scripts do not need a separate `cp media/*` step. Files present under `media-root` but not declared are not exported.

## Default and configured export paths

```yaml
targets:
  de-standard:
    overlays:
      - overlay.translation.de
    exports:
      crowdanki:
        out: build/crowdanki/de-standard
        golden: goldens/de-standard/deck.json
```

When `--out` is omitted, Brain Brew uses `exports.crowdanki.out` when configured; otherwise it defaults to `build/crowdanki/<target>`. For example:

```bash
brainbrew export crowdanki --manifest brainbrew.yaml --target de-standard
```

## Regression guarantees

Brain Brew's regression checks are semantic rather than raw text comparisons where ordering is not meaningful:

- Canonical Deck semantic diffs report stable entity paths such as `notes.note.finland.fields.field.capital` and ignore YAML key ordering or formatting noise.
- Composition/export regression tests exercise one deliberate change at a time: representative note fields, template HTML, CSS/styling, deck descriptions, media references, translation overlays, variants, and extension/field-fill overlays.
- Translation coverage tests distinguish stale source keys, persisted stale translations, strict-mode missing translations, path-specific faithful translations, reviewed no-change text, and target adaptations.
- CrowdAnki export tests compare parsed JSON paths so a one-parameter source edit must affect exactly the expected exported location.

For deck workspaces, run the same gate before review:

```bash
brainbrew verify --manifest brainbrew.yaml --all-targets --media-root media/
brainbrew translations --manifest brainbrew.yaml --all-targets --overlay overlays/languages --summary
```

Use `brainbrew diff` or `brainbrew explain` when reviewing a change interactively; both report semantic paths instead of line-oriented YAML noise.

## Golden checks

When `golden` is configured, `verify` compares generated CrowdAnki JSON against the golden as parsed JSON. This makes snapshots hard to update accidentally: changing generated output without updating the golden fails `verify`.

To update a golden intentionally, export the target to its configured golden directory, review the semantic and JSON differences, then commit the golden with the source change:

```bash
brainbrew export crowdanki --manifest brainbrew.yaml --target de-standard --out goldens/de-standard
brainbrew verify --manifest brainbrew.yaml --target de-standard --media-root media/
```

Use `golden_allowlist` only after reviewing concrete differences and keep it as narrow as possible:

```yaml
golden_allowlist:
  - note_models[0].latex_pre
```
