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
9. media references and SHA-256 hashes, when `--media-root` is passed;
10. configured CrowdAnki golden checks.

## Verify media

```bash
brainbrew verify --manifest brainbrew.yaml --all-targets --media-root media/
```

With `--media-root`, Brain Brew checks that referenced media files exist and match their declared SHA-256 hashes.

## Export CrowdAnki

```bash
brainbrew export crowdanki \
  --manifest brainbrew.yaml \
  --target de-standard \
  --out build/crowdanki/de-standard
```

With media copied into the CrowdAnki folder's `media/` subdirectory:

```bash
brainbrew export crowdanki \
  --manifest brainbrew.yaml \
  --target de-standard \
  --media-root media/ \
  --out build/crowdanki/de-standard
```

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
- Translation coverage tests distinguish stale source keys, strict-mode missing translations, path-specific overrides, reviewed no-change text, and target-language additions.
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
