## Review

### Scope and governing decisions

Reviewed `brain-brew-core` against `CONTEXT.md`, project scope, ADR-003/004/006/007/008/013/016, the extension skill, core implementation, and core tests. The requested `plan.md` and `progress.md` paths do not exist in this checkout, so they provided no additional requirements.

### Correct

- Stable paths and collection identity are deterministic, and validation catches map/payload ID mismatches plus duplicate field/template IDs (`crates/brain-brew-core/src/validate.rs:12-90`, `:98-123`, `:126-212`).
- Scalar destructive changes check concrete expected values, field-level raw `add`/`merge` rejects non-blank values, and same-path overlay conflicts require an explicit later `override` (`crates/brain-brew-core/src/compose.rs:590-650`, `:1113-1172`, `:1594-1616`).
- Full note and note-type bodies are rejected for non-`add` changes (`crates/brain-brew-core/src/compose.rs:296-317`, `:961-981`), matching ADR-007.
- Core tests are green: 79 unit/integration tests and 0 doc tests passed.

### Ranked findings

#### 1. High — Entity-level destructive expected bases are presence-only and often never compared

**Evidence:** `has_expected_base` only checks `Option::is_some()` (`crates/brain-brew-core/src/compose.rs:1619-1633`). `CardTemplateChange` calls it (`:406-411`) and then replaces the complete template (`:489-516`) without comparing either `ExpectedBase::Value` or a fingerprint to the old template. `FieldDefinitionChange` does the same (`:838-847`, `:874-899`). Outer note/note-type replace/override expected bases are also only checked for presence (`:314-327`, `:978-990`). Media replacement does compare `Value`, but accepts `EntityPresent` as sufficient for replacement (`:1223-1255`, `:1269-1293`). This contradicts ADR-007’s requirement that replace/override carry the prior value or fingerprint and fail stale (`documentation/docs/reference/decisions/0007-require-explicit-conflict-and-destructive-change-semantics.md:13-24,41-43`).

**Reproduction/reasoning:** Change a base card template upstream, then apply a complete `CardTemplateChange { intent: Replace, expected_base: Some(ExpectedBase::Value("definitely-wrong")), template: Some(...) }`. Composition succeeds because only `Some` is tested. The same applies to field definitions. A media replace with `EntityPresent` silently discards any upstream path/hash edit.

**Smallest fix direction:** Define deterministic entity summaries/fingerprints and compare them for complete entity replace/override. Restrict `EntityPresent` to operations whose contract intentionally checks existence (at most remove), or remove complete replacement in favor of sparse property changes with concrete expected bases.

**Missing tests:** Wrong `ExpectedBase::Value` must fail for card templates and field definitions; stale media changed after an `EntityPresent` baseline must fail replacement; outer note/note-type expected bases must either be evaluated or rejected as unsupported.

#### 2. High — Structured-image fields are treated as blank, so `add`/`merge` can erase them without an expected base

**Evidence:** Structured images use an empty placeholder in `Note.fields`. Field conflict and expected-base logic reads only that raw string (`crates/brain-brew-core/src/compose.rs:1124-1153`). Therefore `Add` and `Merge` see an existing structured image as blank (`:1155-1172`) and then replace it while deleting `field_images` (`:1178-1205`). `ExpectedBase::Value("")` likewise matches every structured-image payload, regardless of referenced media IDs. ADR-007 says field fills may only fill values that are *still blank*, while changing existing content requires replace with its expected base; ADR-016 says the three field representations are exclusive semantic values (`documentation/docs/reference/decisions/0007-require-explicit-conflict-and-destructive-change-semantics.md:15`; `documentation/docs/reference/decisions/0016-use-structured-image-field-references-and-severable-media-includes.md:70-80`).

**Reproduction/reasoning:** Start with `fields[field.flag] = ""` and `field_images[field.flag] = [media.old]`. Apply a field `Merge` or `Add` with raw value `"replacement"` and no expected base. Composition succeeds and removes `media.old`; the field was semantically non-blank.

**Smallest fix direction:** Centralize a semantic field-value accessor/enum used by blank checks and expected-base checks. A field is blank only when it has no structured message/images and its raw value is empty. Extend expected bases to compare structured values or fingerprints.

**Missing tests:** `add`, `merge`, and field-fill over existing structured images must fail; replace with an empty scalar expected base must not match a structured image; equivalent cases should cover structured messages that render empty.

#### 3. High — Flat, untyped tombstones alias unrelated entity kinds and removals are recorded inconsistently

**Evidence:** Tombstones are `BTreeSet<StableId>` with no entity kind/path (`crates/brain-brew-core/src/model.rs:674-685`). Removing a note type inserts only its bare ID (`crates/brain-brew-core/src/compose.rs:251-293`), while validation treats a note with that same bare ID as tombstoned and skips all live-note invariants (`crates/brain-brew-core/src/validate.rs:203-227`). Cross-kind stable-ID collisions are not checked. Conversely, card-template, field-definition, and media removals physically delete entities without adding tombstones (`crates/brain-brew-core/src/compose.rs:472-487`, `:862-872`, `:1258-1268`), despite ADR-007’s statement that removals are represented as tombstones. `apply_note_add` can also clear any same-ID tombstone, regardless of what kind was removed (`:923-958`).

**Reproduction/reasoning:** Give a live note ID `entity.shared` and an unrelated unused note type ID `entity.shared`. Remove the note type. The result’s tombstone now marks the live note as removed too, and `validate()` skips its invariants. In a later composition, adding a note with the same ID clears the note-type removal record.

**Smallest fix direction:** Make tombstones typed/path-addressed (for example, note/note-type/media/template/field tombstone variants) and make all entity removals emit the appropriate type. If compatibility requires flat tombstones temporarily, enforce global cross-kind StableId uniqueness and reject ambiguous tombstones, but that alone does not fix missing removal records.

**Missing tests:** Same ID across note and note type must not cross-tombstone; note add must not clear another kind’s tombstone; media/template/field removals must preserve deliberate-removal history; validation must reject orphan/ambiguous tombstones.

#### 4. High — Structured-message references are resolved in one snapshot pass, producing stale output and accepting cycles

**Evidence:** Resolution clones one snapshot, renders every message against that unchanged snapshot, and only writes results afterward (`crates/brain-brew-core/src/messages.rs:133-160`). Field references read the cached scalar from `Note.fields`, not the referenced structured message (`:182-190`, `:300-307`). Validation checks only that the referenced raw field key exists (`:95-113`); it does not detect dependency cycles. Composition invokes this one-pass resolver (`crates/brain-brew-core/src/compose.rs:10-26`).

**Reproduction/reasoning:** Let field B be a structured message whose cached raw value is blank and whose component is `Text("resolved")`; let field A be a structured message referencing B. One pass resolves B to `resolved` but resolves A from the snapshot’s blank B, so the returned deck contains the wrong A value. A↔B cycles also validate and resolve to whatever stale cached strings happened to be present.

**Smallest fix direction:** Build a structured-message dependency graph, evaluate acyclic references in topological order, and emit a path-rich cycle error. Do not use mutable cached raw field text as the authority for another structured message.

**Missing tests:** Multi-hop references, order independence, direct self-reference, multi-field cycles, and cycles spanning notes.

#### 5. High — Semantic diff can report materially different canonical decks as identical

**Evidence:** Top-level diff compares name, description, variables, note types, notes, media, and tombstones, but omits `CanonicalDeck.id` and deck-level `adapter_ids` (`crates/brain-brew-core/src/compose.rs:45-67`). Note diff compares only raw `fields`, not `field_messages` or `field_images` (`:1972-2013`, especially `:2001`; `:2017-2052`). Thus changing a structured image media ID while retaining the required blank placeholder yields an empty diff; changing structured-message source while leaving cached text unchanged does too. Current semantic-diff tests cover only one raw field, add/remove notes, and tombstones (`crates/brain-brew-core/tests/semantic_diff.rs:8-61`). This conflicts with ADR-003/004’s canonical-entity and stable-identity semantics and with the documented claim that semantic regression checks cover deck structure rather than source formatting.

**Reproduction/reasoning:** Clone a deck and change only `deck.id`, deck `adapter_ids`, `field_images[field.flag][0].media_id`, or a `MessageComponent::Text` while leaving `Note.fields` unchanged. `semantic_diff().is_empty()` remains true for each case.

**Smallest fix direction:** Add stable paths for deck identity and diff every canonical field, including deck adapter IDs and structured representations. Prefer per-entity/per-property changes over opaque collection summaries so before/after values remain actionable.

**Missing tests:** Deck ID, deck adapter ID, structured image, structured message component/format/reference, and representation-change diffs; assert no false equality between different canonical values.

#### 6. Medium — `validate()` and `compose()` accept unknown structured media IDs even though the ADR requires a hard compose/render error

**Evidence:** Validation iterates image IDs and checks StableId/path shape and emptiness but never checks `self.media.contains_key(image.media_id)` (`crates/brain-brew-core/src/validate.rs:178-200`, `:271-297`). `UnknownMediaReference` exists but is emitted only during `render_variables()` (`crates/brain-brew-core/src/compose.rs:1776-1795`; `crates/brain-brew-core/src/model.rs:1389-1402`). Since compose’s final gate is `validate()`, a resolved deck with an unknown image ID is returned successfully. ADR-016 requires an unknown ID to fail closed with a hard render/compose error (`documentation/docs/reference/decisions/0016-use-structured-image-field-references-and-severable-media-includes.md:76-80,100-107`).

**Reproduction/reasoning:** Replace a valid raw field with `images: [media.missing]` using a matching scalar expected base. `compose()` succeeds; only a later render/verify discovers the invalid reference.

**Smallest fix direction:** Add the media existence check to core validation so compose’s final validation fails with the existing `UnknownMediaReference` path and kind. Keep render-time checking as defense in depth.

**Missing tests:** Base validation and overlay composition with unknown image IDs, including one missing member in a multi-image field.

#### 7. Medium — `FieldChange` admits multiple conflicting payloads and silently chooses one

**Evidence:** The public core model independently permits `value`, `message`, and `images` (`crates/brain-brew-core/src/model.rs:1077-1085`). Application chooses `message`, else `images`, else `value` (`crates/brain-brew-core/src/compose.rs:1178-1205`) without rejecting multiple payloads. This makes invalid states representable and can silently discard caller intent. The YAML conversion also constructs these options independently, so this is not only a hypothetical internal misuse.

**Reproduction/reasoning:** Construct a `FieldChange` with both `value: Some("raw")` and `message: Some(...)`; composition silently uses the message and drops `raw`. Supplying all three behaves the same.

**Smallest fix direction:** Validate payload cardinality before mutation: exactly one of value/message/images for add/merge/replace/override, and none for remove; reject empty image vectors at the change boundary.

**Missing tests:** Every conflicting pair, all three payloads, remove-with-payload, and zero-payload non-remove.

#### 8. Note — Compose errors flatten validation categories

Final validation errors are converted to `ComposeErrorKind::ValidationFailed`, retaining only path/message (`crates/brain-brew-core/src/compose.rs:27-39`). Machine consumers cannot distinguish missing note types, invalid StableIds, conflicting field representations, or unknown media without parsing English. Preserve the originating `ValidationErrorKind` as structured data or map to dedicated compose kinds. Existing human-readable paths/messages are otherwise useful.

### Review gate

- **Blocker:** The high-severity expected-base, structured-field, tombstone, message-resolution, and semantic-diff issues prevent treating the current core as fail-closed for federation/round-trip correctness.
- **Note:** No source fix was applied because this task was explicitly review-only.

```acceptance-report
{
  "criteriaSatisfied": [
    {
      "id": "criterion-1",
      "status": "satisfied",
      "evidence": "Review-only scope was preserved; the only created project artifact is audit/02-core-correctness.md. No project/source files or tests were modified."
    },
    {
      "id": "criterion-2",
      "status": "satisfied",
      "evidence": "The audit contains ranked findings with exact file:line evidence, concrete reproductions/reasoning, smallest fix directions, and missing tests, grounded in governing ADRs."
    }
  ],
  "changedFiles": [
    "audit/02-core-correctness.md"
  ],
  "testsAddedOrUpdated": [],
  "commandsRun": [
    {
      "command": "jj status && jj diff --summary",
      "result": "passed",
      "summary": "Confirmed the working copy was initially clean."
    },
    {
      "command": "devenv shell cargo test -p brain-brew-core",
      "result": "passed",
      "summary": "All 79 brain-brew-core unit/integration tests passed; 0 failed; doc tests passed."
    },
    {
      "command": "jj status && jj diff --summary (post-audit)",
      "result": "passed",
      "summary": "Post-write status was checked; only the requested audit artifact is expected, with no source changes."
    }
  ],
  "validationOutput": [
    "brain-brew-core: 6 unit tests passed",
    "canonical_deck_validation: 9 passed",
    "content_validation: 3 passed",
    "overlay_compose: 55 passed",
    "semantic_diff: 4 passed",
    "translation_coverage: 2 passed",
    "doc-tests: 0 failed"
  ],
  "residualRisks": [
    "No adversarial tests were added because the assignment prohibited source/test modification; reproductions are reasoned from exact control flow.",
    "plan.md and progress.md were absent, so no plan-specific constraints could be reviewed.",
    "Passing current tests does not cover the missing edge cases enumerated in the findings."
  ],
  "noStagedFiles": true,
  "notes": "Jujutsu has no staging area. The review gate is not clear: high-severity correctness gaps remain."
}
```
