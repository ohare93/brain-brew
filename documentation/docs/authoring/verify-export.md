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
4. lock-file package resolution and hashes, including live re-hashing of local path sources;
5. dependency expansion;
6. target composition;
7. Canonical Deck validation;
8. rendered content validation for deck descriptions, card-template HTML fragments, and note-type CSS styling;
9. translation coverage policy, when configured or passed with `--translation-coverage`;
10. stale translations, warning by default and failing under strict translation coverage;
11. media references always, plus media file existence and SHA-256 hashes when `--media-root` is passed;
12. configured CrowdAnki golden checks.

For path-based package sources, `verify` re-hashes the live source tree instead of trusting only a cached source hash. If a local package source drifts after locking, `verify` reports the hash mismatch so CI catches the change.

> **Experimental:** Lock/package federation works today, but the `brainbrew.lock` format and `brainbrew lock` CLI surface may change incompatibly in any release until a real downstream consumer stabilizes them.

## Verify shipped HTML and CSS content

`verify` validates the content Anki renders after target composition and source-variable rendering:

- deck descriptions and card-template question/answer formats are checked as lightweight HTML fragments;
- note-type `styling` is checked for balanced CSS braces, parentheses, brackets, comments, and strings.

The HTML check is structural, not spec-grade: it balances tags while tolerating Anki mustache such as `{{Field}}` and `{{cloze:Text}}`, void elements such as `<br>` and `<img>`, and arbitrary entities. It does not lint attributes, CSS properties, semantics, links, or external references.

Escape hatch: if legacy Anki content renders correctly but triggers a false positive, run:

```bash
brainbrew verify --manifest brainbrew.yaml --target legacy-target --skip-content-validation
```

Prefer fixing the source when possible; the flag skips only this HTML/CSS structural sub-check.

## Verify media

```bash
brainbrew verify --manifest brainbrew.yaml --all-targets --media-root media/
```

Without `--media-root`, Brain Brew checks rendered field/template media references (`<img src>`, `[sound:]`, and related URL forms) against declarations. Referenced-but-undeclared media is an error; declared-but-unreferenced media is a warning.

With `--media-root`, Brain Brew also checks that every declared media file exists and that every declaration has a non-empty SHA-256 matching the file. In a federation, unqualified `--media-root media/` maps only the root package. Repeat the option as `--media-root <package-id>=<directory>` for every dependency that owns a final declaration; duplicate, unknown, and missing package mappings fail before reads. Refresh hashes after intentional root-workspace media edits with:

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

Export copies the declared media set itself, so release scripts do not need a separate `cp media/*` step. Files present under a media root but not declared are not exported. Each declaration is read only from its final declaring package's authorized root; a same-named file under the root package cannot satisfy a dependency declaration.

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
