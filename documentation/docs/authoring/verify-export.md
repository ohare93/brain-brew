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
11. release-strict media integrity by default for every media target: references, owner roots, canonical SHA-256 declarations, file existence, and matching bytes;
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

Referenced-but-undeclared media is an error and declared-but-unreferenced media is a warning. Every declaration must have a canonical 64-character lowercase hexadecimal SHA-256 matching the file. Ordinary declarations require an owning package media root; declarations with a package-relative `source` read from that package and do not require a root. Strict mode never silently degrades when an ordinary declaration lacks its root. In a federation, unqualified `--media-root media/` maps only the root package. Repeat the option as `--media-root <package-id>=<directory>` for every dependency that owns a final declaration; duplicate, unknown, and missing package mappings fail before reads. Refresh hashes after intentional root-workspace media edits with:

```bash
brainbrew media hash --manifest brainbrew.yaml --all-targets --media-root media/
```

For hashless/source-only fixtures, explicitly select development reference checking:

```bash
brainbrew verify --manifest brainbrew.yaml --all-targets --media-mode reference-only
```

This mode still validates all structured/raw references, declaration/output collisions, portable paths, and syntax of non-empty hashes. It skips roots and bytes, emits a prominent `NOT RELEASE-READY` warning/status in human and JSON output, and cannot be combined with `--media-root`. Targets without media are unaffected.

## Export CrowdAnki

```bash
brainbrew export crowdanki \
  --manifest brainbrew.yaml \
  --target de-standard \
  --media-root media/ \
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

Export copies the declared media set itself, so release scripts do not need a separate `cp media/*` step. Files present under a media root but not declared are not exported. Each ordinary declaration is read only from its final declaring package's authorized media root; a declaration with `source` is read beneath that owning package root. A same-named file under the root package cannot satisfy a dependency declaration. A development export without bytes requires `--media-mode reference-only`; it still rejects undeclared references and collisions before staging and reports that the artifact is not release-ready.

Export refuses an existing output directory by default. To rerun intentionally, pass `--force`: Brain Brew validates and stages the complete new tree, moves the old complete tree to a recovery backup, and publishes the stage as one clean directory replacement. This removes stale files without ever copying into the live output tree.

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
brainbrew export crowdanki --manifest brainbrew.yaml --target de-standard --out goldens/de-standard --force
brainbrew verify --manifest brainbrew.yaml --target de-standard --media-root media/
```

Use `golden_allowlist` only after reviewing concrete differences and keep it as narrow as possible:

```yaml
golden_allowlist:
  - note_models[0].latex_pre
```
