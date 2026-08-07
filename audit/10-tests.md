# Test-suite false-confidence audit

> **Current resolution of the UG oracle finding:** This historical audit predates
> the pinned full fixture contract. The default formats integration suite now
> requires exact parsed CrowdAnki equality for all 100 UG targets, exact 74 + 26
> inventory, locked source/expected digests, and strict verification of every
> target against the single real 607-file media tree. A separate locked inventory
> proves exact attribution by 546 UG image rows, 56 HG image rows, and the UG
> notice for five runtime files. The absent external oracle no longer returns a
> passing optional test. Boundary tests reject source drift, count-preserving
> expected-value substitution, missing/extra targets, and attribution drift.
> Other findings below remain historical evidence and are not rewritten.

## Review

### Correct — what the suite genuinely proves

- The maintained default gate is broad: test listing found **404 non-browser tests** (79 core, 159 formats, 165 CLI, 1 native UI), plus **13 WebDriver tests**. There are **0 ignored tests**, **0 `.snap` files**, and no property/fuzz framework in the workspace.
- Core composition has substantial example coverage: overlay ordering/conflicts, add/merge/replace/remove/override families, translation dictionaries, stale records, target adaptations, structured messages/images, and translation coverage are exercised in `crates/brain-brew-core/tests/overlay_compose.rs` (55 tests). Canonical validation and content validation add 12 tests, but their negative matrix is incomplete as detailed below.
- Format coverage is strongest around deterministic positive behavior and fail-closed CrowdAnki fields: canonical/overlay/manifest/lock/media codecs, includes, hostile scalar corpora, import rejection cases, and one independent inline CrowdAnki JSON expectation are covered. In particular, `crates/brain-brew-formats/tests/crowdanki.rs:10-22` compares export against a hand-authored JSON value rather than only round-tripping through Brain Brew.
- CLI tests invoke the real binary for all principal commands: `fmt`, `validate`, `compose`, `diff`, `explain`, `targets`, `export`, `import`, `verify`, `lock`, `media`, `translations`, and `workbench`. `crates/brain-brew-cli/tests/cli_contract.rs:67-376` adds explicit stdout/stderr/exit contracts for validate, explain, diff, targets, help, version, and argument errors.
- Workbench coverage is real rather than mocked: 32 named server/API integration tests in `crates/brain-brew-cli/tests/cli.rs` and 13 browser scenarios in `crates/brain-brew-workbench-e2e/tests/workbench_smoke.rs:26-343`. `devenv shell e2e` passed all 13 against Chromium in 16.57s.
- UG fixtures exercise all 74 main targets and all 26 Hardcore companion targets for composition/validation (`crates/brain-brew-formats/tests/ultimate_geography_fixture.rs:142-181,335-410`), enforce the variable-first/shared-extension source shape (`:412-533`), and cover real-manifest/media browser smoke paths (`crates/brain-brew-workbench-e2e/tests/workbench_smoke.rs:276-320`). These are useful fixture and propagation checks; they are not an independent parity oracle.

### Coverage map

| Surface | Tested flows | Materially untested or weakly tested |
|---|---|---|
| Domain | Stable path parsing/globs; basic deck invariants; most overlay intents; conflict detection; translations/no-change/stale/adaptations; structured image lowering; semantic raw-field/note/tombstone examples | Semantic equality of every canonical property; structured-field overwrite guards; entity-level expected-base comparison; structured-message dependency chains/cycles; cross-kind tombstones; several validation error kinds; general algebraic laws |
| Formats | Canonical/overlay/manifest/lock/media parse/emit/idempotence; include resolution; unknown fields; fixed hostile strings; CrowdAnki import/export and many unsupported fields | Duplicate keys in dynamic maps; malformed mutually exclusive overlay payloads; arbitrary Unicode/control/key generation; parser panic resistance; independent full-format parity; several path-rich diagnostics |
| CLI | Real-binary happy/error flows for every principal command; output contracts for four JSON-capable commands; local path/file-tarball locks; media and translation workflows | Remote Git/GitHub lock end-to-end behavior; crash/interruption recovery; broad concurrent invocation/stress; platform-specific path/process behavior; exact contracts for many large human outputs |
| Workbench | Real loopback APIs, filesystem writes, atomic rename failure handling, two-request serialization; browser staging/refresh/apply, pivots, metadata, multi-pane edits, scaffold, UG load/media | Stale external-file/session rejection; browser-visible invalid preview/apply failure; localStorage corruption/quota/unavailable cases; deterministic unit tests for staging key/collection/clear logic; concurrent conflicting writers beyond two-request serialization |
| Ultimate Geography | Canonical source shape; 74 + 26 target composition; selected resolved content; mutation propagation through exporter; real UI smoke | Required CI release oracle; exact old-tool parity; Experimental/Hardcore release parity; full adapter defaults/config/model metadata; media bytes/hashes against release; independent target-by-target goldens |

## Prioritized findings and proposed regression tests

### Blocker — “strict YAML” duplicate coverage tests only struct fields, not dynamic maps

The only generic duplicate-key assertion is `base` repeated at `crates/brain-brew-formats/tests/manifest_yaml.rs:724-734`. That exercises a duplicate field on a serde struct. Most maintainer content is instead decoded into `BTreeMap` values (for example canonical entity maps in `crates/brain-brew-formats/src/canonical_yaml.rs:2316-2325`, manifest maps at `crates/brain-brew-formats/src/manifest.rs:624-639`, lock packages at `crates/brain-brew-formats/src/lockfile.rs:148-154`, and media-map roots at `crates/brain-brew-formats/src/media_map.rs:13-21`). There is no test for duplicate entity IDs, note fields, overlay changes/translations, targets, languages, packages, or media IDs. The positive canonical/idempotence suite therefore gives false confidence that all YAML source is strict: later dynamic-map values can overwrite earlier source without the existing test noticing.

**Highest-value regression:** a table-driven `rejects_duplicate_dynamic_map_keys` that feeds duplicate keys at every map-bearing schema level through both parse and `fmt`, asserts a nonzero CLI exit, asserts the original file is byte-unchanged, and checks a diagnostic containing the duplicate key and schema/file location. Include duplicate keys introduced by `!include`, not only inline YAML.

### Blocker — semantic round-trip oracles delegate to a demonstrably incomplete `semantic_diff`

`CanonicalDeck::semantic_diff` compares name, description, variables, note types, notes, media, and tombstones but omits deck ID and deck-level adapter IDs (`crates/brain-brew-core/src/compose.rs:45-67`). Note comparison examines raw `fields`, tags, variables, note-type ID, and adapter IDs, but never `field_messages` or `field_images` (`:1972-2014`, especially `:2001`). The dedicated tests cover only unchanged decks, one raw field, note add/remove, and a tombstone (`crates/brain-brew-core/tests/semantic_diff.rs:8-61`).

This weak oracle is reused by `import_export_round_trip_is_semantically_equal_when_suggested_ids_match_source` (`crates/brain-brew-formats/tests/crowdanki.rs:25-34`) and by the test explicitly named `structured_image_fields_survive_crowdanki_export_import_round_trip` (`:162-205`). Those tests can pass after loss/change of precisely the structured representation and deck adapter identity they claim to protect.

**Highest-value regressions:** 

1. A mutation table that changes exactly one canonical property at a time—deck ID, every adapter-ID scope, note type field/template details, structured-message format/component/reference, structured-image media ID, tags, media, and tombstones—and asserts a non-empty diff with an exact stable path.
2. A property test over generated valid decks: `a == b` iff `a.semantic_diff(&b).is_empty()`; also assert symmetry of emptiness.
3. CrowdAnki round-trip tests must directly compare all expected canonical fields/adapter IDs (or compare `CanonicalDeck` after explicitly documented normalization), rather than relying solely on semantic diff.

### High — the 55 composition examples miss the dangerous representation and graph cases

The large count obscures several high-risk holes:

- Field blank/expected-base logic reads only `Note.fields` (`crates/brain-brew-core/src/compose.rs:1124-1172`), while applying a raw/message/image payload deletes the other representations (`:1173-1205`). `field_level_merge_rejects_non_blank_for_structured_messages_and_images` tests a **structured incoming payload over a nonblank raw base** (`crates/brain-brew-core/tests/overlay_compose.rs:2268-2301`); it does not test add/merge over an already-populated structured-image/message base whose raw placeholder is blank.
- Entity-level destructive checks can stop at `expected_base.is_some()` (`crates/brain-brew-core/src/compose.rs:1619-1633`), but no stale/wrong-value tests exist for complete card-template, field-definition, note-type, or note replacement.
- Structured messages resolve all references against one cloned snapshot (`crates/brain-brew-core/src/messages.rs:133-159`), but tests cover direct successful/error substitutions, not multi-hop dependencies, order independence, self-reference, or cycles.
- Validation implements mismatched entity IDs and duplicate field/template detection (`crates/brain-brew-core/src/validate.rs:56-91`), while `crates/brain-brew-core/tests/canonical_deck_validation.rs:55-217` does not exercise those error kinds. Unknown structured media is tested at render/verify, not as a base validation/compose invariant.

**Highest-value regressions:** add/merge/field-fill over existing structured images and messages must fail without a representation-aware expected base; wrong entity expected values must fail after simulated upstream edits; message chains must resolve topologically and cycles must produce path-rich errors; one table should mutate a valid deck to trigger every `ValidationErrorKind` with exact paths. Add cross-kind same-ID tombstone and remove-history cases.

### High — Workbench has no stale external-file/session test, despite the governing contract

ADR-011 requires file fingerprints/polling for stale-session detection and validation before writes (`documentation/docs/reference/decisions/0011-use-a-local-deck-workbench-server-with-iced-wasm-ui.md:45-50`). ADR-014 requires browser tests to exercise staged local edits and Apply preview/validation (`documentation/docs/reference/decisions/0014-require-workbench-api-and-browser-e2e-tests.md:13-17`). Workspace fingerprints are displayed/tested, but `ApplyRequest` carries only language, target, overlay, and edits (`crates/brain-brew-cli/src/commands/workbench.rs:1636-1660`); apply reloads current context (`:878-890`) without an expected workspace fingerprint. Existing concurrency coverage proves mutex serialization of two requests (`crates/brain-brew-cli/tests/cli.rs:1221-1267`), not stale-draft conflict rejection.

A browser can stage a draft, another process can edit the same overlay, and the suite has no test requiring Apply to reject rather than overwrite/rebase unexpectedly.

**Highest-value regression:** stage a translation in the browser, mutate the overlay externally after staging, then click preview and Apply. Assert visible stale-session diagnostics, `data-validation-ok=false`, no write, preservation of the external bytes, and retention of the browser draft. Mirror it at API level with expected fingerprints, including an unrelated-file edit and a conflicting same-path edit so the intended policy is explicit.

### High — UG “parity” is skipped as a passing test in normal CI and compares only a subset when present

`ultimate_geography_fixture_exports_match_release_oracle_semantics_when_available` returns success when the oracle is absent (`crates/brain-brew-formats/tests/ultimate_geography_fixture.rs:536-547`). The audited checkout has no oracle, and CI only runs `devenv shell ci` without fetching one (`.github/workflows/ci.yml:10-26`). Thus the default run reports this test as `ok`, not ignored/skipped, while performing no parity comparison.

When an oracle is supplied, the custom comparator checks selected deck/model/note values (`crates/brain-brew-formats/tests/ultimate_geography_fixture.rs:1798-1855`), omitting much of CrowdAnki JSON such as deck configuration/defaults, field attributes, template metadata beyond name/qfmt/afmt, note data/flags, and any additional note model. It also filters to standard/extended targets (`:551-565`), excluding experimental and Hardcore release flows.

The all-target mutation tests are metamorphic propagation checks, not independent parity: they export a Brain Brew baseline and a Brain Brew-mutated deck with the same exporter and compare their path delta (`:1468-1511`). A shared baseline/export bug survives both sides.

**Highest-value regression:** commit immutable old-tool/release `deck.json` goldens for a bounded representative matrix (source/RTL/CJK languages × standard/extended, plus Hardcore where a release oracle exists), configure manifest golden checks, and make their presence mandatory in CI. Use `compare_deck_json_values` with narrow documented allowlists and assert exact unexpected-path emptiness. Keep all-target identity/count/hash invariants separately; label mutation tests “propagation” rather than parity.

### High — malformed overlay union branches have no negative matrix and can silently discard supplied data

`FieldChangeYaml` independently accepts scalar `value`, positional `message`, formatted `format`, and `variables`; formatted input takes precedence over positional message, while scalar value remains independently populated (`crates/brain-brew-formats/src/canonical_yaml.rs:2176-2225`). `MediaChangeYaml` turns every partial `(path, sha256)` pair into `media: None` (`:2249-2275`). The tail of the overlay tests covers valid media/remove and one unknown top-level field only (`crates/brain-brew-formats/tests/overlay_yaml.rs:850-883`), not conflicting/missing payload alternatives. Format/parse “success” can therefore normalize away author intent.

**Highest-value regression:** exhaustively reject each conflicting pair and all-three field payloads, `variables` without `format`, remove-with-payload, zero-payload non-remove, and media with only path or only hash. Exercise `overlay_from_str`, `overlay_format_str`, and CLI `fmt`, asserting path-specific errors and unchanged files.

### Medium — no property, fuzz, panic-resistance, or mutation testing exists

The format crate has only serde/JSON/YAML/hash dependencies (`crates/brain-brew-formats/Cargo.toml:12-17`). Adversarial coverage is a fixed corpus of 7 hostile values/6 keys (`crates/brain-brew-formats/tests/emitter_roundtrip.rs:17-34`) plus 20 hostile strings (`crates/brain-brew-formats/tests/yaml_scalar_adversarial.rs:17-41`). This is good regression coverage but cannot explore combinations of Unicode, controls, block chomping, map-key grammar, nested includes, or arbitrary malformed JSON/YAML. The custom DeckPath/glob/content/media parsers likewise have only handpicked tables. No fuzz target or generated law test was found.

**Highest-value additions:**

- `proptest` valid model generators for YAML/overlay/manifest/lock/media round-trip and formatter idempotence.
- Generated DeckPath display/parse round-trip and glob matcher comparison against a small independent reference implementation.
- Fuzz targets for canonical/overlay YAML parse+format, CrowdAnki import/comparator, media HTML/CSS reference extraction, and content validation with the invariant “never panic/hang.”
- Mutation testing on semantic diff, expected-base checks, and fail-closed adapter fields to measure whether assertions kill omitted-branch mutants.

### Medium — UG propagation dominates the default suite and duplicates expensive baselines

The full default run passed 404 tests in **189.075s**; `ultimate_geography_fixture` alone took **162.27s**, with five tests reported as running over 60 seconds. The helper recomposes and reexports every target baseline for every mutation case (`crates/brain-brew-formats/tests/ultimate_geography_fixture.rs:1468-1486`). The seven mutation groups contain 26 helper calls, so the 74-target fixture repeatedly performs roughly 1,924 baseline/mutation pairs; identical baselines are recomputed across cases and parallel tests.

This makes the suite slower without adding independent oracle strength and encourages developers to run narrower commands.

**Highest-value restructure:** cache each target’s composed deck/export once per test executable; run each isolated mutation against a representative target matrix, then use one combined overlay or invariant scan for all 74 targets. Keep a separately named exhaustive scheduled/CI tier if full combinatorics are required. Preserve the fast all-target compose/validate test and mandatory independent parity goldens.

### Medium — browser staging behavior has almost no deterministic unit coverage

The UI crate has one native test, only for `WorkspaceSummary` (`crates/brain-brew-workbench-ui/tests/workspace_summary.rs:1-17`), while the localStorage staging implementation is WASM-only (`crates/brain-brew-workbench-ui/src/staging.rs:1-163`). Browser happy paths cover persistence and clearing indirectly, but there are no focused tests for key delimiter collisions, prefix isolation, malformed stored JSON, deterministic edit ordering, unavailable storage, set/remove errors, or quota failures. `collect_staged_edits_for_prefixes` silently skips malformed entries and uses browser storage enumeration order (`:82-105`), yet Apply ordering and diagnostics are not specified by tests.

**Highest-value regression:** extract pure key/edit selection and ordering logic for native unit tests; add `wasm-bindgen-test` cases with a storage facade for malformed/unavailable/quota scenarios; add one browser negative test showing an invalid preview and ensuring Confirm Apply cannot mutate files.

### Note — no brittle snapshot/ignored-test problem was found, but “golden” strength varies

There are no Rust snapshot files or ignored tests. Exact canonical string assertions and the inline `EXPECTED_CROWDANKI_JSON` are intentional format contracts, not incidental UI snapshots. The main weakness is the opposite: too few independent checked-in goldens, optional parity silently reported as success, and several round-trip/metamorphic assertions that share implementation logic.

### Residual risks

- No line/branch coverage tool is installed in the environment, so this is a test-list plus source-to-test mapping audit rather than instrumented coverage.
- No mutation campaign or fuzz run was performed; absence was established from workspace manifests/source and repository layout.
- Remote Git/GitHub locking, non-Linux platforms, process interruption, and filesystem permission/race matrices remain largely outside hermetic CI coverage.
- `plan.md` and `progress.md` were requested inputs but do not exist at the supplied paths; the audit used project guidance, ADRs, source, tests, fixtures, and CI definitions instead.
- Review-only constraints prevented adding the proposed regression tests or fixing the exposed implementation gaps.

```acceptance-report
{
  "criteriaSatisfied": [
    {
      "id": "criterion-1",
      "status": "satisfied",
      "evidence": "The review maps domain, format, CLI, Workbench, and Ultimate Geography coverage and reports prioritized Blocker/High/Medium gaps with exact file:line references and concrete regression-test proposals."
    }
  ],
  "changedFiles": [],
  "testsAddedOrUpdated": [],
  "commandsRun": [
    {
      "command": "cargo test --workspace --exclude brain-brew-workbench-e2e --all-targets -- --list",
      "result": "passed",
      "summary": "Listed 404 default-gate tests across core, formats, CLI, and UI."
    },
    {
      "command": "cargo test --workspace --exclude brain-brew-workbench-e2e --all-targets",
      "result": "passed",
      "summary": "404 tests passed in 189.075s; UG fixture took 162.27s and its absent-oracle parity test returned success without comparing."
    },
    {
      "command": "cargo test -p brain-brew-workbench-e2e -- --list",
      "result": "passed",
      "summary": "Listed 13 browser E2E scenarios."
    },
    {
      "command": "devenv shell e2e",
      "result": "passed",
      "summary": "Built the WASM UI, launched Chromium/chromedriver, and passed all 13 browser tests in 16.57s."
    },
    {
      "command": "repository scans for #[ignore], .snap, and property/fuzz frameworks",
      "result": "passed",
      "summary": "Found 0 ignored tests, 0 snapshot files, and no proptest/quickcheck/fuzz/loom framework usage."
    },
    {
      "command": "jj status && jj diff --stat",
      "result": "passed",
      "summary": "Confirmed the Jujutsu working copy already contained other requested audit artifacts; no source/test edits were made."
    }
  ],
  "validationOutput": [
    "default test listing: 404 tests",
    "default suite: 404 passed, 0 failed, 0 ignored",
    "Workbench browser suite: 13 passed, 0 failed",
    "Ultimate Geography release oracle: absent; parity body skipped via early return",
    "ignored tests: 0; snapshot files: 0; property/fuzz frameworks: 0"
  ],
  "residualRisks": [
    "No instrumented line/branch coverage or mutation score was available.",
    "The external UG release oracle was absent, so actual release parity was not validated.",
    "No proposed regression tests were added because this assignment was review-only.",
    "plan.md and progress.md were absent at the requested paths."
  ],
  "noStagedFiles": true,
  "notes": "Only the requested audit report was written; changedFiles/testsAddedOrUpdated remain empty because no product or test source was modified. Jujutsu has no staging area."
}
```
