# CrowdAnki import/export and round-trip audit

## Review

### Blocker

1. **CrowdAnki import cannot import non-Latin decks whose first fields contain no ASCII, and the advertised correction path does not exist.**
   - Note stable IDs are derived only from the first field (`crates/brain-brew-formats/src/crowdanki.rs:1181-1187`). `slugify` retains only ASCII alphanumerics and maps an all-non-ASCII value to `unnamed` (`crates/brain-brew-formats/src/crowdanki.rs:1289-1307`). The second such note therefore collides in the deck map (`crates/brain-brew-formats/src/crowdanki.rs:867-882`).
   - Focused probe against the actual UG-generated `ru-standard/deck.json` failed on the first two Russian names: both derived `note.unnamed`. By contrast, the generated Chinese values happen to include English parentheticals and imported successfully; this is accidental rather than Unicode-safe.
   - The collision diagnostic says to correct a “suggested-ID override path” (`crates/brain-brew-formats/src/crowdanki.rs:1212-1214`), but the public format API takes only JSON (`crates/brain-brew-formats/src/crowdanki.rs:84-88`) and the CLI exposes only blanket `--accept-suggested-ids` (`crates/brain-brew-cli/src/commands/import.rs:8-35`). There is no override/review artifact or retry input.
   - This conflicts with ADR-004’s requirement that maintainers be able to review and correct suggested IDs before they become canonical (`documentation/docs/reference/decisions/0004-identify-deck-entities-with-human-readable-stable-ids.md:13-18,41-46`). It also makes collisions from ordinary repeated first fields unresolvable without mutating the CrowdAnki input itself.
   - **Required direction:** make suggestion generation Unicode-capable and/or GUID-assisted, return a reviewable import plan, and implement an explicit stable-ID override input before requiring acceptance.

### High

2. **Duplicate note GUIDs are accepted and re-exported, so adapter identity is not fail-closed.**
   - The importer checks duplicate note-model UUIDs (`crates/brain-brew-formats/src/crowdanki.rs:838-864`) but keys notes only by generated stable ID (`crates/brain-brew-formats/src/crowdanki.rs:867-882`); it never tracks GUID uniqueness. Core validation likewise checks duplicate stable IDs, not duplicate adapter-ID values (`crates/brain-brew-core/src/validate.rs:67-91`).
   - A focused probe cloned one actual UG note, changed its first field so the stable ID differed, retained the original GUID, and imported it. The command exited 0 and emitted two notes with the same `crowdanki:guid`. Export then uses that value directly (`crates/brain-brew-formats/src/crowdanki.rs:641-649,686-691`). Such output cannot safely preserve Anki note identity/update behavior.
   - Existing tests cover duplicate note-model UUIDs and stable-ID collisions, but not duplicate/empty note GUIDs (`crates/brain-brew-formats/tests/crowdanki.rs:245-329`).
   - **Required direction:** reject duplicate or invalid GUIDs on import and reject any duplicate effective export GUID, including collisions between explicit GUIDs and stable-ID fallbacks.

3. **Malformed template ordinals are silently normalized, changing card/template identity and order.**
   - Field ordinals are validated against their positions (`crates/brain-brew-formats/src/crowdanki.rs:995-1006,1051-1066`), but templates are merely sorted by `ord`; `validate_supported_defaults` never checks ordinal uniqueness or contiguity (`crates/brain-brew-formats/src/crowdanki.rs:1008-1023,1085-1100`). Export discards imported ordinals and enumerates the canonical vector from zero (`crates/brain-brew-formats/src/crowdanki.rs:597-613`).
   - Probe: changing UG’s first template ordinal from `0` to `99` imported successfully. Input order/ordinals were `Country - Capital:99, Capital - Country:1, Flag - Country:2, Map - Country:3`; re-export became `Capital - Country:0, Flag - Country:1, Map - Country:2, Country - Capital:3`.
   - This violates ADR-010’s fail-closed rule for unsupported adapter data (`documentation/docs/reference/decisions/0010-fail-closed-on-unsupported-adapter-data.md:13-17,41-45`). There is no test for duplicate, gapped, or out-of-range template ordinals; current template fail-closed tests only cover browser-format fields (`crates/brain-brew-formats/tests/crowdanki.rs:619-662`).
   - **Required direction:** after sorting, require ordinals to be exactly `0..n`, or model/preserve the adapter ordinal explicitly.

4. **UG parity evidence is not wired into the maintained verification gate and is too weak to establish full CrowdAnki parity.**
   - Neither `fixtures/ultimate-geography/brainbrew.yaml` nor `brainbrew-hardcore.yaml` configures `targets.*.exports.crowdanki.golden`; the fixture contains zero JSON oracles. The target block begins at `fixtures/ultimate-geography/brainbrew.yaml:355`, and its entries contain only overlay selections (for example `:356-413`). Consequently, `brainbrew verify ... --target en-standard` reports success without comparing CrowdAnki output: golden verification runs only when a target has both a CrowdAnki export and `golden` (`crates/brain-brew-cli/src/commands/verify.rs:303-320`).
   - The actual external UG repository has a useful historical report based on Python Brain Brew 0.3.11 (`/home/jmo/Development/external/ultimate-geography/docs/pr736-equivalence-evidence.md:1-26`), but it covers only 12 representative rows (`:36-51`) while current manifests expose 74 UG and 26 Hardcore targets. The script explicitly compares selected semantics, not full JSON (`/home/jmo/Development/external/ultimate-geography/scripts/collect-pr736-equivalence-evidence.py:2-15`); its “model signature” is only `(model name, template count)` (`:300-301`) and omits template HTML/order, CSS, field definitions/order, config defaults, tags, and media path sets (`:319-355`).
   - The in-repo fixture also intentionally differs from the actual checkout: it has structured `!image` fields and a hoisted `media.yaml`, while the external checkout still has raw image HTML and inline media (for example `fixtures/ultimate-geography/deck.yaml:83-91,5527` versus `/home/jmo/Development/external/ultimate-geography/deck.yaml:83-91,5515`). That is a useful migration fixture, but not an independent current-source oracle.
   - **Required direction:** commit immutable old-tool `deck.json` oracles for a bounded representative matrix and configure manifest goldens. Use the exact JSON comparator with narrowly documented allowlists, plus explicit full-matrix identity/count checks where storing every target is impractical.

### Medium

5. **Core semantic diff omits data used by round-trip tests, allowing false “semantic equality.”**
   - `CanonicalDeck::semantic_diff` compares deck name, description, and variables but not deck stable ID or deck adapter IDs (`crates/brain-brew-core/src/compose.rs:45-67`). Note comparison calls `diff_note_fields` only for raw `Note.fields` (`crates/brain-brew-core/src/compose.rs:1972-2013`), while the model also stores `field_messages` and `field_images` (`crates/brain-brew-core/src/model.rs:1117-1127`); neither structured map is compared (`crates/brain-brew-core/src/compose.rs:2017-2052`).
   - The principal format round-trip assertion relies on this incomplete diff (`crates/brain-brew-formats/tests/crowdanki.rs:25-34`), as does the structured-image round trip (`:163-205`) and CLI UG-style round trip (`crates/brain-brew-cli/tests/ug_style_fixture.rs:113-124`). For example, the basic fixture omits deck-config adapter IDs, export synthesizes them, import adds them, and the test still passes because deck adapter IDs are invisible to semantic diff.
   - **Required direction:** include deck ID/adapter IDs and structured field representations in semantic diff, or make round-trip tests compare a complete, explicitly normalized export projection instead of treating the current diff as complete.

6. **The documented import command does not run.**
   - CLI reference shows `brainbrew import crowdanki ... --out deck.yaml` without the mandatory acceptance flag (`documentation/docs/reference/cli.md:93-99`), while the implementation rejects every invocation lacking `--accept-suggested-ids` (`crates/brain-brew-cli/src/commands/import.rs:15-18`). Media documentation does mention the flag (`documentation/docs/concepts/media.md:57-60`), so the public docs are internally inconsistent.
   - This should be corrected together with the suggested-ID review workflow, not by merely normalizing blanket acceptance as the long-term interface.

### Correct

- Import decoding is strict at each CrowdAnki object layer via `#[serde(deny_unknown_fields)]` (`crates/brain-brew-formats/src/crowdanki.rs:786-806,934-955,1039-1049,1069-1083,1103-1113`).
- Unsupported child decks, scheduling headers, cloze models, non-default model/field/browser options, opaque note data/flags, unknown model UUIDs, and field-count mismatches are rejected (`crates/brain-brew-formats/src/crowdanki.rs:808-830,958-989,1051-1066,1085-1100,1141-1178`). The 42 focused format tests exercise most of these paths and passed.
- Export determinism is structurally sound for supported data: BTree-backed collections drive note/model/media order, field and template vectors preserve canonical order, tags are set-sorted, JSON is pretty-serialized with a final newline (`crates/brain-brew-formats/src/crowdanki.rs:29-80,571-649`). The deterministic byte test passed (`crates/brain-brew-formats/tests/crowdanki.rs:9-23`).
- Raw template/CSS/field HTML is carried as strings. Strict whole-field image HTML is reverse-mapped conservatively, while mixed/non-strict/ambiguous HTML remains raw; positive and negative tests cover that behavior (`crates/brain-brew-formats/tests/crowdanki.rs:36-160`).
- Media export with `--media-root` validates hashes/files and copies only declarations (`crates/brain-brew-cli/src/commands/export.rs:86-101`); this is documented as conditional (`documentation/docs/concepts/media.md:61-73`).

## Test gaps, ranked

1. **High:** import and export rejection for duplicate/empty GUIDs and effective GUID fallback collisions.
2. **High:** Russian/non-Latin and repeated-first-field import with a real review/override flow.
3. **High:** template `ord` duplicate/gap/out-of-range cases and shuffled-but-valid ordinals.
4. **High:** immutable independent UG CrowdAnki goldens covering at least standard, translated non-Latin, extended, experimental, and Hardcore/companion outputs; assert template HTML/CSS, field/template order, tags, media paths, config, GUID/model UUIDs, and note fields.
5. **Medium:** semantic-diff tests proving deck IDs/adapter IDs, structured messages, and structured image references produce differences.
6. **Medium:** import/export of a real folder with media assets, followed by media hash/verification, so the expected asset handoff is explicit.
7. **Low:** CLI documentation/contract test that every documented import example includes the currently required acceptance/review arguments.

## Commands and validation

- `devenv shell cargo test -p brain-brew-formats --test crowdanki` — **passed**, 42/42.
- `devenv shell cargo test -p brainbrew --test ug_style_fixture` — **passed**, 1/1.
- `target/debug/brainbrew verify --manifest fixtures/ultimate-geography/brainbrew.yaml --target en-standard` — **passed**, but no golden was configured, which is part of finding 4.
- Mutated actual UG `ru-standard/deck.json` import — **reproduced blocker**, failed because two Cyrillic first fields both became `note.unnamed`.
- Mutated actual UG duplicate-GUID import — **reproduced defect**, exited 0 and emitted duplicate `crowdanki:guid` values.
- Mutated actual UG template-ordinal import/export — **reproduced defect**, exited 0 and renumbered/reordered templates.
- `jj status`; `jj diff --stat` before report creation — clean working copy, 0 changed files. Final status also showed concurrent `audit/02-core-correctness.md` and `audit/04-yaml-formats.md` additions from other review tasks; this task wrote only `audit/05-crowdanki.md`. `git diff --cached --name-only` was empty.

## Residual risks

- I did not rerun the external historical Python 0.3.11 equivalence script: it bootstraps external dependencies and its checked-in report already records the last representative run. I inspected its implementation and report instead.
- I ran one in-repo UG target rather than all 74+26 targets because no target has a configured independent golden; running all would validate composition/media declarations but would not close the parity gap.
- The external UG working copy contains unrelated uncommitted files, so it was treated as read-only evidence and not as a clean acceptance baseline.

```acceptance-report
{
  "criteriaSatisfied": [
    {
      "id": "criterion-1",
      "status": "satisfied",
      "evidence": "Completed the requested review-only CrowdAnki audit across core, formats, CLI, in-repo UG fixtures, and the external Ultimate Geography checkout; only audit/05-crowdanki.md was written."
    },
    {
      "id": "criterion-2",
      "status": "satisfied",
      "evidence": "Findings cite implementation, ADR, test, fixture, and external-oracle line ranges and include focused reproductions for non-Latin IDs, duplicate GUIDs, and template ordinal normalization."
    }
  ],
  "changedFiles": [
    "audit/05-crowdanki.md"
  ],
  "testsAddedOrUpdated": [],
  "commandsRun": [
    {
      "command": "devenv shell cargo test -p brain-brew-formats --test crowdanki",
      "result": "passed",
      "summary": "42 CrowdAnki format tests passed."
    },
    {
      "command": "devenv shell cargo test -p brainbrew --test ug_style_fixture",
      "result": "passed",
      "summary": "UG-style CLI compose/export/import semantic-diff test passed."
    },
    {
      "command": "target/debug/brainbrew verify --manifest fixtures/ultimate-geography/brainbrew.yaml --target en-standard",
      "result": "passed",
      "summary": "One UG target verified; inspection confirmed it had no CrowdAnki golden comparison."
    },
    {
      "command": "focused mutated actual-UG import/export probes",
      "result": "passed",
      "summary": "Reproduced Russian note.unnamed collision, accepted duplicate GUIDs, and silent template ordinal normalization."
    },
    {
      "command": "jj status && jj diff --stat",
      "result": "passed",
      "summary": "Baseline was clean with 0 changed files before writing the requested report."
    }
  ],
  "validationOutput": [
    "brain-brew-formats CrowdAnki tests: 42 passed, 0 failed",
    "brainbrew ug_style_fixture: 1 passed, 0 failed",
    "UG en-standard verify: verified 1 target",
    "ru-standard import: unsupported collision at note.unnamed",
    "duplicate-GUID import: exit 0 with duplicate crowdanki:guid",
    "template ord round trip: [99,1,2,3] accepted and rewritten to [0,1,2,3] in a changed template order"
  ],
  "residualRisks": [
    "External Python 0.3.11 equivalence generation was inspected but not rerun.",
    "Only one UG target was executed because the maintained manifests contain no independent CrowdAnki goldens.",
    "External Ultimate Geography working copy was not clean and was used read-only."
  ],
  "noStagedFiles": true,
  "notes": "plan.md and progress.md requested by the task were absent at repository root. This task changed only audit/05-crowdanki.md. Concurrent audit/02-core-correctness.md and audit/04-yaml-formats.md additions appeared before final status; git diff --cached --name-only remained empty."
}
```
