## Review

### Scope and baseline

- Reviewed ADR-0011, ADR-0014, ADR-0015, the Workbench server/API, Leptos staging/fetch paths, CLI API tests, browser E2E coverage, embedded assets, and the canonical-source constraints in `CONTEXT.md`.
- The requested `/home/jmo/Development/projects/brain-brew/plan.md` and `progress.md` do not exist in this working copy, so no plan/progress assumptions could be verified.

### Correct

- The production listener is explicitly loopback-only and defaults to an ephemeral port: `TcpListener::bind(127.0.0.1:port)` at `crates/brain-brew-cli/src/commands/workbench.rs:162-175`, matching ADR-0011's local-binding requirement.
- Browser code stages edits in `localStorage`; source files are mutated only by server endpoints. Stable keys include language/target/overlay, edit kind, path, and source at `crates/brain-brew-workbench-ui/src/staging.rs:41-80,130-139`.
- The list APIs enforce default/max bounds and reject invalid pagination at `crates/brain-brew-cli/src/commands/workbench.rs:1567-1608`. Real-server tests verify compact rows, full totals, pages, and invalid parameters at `crates/brain-brew-cli/tests/cli.rs:400-562`.
- In-process write operations are serialized by `apply_mutex` (`crates/brain-brew-cli/src/commands/workbench.rs:444-450,835-869,878-884`), and the focused concurrent API test passes (`crates/brain-brew-cli/tests/cli.rs:1220-1267`).
- Selection/detail response generations exist and the browser E2E suite tests a delayed stale language response (`crates/brain-brew-workbench-ui/src/lib.rs:3202-3239`; `crates/brain-brew-workbench-e2e/tests/workbench_smoke.rs:49-70`).
- Embedded release assets are compiled into the CLI, carry hashed filenames/SRI, and the freshness script compares a clean release build byte-for-byte (`crates/brain-brew-cli/assets/workbench/index.html:1-25`; `scripts/check_workbench_ui_embed.sh:1-32`). The freshness check passed during this audit.
- ADR-0014 has substantial real-server API coverage and browser coverage, including real file mutation, staged edits surviving item unmount/refresh, lazy secondary pivots, and a real UG smoke path. This is a strong base, but it does not cover the failures below.

### Blockers and ranked findings

#### P0 — 1. A non-included source-field edit in a deck containing `!include` rewrites the whole Canonical Deck File into non-canonical YAML

The source-edit path switches the entire deck to generic `serde_yaml::Value` handling whenever the raw file contains any `!include`, even when the edited field itself is not included (`crates/brain-brew-cli/src/commands/workbench.rs:4112-4126,4177-4199`). It then serializes the whole document with `serde_yaml::to_string` (`:4230-4238`) and validates only that generic YAML can be parsed (`:4528-4547`), not that it equals Brain Brew canonical YAML.

A focused reproduction copied the real UG fixture, applied a source edit to `notes.note.abkhazia.fields.field.country`, and received HTTP 200 with `validation.ok=true`. The resulting `deck.yaml` changed list indentation and quoting throughout the file; `brainbrew verify --manifest ... --target en-standard` then failed with:

```text
/tmp/brainbrew-audit-validation/deck.yaml is not in canonical format
```

This violates the source-of-truth/canonicalized-source requirement and ADR-0015's promise to write canonical YAML. The existing include test only checks that the include marker and values survive (`crates/brain-brew-cli/tests/cli.rs:1870-1912`); it never runs `fmt --check`/`verify` or uses the real UG include shape.

#### P0 — 2. File fingerprints are informational only; stale browser drafts silently overwrite external edits

ADR-0011 requires file fingerprints/polling and careful stale-file detection before Apply (`documentation/docs/reference/decisions/0011-use-a-local-deck-workbench-server-with-iced-wasm-ui.md:35-36,47-50`). SHA-256 fingerprints are returned by `/api/workspace` (`crates/brain-brew-cli/src/commands/workbench.rs:601-618,4929-4952`), but `ApplyRequest` contains only selection plus edits and has no expected workspace generation/fingerprint (`:1636-1661`). Apply simply refreshes/read-merges current files and writes (`:878-930,1013-1035`).

Verified reproduction:

1. Loaded UG workspace/detail for German `Sukhumi: Sochumi`.
2. Externally changed the same overlay key to `Sukhumi: ExternalEdit`.
3. Posted the already-staged browser edit `Sukhumi: WorkbenchEdit`.
4. Server returned HTTP 200, `applied=true`, `validation.ok=true`; the external value was replaced by `WorkbenchEdit`.

The existing stale test only rejects a request whose **source deck text** no longer matches (`crates/brain-brew-cli/tests/cli.rs:1571-1594`). It does not protect concurrent edits to the same translation key, manifest, include, or the preview-to-confirm window. The apply mutex protects only requests in this process, not editors or other Brain Brew processes.

#### P1 — 3. Multi-file Apply is not all-or-nothing and has no rollback/cleanup recovery

`write_files_atomically` prepares temporary files, then renames targets sequentially (`crates/brain-brew-cli/src/commands/workbench.rs:3878-3963`). If rename 2 fails, file 1 is already committed. The test explicitly accepts this partial state by asserting the error reports `updated files: deck.yaml` and remaining overlay files (`crates/brain-brew-cli/tests/cli.rs:1153-1184`). There is no backup/rollback, transaction journal, retry protocol, or cleanup of prepared temporary files on temp/rename/fsync errors. A failed new-language transaction can similarly leave overlay files without the manifest update, and retry then rejects the existing files (`crates/brain-brew-cli/src/commands/workbench.rs:835-869`).

This can split a source edit from its stale-translation updates, leaving canonical sources semantically inconsistent. The helper is per-file atomic, not transaction-atomic; its name and prior task intent overstate the guarantee.

#### P1 — 4. Confirm Apply does not require or bind to a successful preview

ADR-0011 says Apply operations must show validation/preview before writing, and ADR-0015 says Preview and Confirm collect/show changes and only then write (`ADR-0011:50`; `ADR-0015:22`). The UI exposes Preview and Confirm as independent always-enabled buttons (`crates/brain-brew-workbench-ui/src/lib.rs:1629-1689`). Confirm posts the edits directly to `/apply`; the server reruns validation but accepts no preview token/hash (`crates/brain-brew-cli/src/commands/workbench.rs:338-359,878-1035`).

The browser E2E suite actually codifies direct writes without preview for a new-language edit and a mixed source/target edit (`crates/brain-brew-workbench-e2e/tests/workbench_smoke.rs:627-646,1375-1428`). Even after a preview, there is no binding between the previewed edit set/file fingerprints and the confirmed request, so the affected files or draft may change before Confirm.

#### P1 — 5. Apply collects only active or currently mounted language prefixes, not all staged edits

ADR-0015 requires Preview/Confirm to collect from the central staged map, “not from only currently visible DOM nodes” (`documentation/docs/reference/decisions/0015-use-lazy-single-work-item-workbench-editing.md:63-71`). The collector filters localStorage by a prefix list (`crates/brain-brew-workbench-ui/src/staging.rs:82-105`). That list contains the current selection plus prefixes discovered from `[data-storage-prefix]` DOM elements (`crates/brain-brew-workbench-ui/src/lib.rs:3895-3921`).

Thus an edit staged in language A remains saved after switching to language B, but its unmounted prefix is omitted from Preview/Apply. Existing E2E covers navigation unmount within one prefix and a secondary pane that remains mounted; it does not cover staging in one language/target/overlay, switching away, and applying from another selection.

#### P1 — 6. The lazy single-item migration is incomplete and makes items beyond the first 50 unreachable

The server correctly returns bounded pages, but the UI never sends `limit`/`offset` and has no pagination controls. Notes explicitly render “pagination controls will load additional pages in the next slice” (`crates/brain-brew-workbench-ui/src/lib.rs:530-599`); card/source lists only show the first-page count (`:1013-1044,1287-1345`). On UG-sized decks, notes/cards/source strings after the default 50 cannot be selected, so the navigation model is not functionally complete.

Only `note-detail` exists. ADR-0015 specifies card/source-string/metadata detail APIs (`ADR-0015:39-48`), but routes expose only list APIs plus full compatibility pivots (`crates/brain-brew-cli/src/commands/workbench.rs:191-212`). The UI loads a selected card/source through the full `card-pivot`/`source-string-pivot` endpoints and metadata through the full endpoint (`crates/brain-brew-workbench-ui/src/lib.rs:4073-4127,4151-4194`).

The comparison endpoint is especially noncompliant: it returns all note details, all source-string groups, and all card summaries in one response (`crates/brain-brew-cli/src/commands/workbench.rs:771-812`), and the UI flattens every comparison note field (`crates/brain-brew-workbench-ui/src/lib.rs:1813-1840`). This contradicts selected-item-scoped multilingual context and can recreate the UG payload/DOM problem ADR-0015 rejected.

#### P1 — 7. Loopback binding is correct, but the local file-writing server lacks defense in depth and path containment

The router has no per-session capability, Host allowlist, Origin validation, or security middleware (`crates/brain-brew-cli/src/commands/workbench.rs:191-229`). This leaves DNS-rebinding/same-origin injection concerns for write endpoints. Card/deck HTML is rendered into `inner_html` (`crates/brain-brew-workbench-ui/src/lib.rs:1085-1091` and analogous note/source previews), while the embedded document has no CSP (`crates/brain-brew-cli/assets/workbench/index.html:1-25`); hostile event-handler markup from an untrusted workspace can execute with same-origin access to mutation APIs.

Lexical traversal checks exist for media/new-language/include paths (`crates/brain-brew-cli/src/commands/workbench.rs:2105-2117,3817-3827,4511-4518`), but paths are joined and followed without canonicalizing the parent/target and proving it remains under an approved root. Symlinked directories can therefore escape the manifest/media root for reads or writes. New-language creation also has a check-to-rename race around `exists()` (`:844-868`).

Threat-model clarification may change priority, but a local-only bind alone is not sufficient for a process that rewrites maintainer files.

#### P2 — 8. Shutdown is abrupt and untested

The server awaits `axum::serve` directly with no `with_graceful_shutdown`, signal handling, draining, or cleanup (`crates/brain-brew-cli/src/commands/workbench.rs:156-188`). Both API and browser harnesses terminate it with `Child::kill` (`crates/brain-brew-cli/tests/cli.rs:5575-5579`; `crates/brain-brew-workbench-e2e/tests/workbench_smoke.rs:2483-2487`). A SIGINT/SIGTERM during sequential renames can therefore leave the partial states and temp files described above. There is no restart recovery test.

#### P2 — 9. Active ADR/API contract drift is undocumented

ADR-0011 remains Accepted and requires Iced/WASM plus a versioned API (`ADR-0011:17,28,35,48-49`). The actual separate UI crate is Leptos CSR (`crates/brain-brew-workbench-ui/Cargo.toml:1-24`; `documentation/docs/reference/workbench.md:13-29`), with no superseding ADR. The HTTP surface is unversioned, error responses are plain text tuples, and unknown `/api/...` paths fall through to the SPA index with HTTP 200 (`crates/brain-brew-cli/src/commands/workbench.rs:391-420`). This should be resolved by updating/superseding ADR-0011 and defining a versioned JSON success/error/404 contract, or by restoring the accepted architecture.

### Missing E2E/API cases required before acceptance

1. **Real UG source write:** edit a non-included note field in the real include-bearing UG fixture, then run `brainbrew verify`; assert only intended canonical files/content changed.
2. **Stale edit CAS:** stage a translation, externally change the same key/file, then Preview and Confirm; require a conflict response and zero writes. Repeat with a change between Preview and Confirm and with included scalar files.
3. **All staged scopes:** stage in language/target/overlay A, switch to B so A is unmounted, preview/confirm from B, and assert both scopes appear and write (or explicitly expose/select a workspace-wide draft set).
4. **Preview enforcement:** Confirm must be unavailable/rejected until a successful preview; changing a draft or any affected fingerprint must invalidate the preview.
5. **Failure recovery:** inject failure at every temp/fsync/rename index and SIGTERM during each phase; assert rollback, no orphan `.brainbrew-tmp-*`, and successful retry/restart. Include new-language manifest+overlay writes.
6. **Pagination usability:** navigate to and edit an item beyond row 50 for each list on the real UG fixture; assert bounded DOM and requests with advancing offsets.
7. **True detail contracts:** API and browser tests for `card-detail`, `source-string-detail`, and `metadata-detail`, plus a selected-item-only comparison payload and bounded comparison DOM.
8. **Path/security:** percent-encoded traversal, absolute/UNC paths, symlink escapes for media/include/new-language targets, hostile Host/Origin/DNS-rebinding simulation, and hostile preview markup under a restrictive CSP/sanitizer policy.
9. **Shutdown:** graceful SIGINT/SIGTERM while idle and during an Apply, with deterministic completion/abort semantics and no file corruption.
10. **API contract:** version field/path, JSON error envelopes/content types, unknown API route JSON 404, request size limits, and forward/backward contract tests.
11. **Release assets:** launch the server without `--dev-assets` in browser E2E and exercise a write flow from the embedded release bundle (the current freshness check proves byte freshness, not runtime behavior).

### Notes

- Focused tests passed, but the full browser E2E gate was not run in this review. Existing E2E breadth is useful; the primary gaps are conflict/failure/canonicality paths rather than happy-path UI wiring.
- No source files were edited. This report is the only file added by this audit agent.

```acceptance-report
{
  "criteriaSatisfied": [
    {
      "id": "criterion-1",
      "status": "satisfied",
      "evidence": "Review-only scope was preserved; only audit/08-workbench-backend.md was added, with no product-source edits."
    },
    {
      "id": "criterion-2",
      "status": "satisfied",
      "evidence": "Findings cite exact ADR/code/test locations and include two focused real-server reproductions plus focused test and asset-validation results."
    }
  ],
  "changedFiles": [
    "audit/08-workbench-backend.md"
  ],
  "testsAddedOrUpdated": [],
  "commandsRun": [
    {
      "command": "devenv shell cargo test -p brainbrew --test cli workbench_navigation_lists_are_paginated_and_compact -- --exact",
      "result": "passed",
      "summary": "1 passed; compact pagination/list-detail API baseline is green."
    },
    {
      "command": "devenv shell cargo test -p brainbrew --test cli workbench_apply_temp_write_failure_leaves_targets_unchanged -- --exact",
      "result": "passed",
      "summary": "1 passed; pre-rename temp failure leaves targets unchanged (cleanup is not asserted)."
    },
    {
      "command": "devenv shell cargo test -p brainbrew --test cli workbench_concurrent_apply_requests_are_serialized -- --exact",
      "result": "passed",
      "summary": "1 passed; in-process Apply transactions do not overlap."
    },
    {
      "command": "devenv shell cargo test -p brainbrew --test cli workbench_source_edits_can_migrate_keys_change_all_and_preserve_includes -- --exact",
      "result": "passed",
      "summary": "1 passed; existing small-fixture include assertions pass, but they do not check canonical formatting."
    },
    {
      "command": "devenv shell ./scripts/check_workbench_ui_embed.sh",
      "result": "passed",
      "summary": "Release Leptos/WASM build matched checked-in embedded assets exactly."
    },
    {
      "command": "temporary UG server: GET workspace/detail, externally replace German Sukhumi translation, POST the staged old edit to /api/workbench/apply",
      "result": "failed",
      "summary": "Audit validation exposed a defect: HTTP 200/applied=true silently replaced ExternalEdit with WorkbenchEdit."
    },
    {
      "command": "temporary UG server: POST source edit for note.abkhazia, then brainbrew verify --manifest <temp>/brainbrew.yaml --target en-standard",
      "result": "failed",
      "summary": "Apply reported validation.ok=true, but verify failed because deck.yaml was no longer canonical after generic serde_yaml serialization."
    },
    {
      "command": "jj status && jj diff --summary",
      "result": "passed",
      "summary": "Before writing this report, product working copy had no source changes from this agent; other audit report files from concurrent reviewers were present."
    }
  ],
  "validationOutput": [
    "Focused Rust tests: 4 passed, 0 failed.",
    "Embedded asset freshness: Workbench embedded release assets are fresh.",
    "Stale-write reproduction: http_status=200, applied=True, validation={'errors': [], 'ok': True}; resulting YAML contained Sukhumi: WorkbenchEdit instead of the external edit.",
    "UG canonicality reproduction: Apply validation.ok=true; subsequent verify exited 1 with '<temp>/deck.yaml is not in canonical format'."
  ],
  "residualRisks": [
    "Full devenv shell e2e was not run; browser findings were established by code/test inspection.",
    "plan.md and progress.md requested by the task were absent, so their intended constraints could not be audited.",
    "Security priority depends on the project's untrusted-workspace and DNS-rebinding threat model, which is not documented.",
    "Cross-platform rename behavior, especially replacement on Windows, was not exercised."
  ],
  "noStagedFiles": true,
  "notes": "Review gate should fail until at least the two P0 findings are fixed and covered by real-server/browser tests. Jujutsu is used, so there is no Git staging area; no commit/staging operation was performed."
}
```
