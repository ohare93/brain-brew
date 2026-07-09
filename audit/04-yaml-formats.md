# Canonical YAML formats audit

## Review

### Blocker — duplicate keys in schema maps are accepted and silently overwrite maintainer source

`deny_unknown_fields` is present on the containing structs, but entity/content maps are deserialized directly into `BTreeMap` (for example overlay maps at `crates/brain-brew-formats/src/canonical_yaml.rs:1406-1428`, note field changes at `:2122-2135`, deck entity maps at `:2316-2325`, manifest maps at `crates/brain-brew-formats/src/manifest.rs:624-639`, lock package maps at `crates/brain-brew-formats/src/lockfile.rs:148-154`, and the media-map root at `crates/brain-brew-formats/src/media_map.rs:13-21`). Duplicate keys in those maps are not rejected; the later value wins. This violates the strict-decoding and round-trip commitments in ADR-005 (`documentation/docs/reference/decisions/0005-store-maintainer-source-as-strict-canonical-yaml.md:13-24`) and the user-facing claim that canonical YAML is deliberately strict (`documentation/docs/concepts/canonical-deck.md:54-62`).

Verified probes using `target/debug/brainbrew fmt`:

- Two `field.capital` entries in one overlay note formatted successfully and retained only the second change.
- Two `media.flag.fi` entries in a standalone media map formatted successfully and retained only the second declaration.

The existing duplicate-key test covers only duplicate struct fields (`base` twice) at `crates/brain-brew-formats/tests/manifest_yaml.rs:724-734`; it does not exercise duplicate keys inside dynamic maps.

**Required fix/test:** use duplicate-detecting map deserialization (including nested maps and mappings first parsed through `serde_yaml::Value`) and add rejection tests for canonical entity IDs, note fields, overlay entities/field changes/translations, manifest overlays/targets/languages, lock packages, media-map IDs, and included media mappings. Diagnostics should identify the duplicate key and YAML location.

### Blocker — contextual canonicalization can emit duplicate YAML and lose a valid translation on the second format pass

The formatter groups contexts beginning with `notes.note.` / `note_types.note-type.` as child nodes (`crates/brain-brew-formats/src/canonical_yaml.rs:819-853`) and then emits a node's translation keys followed by child keys at the same indentation (`:855-874`). A source key can equal a child suffix, so duplicate-free accepted input can become duplicate-key output.

Reproducer:

```yaml
translations:
  contextual:
    notes.note:
      denmark: Denmark label
    notes.note.denmark:
      Country: Danmark
```

The first `brainbrew fmt` emitted two `denmark:` keys under `notes.note`; the second `fmt` silently removed `denmark: Denmark label` because of the duplicate-map behavior above. This directly contradicts byte-idempotent canonicalization. The UG fixture checks only its current non-colliding data (`crates/brain-brew-formats/tests/ultimate_geography_fixture.rs:183-229`), so it does not cover the legal key-space collision.

**Required fix/test:** make the contextual representation/emitter unambiguous (or reject collisions before writing), and add parse → format → parse semantic-equality plus first-format/second-format byte-idempotence tests where a contextual source equals a descendant context segment for both grouped prefixes.

### High — malformed overlay alternatives are accepted, and formatting silently discards supplied data

Overlay decoding models mutually exclusive payloads as independent optional fields without enforcing exclusivity:

- `FieldChangeYaml` accepts `value`, `message`, `format`, and `variables` together (`crates/brain-brew-formats/src/canonical_yaml.rs:2176-2190`). If `format` is present, positional `message` is ignored (`:2192-2205`); orphan `variables` are ignored when `format` is absent; a scalar `value` can coexist with a structured message (`:2206-2220`). Compose then gives the message precedence and ignores the scalar (`crates/brain-brew-core/src/compose.rs:1178-1205`).
- `MediaChangeYaml` accepts `path` and `sha256` independently and converts every partial pair to `media: None` (`crates/brain-brew-formats/src/canonical_yaml.rs:2249-2275`).

Verified probes:

- An overlay field containing scalar `value`, `format`, `variables`, and positional `message` formatted successfully but dropped the positional message; the scalar remained in YAML even though compose ignores it.
- An overlay field containing only orphan `variables` formatted successfully and removed the variables.
- An add-media change containing `path` but no `sha256` formatted successfully and removed the path.

This is more severe than normalization of hand formatting: intentional schema data is discarded. Existing overlay tests reach only unknown top-level fields (`crates/brain-brew-formats/tests/overlay_yaml.rs:872-883`) and valid complete media (`:850-851`).

**Required fix/test:** reject conflicting field payload branches, reject `variables` without `format`, require `path` and `sha256` together, and test every malformed combination through both `overlay_from_str` and `overlay_format_str` with path-specific diagnostics and unchanged input files on CLI failure.

### Medium — YAML booleans, nulls, and numbers are coerced into strings instead of failing the typed schema

Most schema text properties use plain `String` deserialization (for example canonical deck properties at `crates/brain-brew-formats/src/canonical_yaml.rs:2371-2380`), and field values explicitly try `serde_yaml::from_value::<String>` (`:2525-2569`). `serde_yaml` coerces non-string scalars. Probes replacing a deck description with unquoted `true`, `null`, `123`, `1.5`, and `0x10` all formatted successfully into quoted strings (`'true'`, `'null'`, etc.).

The emitter correctly quotes ambiguous strings (`crates/brain-brew-formats/src/yaml_scalar.rs:63-94`), but the decoder does not enforce that source values were YAML strings. In a format whose ADR explicitly calls for careful handling of ambiguous YAML values (`documentation/docs/reference/decisions/0005-store-maintainer-source-as-strict-canonical-yaml.md:28-30`), a missing value (`description:`/null) becoming the literal text `"null"` is especially surprising.

**Missing tests:** reject non-string YAML scalar types for all string schema positions (content, IDs, paths, hashes, adapter IDs, versions, and translation keys/values), while continuing to accept explicitly quoted lookalikes. If coercion is intentional, document it explicitly and test every accepted conversion; strict-source wording currently implies rejection.

### Medium — public emitters can panic on constructible domain values instead of returning an error

Canonical deck emission is fallible and prevalidates keys, but overlay, manifest, and lock emitters are infallible public APIs. Their key helper calls `expect` (`crates/brain-brew-formats/src/canonical_yaml.rs:810`, `crates/brain-brew-formats/src/manifest.rs:281-315`, `crates/brain-brew-formats/src/lockfile.rs:54-85`). Public domain structs contain unrestricted `String` map keys, so a programmatically constructed overlay/manifest/lock with a newline key panics. Parse-path tests do not cover direct emission; `crates/brain-brew-formats/tests/emitter_roundtrip.rs:149-171` only verifies rejection while parsing (except canonical deck, whose emitter is already fallible).

**Required fix/test:** make these emitters fallible and run the same prevalidation as their parsers, or make invalid keys unconstructible. Add `catch_unwind`-free tests asserting typed errors from direct emission for every unrestricted key location.

### Low — include behavior and documentation have drifted, and invalid safe roots are silently ignored

The reference says formatting scalar `!include` values materializes included content (`documentation/docs/reference/yaml.md:220-222`), while the implementation explicitly formats without materializing and restores include directives (`crates/brain-brew-formats/src/source_includes.rs:49-80`). CLI tests enforce preservation and non-inlining (`crates/brain-brew-cli/tests/cli.rs:5218-5259`), as do format-codec tests (`crates/brain-brew-formats/tests/canonical_yaml.rs:610-623`). The docs should describe the behavior users actually receive.

Separately, configured safe include roots that cannot be canonicalized are silently skipped (`crates/brain-brew-formats/src/source_includes.rs:340-359`). A typo/nonexistent root then produces a misleading escape error later rather than identifying the invalid manifest root. The path containment check does correctly reject absolute paths and canonicalized symlink escapes (`:525-566`).

**Missing tests:** invalid/nonexistent/unreadable safe roots with actionable diagnostics; scalar-include behavior as documented; and an explicit overlay `media: !include` rejection test (the reference says structural media includes are deck-only at `documentation/docs/reference/yaml.md:220`, but the resolver recognizes structural includes solely by YAML path at `source_includes.rs:386-389`).

### Low — conversion diagnostics often omit the schema path and source location

Serde shape errors generally carry line/column, but conversion errors such as `StableId`, `InvalidChangeIntent`, `InvalidExpectedBase`, and ordered-entity failures are returned after deserialization without retaining YAML location or a full schema path. For example, all IDs pass through unscoped `sid(...)` calls during map conversion (`crates/brain-brew-formats/src/canonical_yaml.rs:1431-1455`, `:2328-2363`). In a large UG source, `invalid stable id "..."` is insufficient to identify whether the bad value was an entity key, reference, order entry, or tombstone.

**Missing tests:** nested malformed IDs/intents/order entries asserting file/schema path plus line/column (or, at minimum, an exact dotted schema path). Consider path-aware deserialization and path-bearing conversion error variants.

### Correct / verified strengths

- Unknown fields are denied broadly on schema structs, explicit order arrays are validated, stable scalar emission is conservative, and hostile control/newline values have round-trip coverage.
- The focused codec suite passed: 83 tests across canonical deck, overlay, emitter, scalar, manifest, lock, and media-map tests.
- Ultimate Geography remains a strong positive fixture: canonical checked-in YAML, byte idempotence, and all 74 main-manifest targets composed and validated in the focused runs (`crates/brain-brew-formats/tests/ultimate_geography_fixture.rs:142-229`). The fixture's current clean key-space does not exercise the blockers above.
- `plan.md` and `progress.md` requested as inputs were not present at the supplied repository paths; review proceeded from ADRs, format docs, source, tests, and fixtures.

## Acceptance report

```acceptance-report
{
  "criteriaSatisfied": [
    {
      "id": "criterion-1",
      "status": "satisfied",
      "evidence": "Review-only task completed without modifying project/source files; only audit/04-yaml-formats.md was created."
    },
    {
      "id": "criterion-2",
      "status": "satisfied",
      "evidence": "Findings include exact code/doc/test references, reproducible formatter probes, focused test results, and explicit missing-test recommendations."
    }
  ],
  "changedFiles": [
    "audit/04-yaml-formats.md"
  ],
  "testsAddedOrUpdated": [],
  "commandsRun": [
    {
      "command": "devenv shell -- cargo test -p brain-brew-formats --test canonical_yaml --test overlay_yaml --test emitter_roundtrip --test yaml_scalar_adversarial --test manifest_yaml --test lockfile_yaml --test media_map",
      "result": "passed",
      "summary": "83 focused codec/formatting tests passed (24 + 21 + 4 + 4 + 20 + 5 + 5)."
    },
    {
      "command": "devenv shell -- cargo test -p brain-brew-formats --test ultimate_geography_fixture ultimate_geography_fixture_yaml_sources_are_checked_in_canonical",
      "result": "passed",
      "summary": "UG checked-in YAML canonicality test passed."
    },
    {
      "command": "devenv shell -- cargo test -p brain-brew-formats --test ultimate_geography_fixture ultimate_geography_fixture_formatting_is_byte_idempotent",
      "result": "passed",
      "summary": "UG fixture byte-idempotence test passed."
    },
    {
      "command": "devenv shell -- cargo test -p brain-brew-formats --test ultimate_geography_fixture ultimate_geography_fixture_manifest_composes_all_targets",
      "result": "passed",
      "summary": "All 74 UG main-manifest targets composed and validated."
    },
    {
      "command": "target/debug/brainbrew fmt <temporary malformed/duplicate YAML probes>",
      "result": "failed-review",
      "summary": "CLI returned success while overwriting duplicate map entries, generated then consumed a contextual duplicate key, and discarded partial/conflicting overlay payload data; scalar-type probes were coerced to strings."
    },
    {
      "command": "jj status && jj diff --summary",
      "result": "passed",
      "summary": "Pre-review working copy had no changes."
    }
  ],
  "validationOutput": [
    "Duplicate overlay field key retained only the second value after fmt.",
    "Duplicate media-map ID retained only the second declaration after fmt.",
    "Contextual collision first fmt emitted duplicate denmark keys; second fmt removed the scalar translation.",
    "Partial media change lost its path; orphan/conflicting structured-message branches were dropped or shadowed.",
    "Unquoted true/null/123/1.5/0x10 deck descriptions were accepted and rewritten as quoted strings.",
    "UG canonicality, idempotence, and 74-target composition checks passed."
  ],
  "residualRisks": [
    "No release-oracle CrowdAnki parity run was requested or performed; this audit focused on YAML codecs.",
    "Malformed-input probes used temporary files and are not permanent regression tests.",
    "Lock/package federation is documented experimental, but duplicate-map and emitter-panic behavior also affects non-experimental canonical/overlay/manifest surfaces.",
    "plan.md and progress.md were absent at the requested paths."
  ],
  "noStagedFiles": true,
  "notes": "Review gate should block on both duplicate-key findings and the overlay data-loss finding before accepting strict canonical YAML behavior."
}
```
