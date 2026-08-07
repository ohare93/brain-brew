# Architecture audit

## Review

- **Correct:** The primary dependency direction is sound: `brain-brew-core` has no dependencies; `brain-brew-formats` depends on core plus serialization/hash libraries; the CLI depends on both; and browser automation remains isolated in `brain-brew-workbench-e2e`. This matches ADR-002 at the coarse crate-graph level.
- **Correct:** The core has a real stable addressing Interface (`DeckPath`) rather than relying only on anonymous strings (`crates/brain-brew-core/src/model.rs:75-620`), and core translation code already uses it (`crates/brain-brew-core/src/translation.rs:845-904`). That is a useful deepening foothold.
- **Correct:** The canonical YAML Module is large but externally deep: a small parse/format/emit Interface hides strict schema conversion and deterministic emission (`crates/brain-brew-formats/src/canonical_yaml.rs:23-182`). Its size alone is not evidence that it should become more public Modules.
- **Correct:** The repository has unusually broad behavior coverage. `devenv shell test` passed all default Rust tests, including 133 CLI integration tests and 18 full Ultimate Geography fixture tests.
- **Blocker:** None for this review-only task. Findings 1-4 are high-priority architecture risks.
- **Note:** The requested `plan.md` and `progress.md` do not exist at the repository root, so there was no plan/progress context to reconcile with the implementation.

## Ranked deepening opportunities

### 1. High — Workbench multi-file “atomic” Apply can leave canonical source partially updated

**Files / evidence**

- `crates/brain-brew-cli/src/commands/workbench.rs:1013-1035` batches the Canonical Deck file first, then translation overlays, and calls `write_files_atomically`.
- `crates/brain-brew-cli/src/commands/workbench.rs:3878-3963` prepares all temporary files but renames them sequentially without rollback.
- `crates/brain-brew-cli/src/commands/workbench.rs:4004-4017` explicitly reports already-updated and not-updated files after a rename failure.
- `crates/brain-brew-cli/tests/cli.rs:1153-1184` attests the failure mode: `deck.yaml` is updated while `da.yaml` and `nb.yaml` are not.

**Problem**

The filesystem Adapter offers a misleadingly strong Interface. Each individual rename is atomic, but the batch is not. A source edit and its stale-translation/migrated-key updates are one domain operation; splitting them can leave the Canonical Deck and Translation Overlays semantically inconsistent. The current diagnostic makes partial failure visible but does not restore the invariant.

The deletion test also fails: the transaction-looking seam cannot be removed or replaced locally because Workbench apply policy, serialization, and file ordering are embedded in one 4,985-line Implementation.

**Solution direction**

Create a dedicated workspace write transaction Module behind an honest Interface. At minimum: write and fsync all new files, preserve backups, record a recovery journal before renames, rollback completed renames on ordinary errors, and recover or clearly quarantine an interrupted transaction on next startup. Explicitly handle writes outside the package root/same filesystem; if all-or-nothing cannot be guaranteed there, reject such a batch or expose that limitation before confirmation. Rename the current helper if it remains only a durable batch writer.

**Benefits**

- Preserves locality of a domain Apply across all affected source files.
- Makes the filesystem Adapter deep: callers ask for one transaction instead of managing partial-file states.
- Removes hidden coupling between write order and source consistency.

**Test impact**

Replace the current “reports partial update” expectation with rollback/recovery tests for failures at every rename index, process-restart recovery tests, and cross-filesystem/external-include-root policy tests. Retain fsync and concurrent-Apply coverage.

**ADR conflicts**

No accepted ADR requires partial writes. This deepens ADR-011/015’s explicit Apply semantics and ADR-014’s file-mutation testing requirement.

### 2. High — ADR-015’s list/detail Seam is only partially implemented; compatibility pivots remain the active detail API

**Files / evidence**

- ADR-015 requires `card-detail`, `source-string-detail`, and `metadata-detail` endpoints and calls full pivots temporary compatibility scaffolding (`documentation/docs/reference/decisions/0015-use-lazy-single-work-item-workbench-editing.md:26-48`).
- The router has list endpoints, `note-detail`, and old full-pivot/metadata endpoints, but no other detail endpoints (`crates/brain-brew-cli/src/commands/workbench.rs:191-220`).
- Card detail requests still call `/api/workbench/card-pivot` (`crates/brain-brew-workbench-ui/src/lib.rs:4104-4127`), and source-string detail requests still call `/api/workbench/source-string-pivot` (`crates/brain-brew-workbench-ui/src/lib.rs:4172-4195`). Metadata still fetches `/api/workbench/metadata` (`crates/brain-brew-workbench-ui/src/lib.rs:4073-4081`).
- The old card response rebuilds and returns summaries for every matching card plus one selected detail (`crates/brain-brew-cli/src/commands/workbench.rs:2568-2635`); the source-string response returns every matching string plus selected occurrences (`crates/brain-brew-cli/src/commands/workbench.rs:2473-2565`); metadata returns every item (`crates/brain-brew-cli/src/commands/workbench.rs:2437-2470`).

**Problem**

The intended bounded Interface exists for navigation but not for three detail paths. This is an inappropriate Seam: UI code speaks in “detail” concepts while the Adapter invokes broad compatibility endpoints. On UG-sized decks, selecting one item still serializes unrelated summaries. By the deletion test, the compatibility endpoints are not temporary because production UI depends on them.

**Solution direction**

Implement the three specified detail endpoints with selected-item DTOs, migrate the UI and E2E tests, then delete the old pivot aliases and the `optional-metadata` aliases after a bounded compatibility window. Keep deck-wide progress on list responses; detail responses should contain only selected context.

**Benefits**

- Completes the accepted lazy architecture rather than adding another optimization layer.
- Bounds payload, projection, and DOM work per selection.
- Increases locality: list policy and selected-detail policy can evolve independently.
- Makes obsolete endpoint deletion possible.

**Test impact**

Add API tests asserting that detail payloads omit unrelated cards/strings/metadata and that unknown IDs/paths return 400. Update browser E2E network assertions to require the detail endpoints and forbid old pivots after migration. Preserve staged-edit navigation tests.

**ADR conflicts**

This resolves concrete friction with accepted ADR-015; it does not re-litigate the decision.

### 3. High — YAML and filesystem Adapter responsibilities cross the documented crate boundary in both directions

**Files / evidence**

- The declared boundary puts codecs in formats and filesystem access in `brainbrew` (`documentation/docs/reference/project-scope.md:32-40`; ADR-002).
- The CLI directly depends on `serde_yaml` (`crates/brain-brew-cli/Cargo.toml:20-37`).
- Media commands parse, mutate, and serialize YAML ASTs (`crates/brain-brew-cli/src/commands/media.rs:48-65`, `101-115`, `602-604`).
- Workbench source editing implements YAML path traversal, tag handling, and emission in the CLI (`crates/brain-brew-cli/src/commands/workbench.rs:4112-4238`, `4433-4509`).
- Translation stub application performs its own line-oriented YAML section insertion in the CLI (`crates/brain-brew-cli/src/commands/translations.rs:2682-2765`).
- Conversely, formats performs real filesystem canonicalization and reads through `std::fs` in its include resolver (`crates/brain-brew-formats/src/source_includes.rs:1-45`, with the resolver beginning at line 333).

**Problem**

Format policy is duplicated across `canonical_yaml`, `source_includes`, media commands, translations, and Workbench. The CLI is no longer thin, while the reusable formats crate is not purely a codec Adapter. A source-format change can require edits in several CLI commands; include safety and source-preservation behavior cannot be tested independently of filesystem behavior.

**Solution direction**

Define pure source-document operations in `brain-brew-formats`: parse/mutate/emit Canonical Deck and Overlay source, preserve supported include directives, update media hashes, migrate image fields, and insert translation decisions. Keep reads, writes, canonicalization, and transaction policy in the CLI. For include expansion, inject a narrow loader callback/trait into formats rather than calling `fs` directly; the CLI Adapter should own path authorization and bytes retrieval.

Do not create one Interface per command. Prefer a small, deep `CanonicalSourceDocument` / `OverlaySourceDocument` API that hides YAML AST details.

**Benefits**

- Restores ADR-002’s crate boundary and lowers change amplification.
- Gives format operations one deterministic Implementation and one test surface.
- Removes `serde_yaml` types from CLI code and improves AI navigation.
- Makes filesystem policy independently auditable.

**Test impact**

Move AST/source-preservation unit cases to formats; retain CLI integration tests for actual reads/writes. Add loader-fake tests for include cycles, allowed roots, and errors. A dependency check should assert that the CLI no longer directly depends on `serde_yaml` and formats’ pure codec layer does not call `fs`.

**ADR conflicts**

Aligned with ADR-002, ADR-005, and ADR-016. ADR-016 requires include-aware formatting/writeback behavior but does not require filesystem access to live in formats.

### 4. High — Translation mutation policy lives in CLI Implementations and leaks through public collection fields

**Files / evidence**

- `TranslationDictionary` exposes all interacting collections publicly (`crates/brain-brew-core/src/model.rs:700-721`). Its one mutation method handles only basic stale promotion (`model.rs:723-747`).
- Workbench independently implements direct/contextual/no-change precedence, stale removal, and context cleanup (`crates/brain-brew-cli/src/commands/workbench.rs:4243-4393`, `4682-4795`).
- The translations command separately implements stale shadowing/resolution and context matching (`crates/brain-brew-cli/src/commands/translations.rs:1082-1335`) and a separate source-editing path (`translations.rs:2682-2820`).
- Core already owns translation resolution semantics in `crates/brain-brew-core/src/translation.rs`, so these mutations are domain policy, not terminal/report rendering.

**Problem**

The Translation Dictionary Module is shallow as a mutation Interface: callers receive raw maps and must know cross-map invariants. Adding a decision may require deleting entries from three other collections; stale shadow behavior is implemented differently for CLI resolution and Workbench editing. This is hidden coupling and duplicated policy at the wrong crate boundary.

**Solution direction**

Add core domain commands such as `set_direct`, `set_contextual`, `set_no_change`, `record_source_change`, and `resolve_stale`, returning a typed change report. Centralize descendant-context matching and shadow cleanup there. Migrate CLI and Workbench to these commands before considering private fields; do not hide collections until codecs have a workable construction Interface.

**Benefits**

- One policy Implementation for CLI, Workbench, and future Adapters.
- Higher Leverage from core tests and fewer stale-translation regressions.
- Makes the CLI thin and source-format editing orthogonal to domain edits.

**Test impact**

Move precedence/shadow/migration matrices into core TDD tests. Keep CLI/API tests to prove request mapping and file persistence. Add equivalence tests showing that CLI resolve and Workbench Apply produce the same dictionary for the same command.

**ADR conflicts**

Aligned with ADR-008 and ADR-013. It deepens their semantics rather than changing them.

### 5. Medium — The Workbench HTTP Seam is untyped and unversioned despite being a separate-crate boundary

**Files / evidence**

- Server handlers and projection builders return ad hoc `serde_json::Value` (`crates/brain-brew-cli/src/commands/workbench.rs:233-360`, `601-779`, and many `json!` builders thereafter).
- The UI stores API state as `Value` and indexes string keys with fallback defaults (`crates/brain-brew-workbench-ui/src/lib.rs:1-35`, `947-1079`, `1287-1385`).
- Fetches deserialize every endpoint to `Value` (`crates/brain-brew-workbench-ui/src/lib.rs:3872-3882`, `3996-4004`, `4217-4229`).
- Staged edits are also anonymous JSON blobs with stringly typed modes/scopes (`crates/brain-brew-workbench-ui/src/staging.rs:33-104`).
- ADR-011 says the server/frontend API must be versioned and tested (`documentation/docs/reference/decisions/0011-use-a-local-deck-workbench-server-with-iced-wasm-ui.md:33-36`). No API version is present in routes or DTOs.

**Problem**

This is a real process/crate Seam but has no shared Interface. Renaming a field compiles on both sides and degrades to `null`, empty arrays, or defaults at runtime. The many fallbacks conceal contract drift. E2E tests provide behavioral confidence but low diagnostic locality.

**Solution direction**

Introduce a small shared workbench-contract Module/crate containing serde request/response DTOs, enums for statuses/edit modes/scopes, stable query structures, and an explicit API version. Keep domain models out of it. Server projections return DTOs; the UI client deserializes them before views render. Unknown response fields may remain forward-compatible, but missing required fields should fail visibly.

**Benefits**

- Compile-time Leverage across server and UI.
- Smaller, navigable view Interfaces and fewer string-key lookups.
- Cleaner API versioning and more precise errors.

**Test impact**

Add DTO JSON round-trip/golden contract tests and server/UI compile-time use. Keep API integration and browser E2E tests required by ADR-014.

**ADR conflicts**

Aligned with ADR-011 and ADR-014. A new crate is justified here because two independently compiled workspace crates already share the contract; avoid placing domain behavior in it.

### 6. Medium — `DeckPath` is a good Interface, but CLI code bypasses it and duplicates path/glob policy

**Files / evidence**

- `DeckPath` defines and parses the canonical grammar (`crates/brain-brew-core/src/model.rs:75-620`).
- Core compose contains roughly 250 lines of one-line path wrappers around `DeckPath` (`crates/brain-brew-core/src/compose.rs:1297-1549`).
- The translations command manually extracts note IDs, field IDs, and parents using string markers (`crates/brain-brew-cli/src/commands/translations.rs:3015-3061`).
- Workbench manually parses note-field paths (`crates/brain-brew-cli/src/commands/workbench.rs:4396-4430`) and manually derives note grouping elsewhere (`workbench.rs:1412-1423`, `3609-3612`).
- Core exports `glob_matches`, but Workbench carries another wildcard matcher (`crates/brain-brew-cli/src/commands/workbench.rs:3371-3405`).

**Problem**

The address grammar leaks as string manipulation. Any new path variant requires auditing many unrelated modules. Core’s constructor wrappers are shallow Modules: their Interface adds names but little abstraction, while callers outside core still cannot ask useful questions of a parsed path.

**Solution direction**

Deepen `DeckPath` with constructors/accessors such as `note_id()`, `field_id()`, `entity_parent()`, and `is_descendant_of()`. Use typed paths internally and stringify only at YAML/API boundaries. Reuse the exported wildcard matcher, or define one `DeckPathPattern` if wildcard semantics need stronger validation. Delete manual parsers and most one-line wrappers after migration.

**Benefits**

- One grammar and one descendant/context policy.
- Better locality for future entity/path additions.
- Safer metadata grouping and translation context matching.

**Test impact**

Extend table-driven path tests with accessor/parent/descendant cases, then replace CLI helper tests with shared core tests. Add a cross-check that metadata path matching and translation ignore-path matching use identical wildcard semantics.

**ADR conflicts**

Aligned with ADR-004, ADR-006, and ADR-008.

### 7. Medium — Workbench and translation command files are oversized orchestration Modules with low internal Locality

**Files / evidence**

- `crates/brain-brew-cli/src/commands/workbench.rs` is 4,985 lines and contains CLI parsing, Axum routing, cache invalidation, workspace selection, list/detail projections, card rendering, media serving, new-language scaffolding, staged-edit policy, YAML mutation, and durable writes.
- `crates/brain-brew-workbench-ui/src/lib.rs` is 4,243 lines and contains API access, app state, every pivot/view, direct DOM mutation, preview patching, request generations, and storage coordination.
- `crates/brain-brew-cli/src/commands/translations.rs` is 3,346 lines and combines argument parsing, interactive terminal UI, reports, stale resolution, source editing, and filesystem discovery.
- `crates/brain-brew-core/src/compose.rs` is 2,169 lines and combines composition, render lowering, and semantic diff.

**Problem**

These Modules are deep only at their outermost `run`/`App` entry points; internally they provide few named seams between unrelated policies. A change has high search cost and broad merge-conflict surface. This is significant AI-navigation friction. In contrast, `canonical_yaml.rs` is large but substantially cohesive around one Adapter, so it is not the first split target.

**Solution direction**

Split by responsibility using private/internal Modules, not new public crates by default:

- Workbench server: `api/routes`, `contracts/projections`, `workspace/cache`, `apply/domain`, `apply/source_io`, `media`, `new_language`.
- UI: `api_client`, `state`, `views/{notes,cards,source_strings,metadata,apply}`, `preview`, `staging`.
- Translations command: `args`, `terminal`, `report`, `resolve`, `source_editor`.
- Core: `compose`, `render`, `semantic_diff`, with the public `CanonicalDeck` methods unchanged.

Keep each new Interface deep and task-oriented. Do not create pass-through files merely to reduce line counts.

**Benefits**

- Better Locality and ownership boundaries.
- Smaller review surfaces and easier deletion of compatibility code.
- Faster, more reliable navigation for humans and AI tools.

**Test impact**

Primarily refactor under existing tests. Add focused unit tests only where extracted pure projections/policies currently require full CLI integration setup.

**ADR conflicts**

None. This preserves accepted crate-level architecture.

### 8. Medium — Active architecture records conflict with the implemented Workbench framework and omit two workspace crates

**Files / evidence**

- ADR-011 remains Accepted and requires Iced/WASM (`documentation/docs/reference/decisions/0011-use-a-local-deck-workbench-server-with-iced-wasm-ui.md:1-48`); ADR-014 also describes the real browser as Iced/WASM.
- The implementation and current reference docs use Leptos: `crates/brain-brew-workbench-ui/Cargo.toml:9-24`, `crates/brain-brew-workbench-ui/src/lib.rs:7-17`, and `documentation/docs/reference/workbench.md:13-29`.
- The current project scope and root README list only core, formats, and CLI (`documentation/docs/reference/project-scope.md:32-38`; `README.md:81-88`), while the workspace has five crates.

**Problem**

The accepted decision log and code disagree about a major Implementation choice. Agents following `AGENTS.md` are told to read the ADR index before design work, then receive obsolete guidance. The crate map also hides the API/UI/E2E seams. This is direct AI-navigation friction, not a request to re-litigate Iced versus Leptos.

**Solution direction**

Record the actual framework decision in a superseding ADR (including why Leptos replaced Iced and consequences), update ADR-014 terminology if appropriate, and update project scope/README with the UI and E2E crates and their dependency rules. Keep historical ADR status honest rather than silently rewriting the old decision.

**Benefits**

- Restores the architecture documents as reliable Interfaces for contributors.
- Prevents repeated framework debate and incorrect implementation plans.
- Makes the full workspace graph discoverable.

**Test impact**

Documentation-only. A lightweight doc/metadata check could verify that listed workspace crates match `cargo metadata` and that active ADR links exist.

**ADR conflicts**

This finding identifies the conflict with ADR-011/014. Resolution should supersede or explicitly amend them; it should not silently treat the implementation as self-authorizing.

### 9. Low — Filesystem path-safety policy is duplicated, including one divergent media-server variant

**Files / evidence**

- Identical media path validation exists in `crates/brain-brew-cli/src/media_assets.rs:67-76` and `crates/brain-brew-cli/src/commands/media.rs:618-627`.
- Workbench uses a different string-based rule (`crates/brain-brew-cli/src/commands/workbench.rs:3817-3826`).
- Workbench source includes use yet another `contains("..")` rule (`workbench.rs:4511-4518`) instead of the package include resolver.

**Problem**

Path authorization is security-sensitive Adapter policy. Duplicate predicates have different treatment of empty paths, `~`, absolute prefixes, and any name merely containing `..`. The deletion test shows no canonical seam: removing any copy breaks only one workflow.

**Solution direction**

Create one CLI-owned `SafeRelativePath` parser with explicit policies for media assets and writable include targets, based on path components plus canonical-root containment. Reuse the package include authorization logic where external roots are supported. Keep URL decoding/routing separate from filesystem authorization.

**Benefits**

- One auditable traversal policy.
- Consistent diagnostics and platform behavior.
- Removes duplicate helpers.

**Test impact**

Add a shared adversarial table for Unix/Windows prefixes, parent components, encoded URL input, empty/dot paths, Unicode, symlinks, and allowed external roots; run all media/include callers against it.

**ADR conflicts**

Aligned with ADR-010’s fail-closed posture and ADR-016’s include rules.

## Lower-priority observations

- `format_source_at(_path, input)` ignores its path (`crates/brain-brew-cli/src/io.rs:45-47`). By the deletion test, this is a shallow Interface: remove the parameter now, or make source-kind selection path-aware in the formats Module rather than carrying a false extension point.
- `brain-brew-core` remains dependency-pure, so there is no evidence for moving YAML, filesystem, terminal, or HTTP concerns into it.
- `brain-brew-formats/src/manifest.rs` contains manifest validation/target expansion as well as YAML conversion. ADR-009 explicitly places manifest codecs in formats, and no concrete regression was found; do not move it merely for conceptual tidiness.

## Residual risks

- `devenv shell e2e` was not run; ADR-014’s browser gate therefore remains unverified in this audit.
- The audit is static. The payload concern in finding 2 is demonstrated by response shape and computation, but no new UG payload/latency benchmark was collected.
- The missing root `plan.md` and `progress.md` may mean intended transitional work was unavailable; findings were ranked against accepted ADRs and current source only.
- The local working copy already contained other audit files before this report; they were not reviewed or modified.

```acceptance-report
{
  "criteriaSatisfied": [
    {
      "id": "criterion-1",
      "status": "satisfied",
      "evidence": "review-findings: nine ranked findings with severity and file/line evidence; residual-risks: four explicitly listed risks"
    }
  ],
  "changedFiles": [],
  "testsAddedOrUpdated": [],
  "commandsRun": [
    {
      "command": "jj status && jj diff --summary",
      "result": "passed",
      "summary": "Inspected the pre-existing working-copy state before review."
    },
    {
      "command": "cargo metadata --no-deps --format-version 1",
      "result": "passed",
      "summary": "Verified all five workspace crates and their direct dependency graph."
    },
    {
      "command": "devenv shell test",
      "result": "passed",
      "summary": "All default Rust unit and integration tests passed, including core, formats, CLI, UI host tests, and full UG fixture tests."
    },
    {
      "command": "source/ADR inventories, line counts, grep, and targeted read-only source inspection",
      "result": "passed",
      "summary": "Inspected all ADRs, all crate manifests, the complete source inventory, module declarations/public surfaces, and evidence ranges cited above."
    }
  ],
  "validationOutput": [
    "brain-brew-core has no direct dependencies",
    "brain-brew-formats depends on brain-brew-core plus serde/serde_json/serde_yaml/sha2",
    "brainbrew depends on core, formats, CLI/server/filesystem libraries, and directly on serde_yaml",
    "devenv shell test completed with zero failures"
  ],
  "residualRisks": [
    "Browser E2E gate was not run.",
    "No performance benchmark was collected for full-pivot detail responses.",
    "plan.md and progress.md were absent.",
    "Pre-existing working-copy audit files were not reviewed."
  ],
  "noStagedFiles": true,
  "notes": "Review-only: no product source or tests were edited. The requested audit report was written to audit/11-architecture.md."
}
```
