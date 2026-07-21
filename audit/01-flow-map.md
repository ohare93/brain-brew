# Code Context

> **Resolution annotation (current fixture):** This is a historical audit of the
> earlier partial fixture. The fixture-sync gaps and optional parity findings
> below are now resolved by the exact pinned UG `1017a399...` source/media
> snapshot, UG attribution plus the pinned HG `09ce7c3...` supplement with exact
> 607-file coverage, and the mandatory 100-output oracle documented in
> `documentation/docs/reference/ultimate-geography-fixture.md`.
> No fixture-only language/profile delta remains. Live-consumer acceptance stays
> a separate release gate.

Scope: read-only review of the tracked Brain Brew workspace and `/home/jmo/Development/external/ultimate-geography`. No project or consumer source was changed.

## Files Retrieved

1. `Cargo.toml` (lines 1-27), `crates/*/Cargo.toml` (whole files) - workspace and dependencies.
2. `crates/brain-brew-cli/src/main.rs` (1-116), `args.rs` (1-363), `io.rs` (1-643) - dispatch, arguments, and the shared filesystem/package planner.
3. `crates/brain-brew-core/src/model.rs` (1-1402), `compose.rs` (1-2169), `translation.rs` (1-1870), `validate.rs` (1-459), `messages.rs` (1-342) - pure domain and behavior.
4. `crates/brain-brew-formats/src/canonical_yaml.rs` (1-2717), `manifest.rs` (1-851), `source_includes.rs` (1-743), `crowdanki.rs` (1-1309), `media.rs` (1-354), `lockfile.rs` (1-243) - codecs/adapters.
5. `crates/brain-brew-cli/src/commands/{import,compose,export,verify,media,lock,translations,workbench}.rs` (whole files) - requested command flows and workbench server.
6. `crates/brain-brew-workbench-ui/src/lib.rs` (1-4243), `staging.rs` (1-163), `main.rs` (1-8) - Leptos/WASM UI and local drafts.
7. `crates/*/tests/*.rs`, `crates/brain-brew-core/src/tests.rs` - unit, integration, fixture, API, and browser tests.
8. `devenv.nix` (1-114), `scripts/check_workbench_ui_embed.sh` (1-28), `scripts/sync-ug-fixture.sh` (1-85) - build/test/fixture pipelines.
9. `/home/jmo/Development/external/ultimate-geography/{brainbrew.yaml,brainbrew-hardcore.yaml,deck.yaml,deck-hardcore.yaml}` (1-605, 1-373, 1-7154, 1-79) - first consumer's graph and bases.
10. `/home/jmo/Development/external/ultimate-geography/overlays/languages/es.yaml` (1-1005), `overlays/extensions/hardcore.yaml` (1-1047), `overlays/extensions/hardcore/field-fills.yaml` (1-84), `overlays/variants/extended.yaml` (1-28) - representative translation/extension/variant forms.
11. `/home/jmo/Development/external/ultimate-geography/CONTRIBUTING.md` (1-527), `.github/workflows/integrity-check.yml` (1-46), `docs/pr736-equivalence-evidence.md` (1-87) - documented/CI use.
12. `fixtures/ultimate-geography/**`, `crates/brain-brew-formats/tests/ultimate_geography_fixture.rs` (1-1929) - migrated consumer-shaped fixture.

## Key Code

### Topology and interfaces

```text
brain-brew-core (pure, no dependencies)
        ^
brain-brew-formats (strict YAML/JSON, CrowdAnki, manifests, locks, media)
        ^
brainbrew CLI (filesystem, terminal, HTTP/fetching, workbench server)

brain-brew-workbench-ui (independent wasm-only Leptos/gloo/web-sys client)
brain-brew-workbench-e2e (independent thirtyfour WebDriver harness)
```

Workspace members are `Cargo.toml:1-8`; intended boundaries are documented in `crates/brain-brew-core/src/lib.rs:1-16` and `crates/brain-brew-formats/src/lib.rs:1-18`.

Critical interfaces:

- `StableId` and typed dotted `DeckPath`: `crates/brain-brew-core/src/model.rs:9-374`.
- `CanonicalDeck`, `Overlay`, `TranslationDictionary`, change intents/expected bases, notes/types/templates/messages/media: `model.rs:676-1186`.
- `CanonicalDeck::{compose,semantic_diff,render_variables}`: `compose.rs:10-73`.
- `CanonicalDeck::{translation_coverage,translation_context}`: `translation.rs:10-26`.
- `FederatedDeckManifest`, `BuildTarget`, language/profile/export metadata: `crates/brain-brew-formats/src/manifest.rs:12-19,161-270`.
- `ManifestTargetPlan`: composed CLI plan containing loaded base plus ordered `(PlannedOverlay, Overlay)` entries, `crates/brain-brew-cli/src/io.rs:143-180`.
- `CrowdAnkiExport` and strict import/export: `crates/brain-brew-formats/src/crowdanki.rs:13-88`.
- `FederationLock`/`LockedPackage`/`LockedSource`: `crates/brain-brew-formats/src/lockfile.rs:12-42`.

### Composition semantics

`CanonicalDeck::compose` clones the base, resolves structured messages, applies overlays in order, resolves messages after each overlay, accumulates errors, then validates (`core/src/compose.rs:10-43`). Application order is translation dictionary -> deck metadata -> note types/templates/fields -> notes -> added-field blank backfill -> media (`compose.rs:76-151`). Changed paths are attributed to overlays; a second non-override writer conflicts, and destructive intents require expected bases (`compose.rs:1594-1640`).

Translation precedence is target adaptation -> ignored/empty -> variable-specific -> most-specific contextual -> direct -> no-change -> stale -> missing (`core/src/translation.rs:240-310`). Coverage extracts deck, note-type, note/message, tag, variable, and adapter text (`translation.rs:28-157`). Structured source messages retain translatable text and field references, while composition renders final scalar fields (`core/src/messages.rs:5-196`).

### Source contract

Canonical YAML parse validates immediately; overlay parsing is strict; emitters are deterministic (`formats/src/canonical_yaml.rs:23-182`). Reads resolve `!include` before parsing (`cli/src/io.rs:47-91`). Includes are restricted to scalar content positions plus top-level `media: !include`, and root escape needs an allowed `include_roots` path (`formats/src/source_includes.rs:13-78,321-450`). Formatting replaces directives with sentinels, formats the domain value, then restores directives (`source_includes.rs:56-181`).

## Architecture

### CLI entry and package/target spine

`main` manually dispatches `fmt`, `validate`, `compose`, `export`, `import`, `lock`, `media`, `targets`, `translations`, `verify`, `workbench`, `explain`, and `diff`; it aliases `translate/translation` and limits JSON error envelopes to machine-facing commands (`cli/src/main.rs:15-90`). Parsing is handwritten in `args.rs:1-363`.

Most target commands share this flow:

```text
manifest
 -> ManifestRegistry::load
    (root + --include + recursive --package-root + sibling brainbrew.lock manifests)
 -> optional package dependency/version validation
 -> recursive package-qualified target `extends`
 -> dependency-expand/de-duplicate/cycle-check overlay refs
 -> read base/overlays with each package's include roots
 -> ManifestTargetPlan
 -> CanonicalDeck::compose
```

See `cli/src/io.rs:151-180,196-435`. A second local-only expansion engine lives in `formats/src/manifest.rs:22-151`.

### Compose

- Ad hoc: read deck/overlays -> warn on stale translations -> core compose -> canonical YAML (`commands/compose.rs:52-87`).
- Manifest: parse target -> package-aware plan -> warning -> compose -> canonical YAML (`compose.rs:18-50`).

Composition validates but does not variable-render, so canonical output may retain `${...}`.

### Import

```text
folder/deck.json
 -> strict serde (`deny_unknown_fields`)
 -> reject unsupported children/scheduling/config/model/field/template/note data
 -> derive slugged suggested StableIds
 -> preserve CrowdAnki UUID/GUID adapter IDs
 -> fail on ID/UUID collisions
 -> media_files become references with empty sha256
 -> strict whole-field <img src="..." /> sequences become structured images
 -> core validate -> canonical YAML
```

CLI requires `--accept-suggested-ids` (`commands/import.rs:8-37`). Conversion is `formats/src/crowdanki.rs:788-925,936-1218`. It imports no media bytes and exposes no review/override mapping; collision text references a not-yet-public override path (`crowdanki.rs:1212-1214`). Empty imported hashes (`crowdanki.rs:885-910`) require later `media hash` before asset verification.

### Export

Both forms compose then call `write_crowdanki_export` (`commands/export.rs:17-94`). The adapter validates, variable-renders/lower messages and images, validates again, preserves configured adapter identities, supplies fixed defaults, omits tombstoned notes, and deterministically emits `deck.json` (`formats/src/crowdanki.rs:13-82,571-764`). With `--media-root`, CLI validates references/assets/hashes and copies each declared asset to `<out>/media` (`commands/export.rs:86-106`; `media_assets.rs:23-75`). Without it, reference validation is skipped; `verify` is the stronger release gate.

### Verify

`verify --target/--all-targets` performs (`commands/verify.rs:18-90`):

1. byte-canonical root manifest;
2. byte-canonical root base and its top-level included media map;
3. package-aware planning and byte-canonical expanded overlays;
4. incremental translation coverage (strict rejects fallback/stale; lenient warns stale);
5. composition + core validation;
6. media references only, or existence/hash with `--media-root`;
7. variable rendering and HTML/CSS validation unless skipped;
8. optional CrowdAnki golden comparison with explicit path-glob allowlist (`verify.rs:94-350`).

CrowdAnki parity compares known arrays/media semantically and reports exact JSON paths (`formats/src/crowdanki.rs:92-568`).

### Media

Formats scans raw `[sound:]`, `src=`, `href=`, CSS `url()`, and structured image IDs. Used/undeclared and unknown IDs error; unused declarations warn; hashes validate against supplied bytes (`formats/src/media.rs:14-203`).

- `media hash`: gathers root base plus expanded overlay source files, follows a hoisted media map, hashes under `--media-root`, and canonically rewrites changed sources (`commands/media.rs:34-87,262-363`).
- `media images-to-refs`: builds unique path->media-ID lookup and converts only strict whole-field image sequences; mixed/ambiguous/unmatched fields are counted and retained (`media.rs:89-136,365-628`).
- Export copies declared assets; verify catches missing/empty/mismatched assets (`cli/src/media_assets.rs:8-75`).

### Lock/package federation

`lock update` accepts one path, GitHub git URL, or tarball; snapshots a filtered tree, computes a Nix Archive Representation SHA-256, caches by hash, validates manifest package id/version, and emits deterministic lock YAML (`commands/lock.rs:25-91,321-575,714-770,924-970`). `lock verify` re-hashes (including live path drift) and checks locked metadata (`lock.rs:93-120,403-500,744-770`). Git mode is GitHub-HTTPS/API/codeload-specific; other remotes need tarballs (`lock.rs:509-548,675-735`).

Normal planning silently discovers sibling `brainbrew.lock`, resolves cached/fetched package manifests, checks hash/metadata, and registers them (`io.rs:208-240`; `lock.rs:457-500`). Dependencies use exact optional `id@version`, not semver (`package_resolver.rs:9-61,89-93`).

### Translations and auxiliary commands

`translations` plans targets, composes incrementally to each translation overlay, asks core for coverage/context, filters/reports it, and optionally rewrites stubs or resolves stale records (`commands/translations.rs:23-121,975-1168,1251-1290,2682-2920`). Report/workbench composition clones and sanitizes stale entries and disables complete coverage (`commands/translation_overlay.rs:3-65`); release composition does not.

`fmt` tries deck, overlay, manifest, lock, and media-map codecs (`io.rs:14-42`); `validate` composes then core-validates (`commands/validate.rs:14-184`); `diff` uses semantic stable paths and can draft a documented subset of overlay changes (`commands/diff.rs:10-35`; `overlay_draft.rs:11-42`); `targets` and `explain` expose package/stack/diff/conflict plans (`commands/targets.rs:9-78`; `commands/explain.rs:7-119`).

### Workbench server -> UI -> source

`workbench serve` reads one manifest, binds loopback, optionally opens a browser, and serves embedded Trunk assets or `--dev-assets` (`commands/workbench.rs:41-231,405-449`). Routes at `workbench.rs:191-221` serve workspace, paginated list/detail/pivot data, comparison, metadata, new-language, apply preview/apply, and declared media.

Selection is language catalog -> target/overlay labels -> package plan -> incremental composition to the selected translation overlay -> source deck + coverage -> lenient target composition -> JSON context (`workbench.rs:1162-1360`). Selected contexts are cached by selection (`workbench.rs:491-599`).

The real frontend is Leptos CSR. `App` owns selection/list/detail/error generations and staging (`workbench-ui/src/lib.rs:24-298`); Notes load first, Cards/Source Strings/Metadata lazily (`lib.rs:2510-3030`); generation tokens discard late responses (`lib.rs:3160-3241`). Draft JSON lives in localStorage keyed by language+target+overlay+kind+path+source (`staging.rs:33-146`).

Apply Preview/Confirm gather every active storage prefix and post identical edit requests to preview/write endpoints; successful write clears drafts (`ui/src/lib.rs:1629-1694,3873-3969`). Server serializes applies, changes source first, recalculates context, groups translation edits by overlay, validates, previews affected files, then atomically temp-writes/renames and invalidates cache (`workbench.rs:878-1064,4050-4288,4682-4937`). Source edits create stale records or migrate keys (`workbench.rs:4099-4375`); translation edits mutate direct/contextual/no-change maps (`workbench.rs:4682-4775`). New-language creation atomically writes manifest and overlays (`workbench.rs:815-877,1837-2117`).

### Tests

The executed non-browser suite passed **404 tests**:

| Layer | Entry points | Coverage |
|---|---|---|
| Core | `core/src/tests.rs:1-408`; `core/tests/*.rs` | paths/invariants, full intent/conflict matrix, messages, translations, render, semantic diff |
| Formats | `formats/tests/*.rs` | strict/adversarial deterministic YAML, overlays/manifests/locks/media/includes, CrowdAnki fail-closed cases |
| Full UG fixture | `formats/tests/ultimate_geography_fixture.rs:16-1929` | 74+26 targets, canonical source, structured messages/media, exact output propagation, optional release oracle |
| CLI/API | `cli/tests/cli.rs:13-7038` and companion files | subprocess/files/server, contracts, all commands, locks, workbench API/atomic writes |
| UI native | `workbench-ui/tests/workspace_summary.rs:1-17` | JSON summary only; real UI is wasm-gated |
| Browser | `workbench-e2e/tests/workbench_smoke.rs:1-3061` | 26 WebDriver workflows over real server/UI/localStorage/files |

`devenv.nix:16-114` defines test/check/clippy, Trunk build/embed, browser E2E, and CI. Embedded release assets are regenerated and byte-diffed by `scripts/check_workbench_ui_embed.sh:1-28`.

### Ultimate Geography consumer

The live checkout has:

- `brainbrew.yaml`: `anki-geo.ultimate-geography`, English `deck.yaml`, 92 overlay files, **74** standard/extended/experimental/standalone-Hardcore targets (`brainbrew.yaml:1-605`).
- `brainbrew-hardcore.yaml`: `anki-geo.hardcore-geography`, minimal shell, shared Hardcore overlays, **26** companion targets (`brainbrew-hardcore.yaml:1-373`).

`deck.yaml:1-79` preserves CrowdAnki identity, variables, external descriptions/templates/CSS; 319 notes and 546 inline empty-hash media declarations follow (`deck.yaml:80-7154`). Spanish demonstrates dictionaries, adapter-ID maps, and guarded metadata replacement (`overlays/languages/es.yaml:1-1005`). Hardcore shares extension notes and blank-only field fills (`hardcore.yaml:1-1047`; `field-fills.yaml:1-84`). Extended adds shared templates (`variants/extended.yaml:1-28`).

Documented use is targets -> verify both manifests/media -> compose inspection -> export (`CONTRIBUTING.md:47-102`). CI also runs a Python source checker then verifies/exports every target (`.github/workflows/integrity-check.yml:25-46`). Historical evidence records representative old-Python/new-Rust GUID/card/note parity (`docs/pr736-equivalence-evidence.md:1-87`).

The Brain Brew fixture is a migrated derivative, not a byte snapshot: sync appends temporary language/profile catalogs (`scripts/sync-ug-fixture.sh:62-66`; fixture `brainbrew.yaml:606-807`) and tests expect hoisted, hashed media (`ultimate_geography_fixture.rs:258-328`). Current-binary diagnostic verify of live `en-standard` stopped because `deck.yaml` is noncanonical; its 546 empty hashes would then fail media-root validation. Fixture success therefore proves the intended migrated shape, not current live-checkout acceptance.

## Surprising Coupling, Unmapped, and Dead/Transitional Areas

1. **Workbench monolith/YAML coupling:** almost 5k server lines include DTOs, rendering, raw YAML surgery, translation policy, and atomic writes. Scalar-include writes bypass the reusable resolver and use `root.join` plus literal `..` rejection (`workbench.rs:4420-4519`), so readable safe external include roots are not equivalently editable.
2. **Incomplete cache watch:** signatures cover manifest/base/catalogued overlays only (`workbench.rs:573-598`), not included HTML/CSS/media maps or locked package sources. Materialized previews can remain stale.
3. **Consumer layout in product code:** media lookup walks ancestors for `external/<manifest-dir>/media` (`workbench.rs:1094-1124`), explicitly matching this developer checkout; API coverage names it (`cli/tests/cli.rs:737-764`).
4. **Workbench package options narrower than CLI:** planning is always passed empty explicit include/package-root lists (`workbench.rs:1230-1238`), and serve exposes no such flags (`workbench.rs:71-154`); implicit locks still work.
5. **Transitional API:** ADR-015 specifies `card/source-string/metadata-detail` (`decisions/0015...md:26-47`), but only `note-detail` exists; UI still uses pivots/metadata (`workbench.rs:191-219`; UI `lib.rs:4038-4196`).
6. **Stale frontend docs:** accepted ADR-011/014 and UG guide say Iced/WASM (`0011...md:1-49`; `0014...md:7-17`; UG `CONTRIBUTING.md:185-199`), while code/Cargo use Leptos (`workbench-ui/Cargo.toml:17-25`; `src/lib.rs:108-121`). Current workbench reference is correct (`documentation/docs/reference/workbench.md:10-31`).
7. **Root-biased canonical verification:** only root manifest/base/media include and expanded overlays are byte-checked (`verify.rs:23-60`). Package manifests/bases reached through `extends`, and overlay media maps, are parsed but not all independently byte-checked.
8. **Export weaker without media root:** it skips reference validation unless `--media-root` exists (`export.rs:86-100`), allowing HTML references absent from emitted `media_files`; release callers need verify first.
9. **Import UX incomplete:** generated IDs, empty hashes, no media-byte copy, and no accepted-ID override interface (`import.rs:8-37`; `crowdanki.rs:808-925,1212-1214`) make import a bootstrap, not a complete guided round trip.
10. **Duplicated manifest expansion:** reusable local expansion (`manifest.rs:22-151`) and CLI package expansion (`io.rs:196-435`) can drift.
11. **Fixture sync gap:** sync appends consumer-missing language metadata but does not copy/create current fixture `media.yaml` (`sync-ug-fixture.sh:49-66`), while tests require it (`ultimate_geography_fixture.rs:258-328`). Running it against this checkout likely needs an unlisted migration step.
12. **UG redundant media copies:** CLI already copies exact declarations (`export.rs:99-101`), but UG docs/CI copy every media file again (`CONTRIBUTING.md:63-90`; workflow `35-45`).
13. **UG mid-adoption:** live manifests lack the workbench-required language catalog; the fixture supplies a temporary delta (`sync-ug-fixture.sh:62-66`; selection failure `workbench.rs:1287-1318`). The live checkout is not yet proof all current flows pass.
14. **Optional parity can skip:** release oracle absence prints a skip and passes (`ultimate_geography_fixture.rs:536-548,1922-1929`).
15. **Native UI tests miss real UI:** most UI code is wasm-gated; normal tests exercise only summary extraction. Trunk/embed checks and browser E2E are essential.

## Start Here

Open `crates/brain-brew-cli/src/io.rs:151-435`: it turns manifests, package roots, and locks into the ordered plan consumed by compose/export/verify/translations/workbench. Then read `crates/brain-brew-core/src/compose.rs:10-151,1594-1640`, followed by the relevant command adapter.

## Acceptance Evidence

```acceptance-report
{
  "criteriaSatisfied": [
    {
      "id": "criterion-1",
      "status": "satisfied",
      "evidence": "Created only the assigned audit/01-flow-map.md artifact and reviewed CLI, core/formats, workbench server/UI, requested flows, tests, and Ultimate Geography without changing source."
    },
    {
      "id": "criterion-2",
      "status": "satisfied",
      "evidence": "Report provides exact file/line references, end-to-end data flow, test evidence, downstream observations, coupling findings, and residual risks for independent review."
    }
  ],
  "changedFiles": ["audit/01-flow-map.md"],
  "testsAddedOrUpdated": [],
  "commandsRun": [
    {
      "command": "cargo test --workspace --exclude brain-brew-workbench-e2e --all-targets",
      "result": "passed",
      "summary": "404 passed, 0 failed across core, formats, CLI/API, fixtures, and non-wasm UI."
    },
    {
      "command": "target/debug/brainbrew targets --manifest /home/jmo/Development/external/ultimate-geography/brainbrew.yaml | wc -l; repeat for brainbrew-hardcore.yaml",
      "result": "passed",
      "summary": "Found 74 main and 26 Hardcore targets."
    },
    {
      "command": "target/debug/brainbrew verify --manifest /home/jmo/Development/external/ultimate-geography/brainbrew.yaml --target en-standard --media-root /home/jmo/Development/external/ultimate-geography/media",
      "result": "failed (diagnostic)",
      "summary": "Live consumer deck.yaml is not in canonical format; recorded as downstream drift, not a project test failure."
    },
    {
      "command": "find/rg/read/nl/wc repository inspection; fixture/consumer diff; jj status and jj diff --summary",
      "result": "passed",
      "summary": "Mapped entry points and ranges, quantified surfaces, confirmed consumer-fixture divergence, and checked working-copy state."
    }
  ],
  "validationOutput": [
    "Non-browser workspace: 404 passed, 0 failed.",
    "Ultimate Geography targets: 74 main, 26 Hardcore.",
    "Diagnostic consumer verify: exit 1, deck.yaml is not in canonical format.",
    "This subagent changed only audit/01-flow-map.md; concurrent audit artifacts from other workers are also visible in the shared working copy."
  ],
  "residualRisks": [
    "Browser E2E was not run; it needs Chromium/chromedriver and devenv e2e.",
    "Embedded assets were inspected but not rebuilt/diff-validated.",
    "Optional UG v5.3 oracle parity may have skipped when its cache was absent.",
    "Live Ultimate Geography is mid-migration and differs from the tested fixture.",
    "Review gate remains required by the parent/reviewer."
  ],
  "noStagedFiles": true,
  "notes": "Jujutsu has no staging area. Shared status contains this artifact plus concurrent audit files owned by other workers; no project or consumer source file is modified by this task."
}
```
