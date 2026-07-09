## Review

### Scope and baseline

- Reviewed composition/translation complexity, manifest planning and package discovery, canonical/CrowdAnki serialization, media hashing, lock hashing, output ordering, and Workbench caching/list behavior.
- The requested `plan.md` and `progress.md` do not exist in this working copy, so no plan/progress assumptions could be verified.
- The checked-in Ultimate Geography fixture is a useful source/overlay workload: 74 targets, 16 languages, 319 base notes, 546 declared media references, 92 overlay files, 119 files total, and 1.4 MiB of source (`crates/brain-brew-formats/tests/ultimate_geography_fixture.rs:145-179`; fixture inspection commands below). It does not contain the real media payload, so media-byte conclusions below are code-derived rather than measured against release assets.

### Correct

- Deterministic ordering is designed in rather than repaired after the fact. Domain collections, manifests, overlay dependency visitation, and serialization use `BTreeMap`/`BTreeSet`; target dependency expansion is deterministic (`crates/brain-brew-formats/src/manifest.rs:22-42,124-145`), and CrowdAnki exports iterate those ordered collections before pretty serialization (`crates/brain-brew-formats/src/crowdanki.rs:18-80`). No production `HashMap`/`HashSet` use was found.
- The real UG fixture's all-target composition test passed, and its byte-idempotent formatter test passed. The latter covers manifests, decks, the media map, overlays, and lockfiles (`crates/brain-brew-formats/tests/ultimate_geography_fixture.rs:182-230`).
- Lock content hashing is reproducible and bounded-memory: the filtered source snapshot is encoded as NAR and SHA-256 is updated through a 64 KiB buffer (`crates/brain-brew-cli/src/commands/lock.rs:540-581,767-783`). Cached sources are rehashed before reuse (`:584-601`).
- A direct UG `verify --all-targets --skip-content-validation` probe completed successfully in 18.910 s with 18.803 s user CPU and sampled peak RSS of only 24,308 KiB. A large source/overlay stack therefore works today without excessive peak memory when no media root or golden export is requested.

### Ranked findings

#### P1 / High — 1. Workbench selection caching grows without a bound and retained about 430 MiB across the valid UG selection matrix

**Evidence.** `WorkspaceCache.selected_contexts` is an unrestricted map keyed by language/target/overlay and is only cleared on a workspace generation change (`crates/brain-brew-cli/src/commands/workbench.rs:464-493`). Every entry retains a `SelectedTranslationContext` containing three full `CanonicalDeck`s, every planned overlay, and the full coverage report (`:1746-1754`; construction at `:1273-1284`). Cache hits also clone the whole cached value into each request (`:1186-1226`).

A local Workbench probe requested `note-list?limit=1` for all 93 valid target-language/target/overlay selections in the UG manifest. RSS rose monotonically from **20,228 KiB to 448,672 KiB**; all requests succeeded. The run took 32.368 s (mean cold selection 0.348 s, maximum 0.739 s). Intermediate RSS was 70,960 KiB after 10 selections, 142,032 KiB after 25, and 253,616 KiB after 50. This is retained growth, not a transient peak.

**Likely impact.** A maintainer comparing many languages/variants can push a local Workbench process toward hundreds of MiB on the current 319-note fixture; larger decks multiply the retained cost. This conflicts with ADR-0015's UG responsiveness goal (`documentation/docs/reference/decisions/0015-use-lazy-single-work-item-workbench-editing.md:15-24,92-95`).

**Measurement/fix test.** Add a UG API test with observable cache statistics (entry count and estimated retained bytes), traverse the selection matrix, and require a bounded LRU/single-active-selection policy. Store shared immutable decks/overlays behind `Arc` and return shared contexts instead of deep clones. Track RSS or allocator bytes in a non-flaky benchmark, not as a normal unit-test wall-clock assertion.

#### P1 / High — 2. The first Workbench media request composes every manifest target synchronously

**Evidence.** On a cache miss, `media_path_declared` calls `collect_declared_media_paths`; that loops through every target, creates a fresh manifest target plan, and composes every overlay state before serving the requested asset (`crates/brain-brew-cli/src/commands/workbench.rs:1127-1183`). On UG, a request for one declared but absent image (`/api/media/ug-flag-abkhazia.svg`) took **14.630 s** and returned the 393-byte placeholder. The identical second request took **0.002 s** after the global declared-path cache was populated.

**Likely impact.** The first card/note preview that requests media can appear hung for roughly the cost of all-target verification, even though it needs one path. More targets and overlays worsen first-image latency. This directly conflicts with ADR-0015's fast-visible-feedback goal.

**Measurement/fix test.** Add a real UG server test/benchmark for first and warm media fetches, with an internal counter proving one selected-target parse at most. Build declared paths cheaply from the base plus overlay media deltas, precompute in a background startup task, or lazily validate against only the active target; do not compose all targets on the request critical path.

#### P1 / High — 3. `--all-targets --media-root` rereads and retains every media asset once per target

**Evidence.** Verification invokes `validate_media_assets` inside the per-target loop (`crates/brain-brew-cli/src/commands/verify.rs:51-79`). That function reads every declared file completely into `BTreeMap<String, Vec<u8>>` before hashing any of them (`crates/brain-brew-cli/src/media_assets.rs:23-45`); `validate_hashes` then makes a second pass over those resident buffers (`crates/brain-brew-formats/src/media.rs:89-127`). For UG this means up to 74 target passes over 546 declarations—**40,404 file reads/hash checks** if sets are identical—rather than hashing each distinct asset once. Peak asset memory is the sum of all media bytes for one target, not a streaming buffer.

**Likely impact.** This is negligible for the source-only fixture but potentially dominant for real image/audio decks: repeated disk I/O and SHA-256 work scale as `targets × total media bytes`, while peak RSS scales as total media bytes. Large videos/audio can cause OOM even though SHA-256 itself is streamable.

**Measurement/fix test.** Use a synthetic 74-target workspace sharing, for example, 500 × 1 MiB assets and count opens/bytes read with an injected reader or syscall tracer. Require one streaming hash per unique `(canonical path, expected digest)` per verify invocation and bounded peak buffer size (for example 64 KiB), while still reporting target-specific missing/hash errors deterministically.

#### P1 / High — 4. Workbench freshness excludes all `!include` targets, so cached results can remain stale indefinitely

**Evidence.** Cache invalidation signatures include only the manifest, top-level base file, and top-level overlay files (`crates/brain-brew-cli/src/commands/workbench.rs:517-559,573-598`). Workspace SHA-256 fingerprints use the same incomplete file set (`:4929-4952`). Yet deck/overlay reads materialize scalar and structural media includes before parsing (`crates/brain-brew-cli/src/io.rs:68-71,88-91`; include resolution at `crates/brain-brew-formats/src/source_includes.rs:15-36,367-390`). The UG fixture deliberately puts descriptions, templates, styling, translations, and its 546-entry media map in includes.

**Likely impact.** Editing an included template/description/media map externally does not advance the generation or clear `selected_contexts`; subsequent Workbench requests can show an old composed deck while the CLI sees new source. This is primarily a cache-correctness defect, with determinism/reproducibility implications for preview/apply workflows.

**Measurement/fix test.** Start Workbench on the real UG fixture, warm a context, edit each include kind (scalar nested include and top-level `media` include), and require the next response/fingerprint to change without touching the parent YAML. Have include resolution return its dependency set and signature/hash that complete transitive set.

#### P2 / Medium — 5. All-target verification reparses and recomposes shared source serially for every target

**Evidence.** The verify loop creates a fresh `ManifestRegistry` per target and re-verifies each overlay file (`crates/brain-brew-cli/src/commands/verify.rs:51-70`; `crates/brain-brew-cli/src/io.rs:173-180,196-233`). Planning rereads/parses the base and all expanded overlays (`io.rs:298-325`). Translation-policy checking then composes one overlay at a time (`verify.rs:136-185`), after which `plan.compose()` starts over and composes the complete stack. `CanonicalDeck::compose` clones the deck and validates after every call (`crates/brain-brew-core/src/compose.rs:10-40`), and verify immediately validates the final deck again.

Measured on the UG fixture, one complex target took **0.500 s**, while all 74 took **18.910 s** (single process, debug binary, content validation skipped). CPU nearly equaled elapsed time, confirming serial CPU-bound work. Peak RSS remained good at about 24 MiB, so this finding is throughput rather than memory.

**Likely impact.** Current CI cost is noticeable but tolerable; target counts, package roots, and overlay stacks will increase it roughly linearly while repeatedly paying YAML parsing, include resolution, translation coverage, clones, and validation for common prefixes.

**Measurement/fix test.** Add Criterion or a dedicated perf harness that records parse count, composed-overlay count, cloned deck bytes, and elapsed time for 1/10/74 UG targets. Load the registry/base/overlay catalog once, memoize deterministic composition prefixes, and only then consider bounded parallel target evaluation with results emitted in manifest order.

#### P2 / Medium — 6. Workbench pagination bounds response rows but not server computation or allocation

**Evidence.** Note lists build progress, metadata, and all navigation rows before `paginate` (`crates/brain-brew-cli/src/commands/workbench.rs:2119-2135`). Card lists produce every card, filter all cards, and create all summaries before pagination (`:2165-2206`). Source-string lists build/group the full row set twice, clone source keys, then paginate summaries (`:2244-2300`).

On a warmed German hardcore-extended UG context, median request times were effectively independent of page size: note list 66.6 ms (`limit=1`) vs 71.0 ms (`limit=50`), card list 59.8 vs 58.8 ms, and source-string list 51.3 vs 54.1 ms. Response bytes were bounded, but CPU/allocation work was not.

**Likely impact.** The browser receives compact pages, but interaction latency still scales with total notes × templates/source occurrences, undermining the intended benefit on decks much larger than UG or with many templates.

**Measurement/fix test.** Benchmark warm endpoint CPU/allocation for 319/3,190/31,900 notes at `limit=1` and `50`. Cache compact navigation/progress indexes per selection, paginate before expensive detail rendering, and assert response construction work is near `O(total-for-required-totals + page size)` rather than rebuilding full card/detail objects.

#### P2 / Medium — 7. Package discovery is an unpruned recursive filesystem walk, and all-target planning can repeat it per target

**Evidence.** `discover_package_manifests` recursively descends every directory under every supplied package root (`crates/brain-brew-cli/src/package_resolver.rs:7-13,64-86`). It does not skip `.git`, `.jj`, `target`, cache/output directories, or stop beneath a discovered package; `Path::is_dir` follows directory symlinks and there is no visited-directory set. By contrast, lock snapshot copying explicitly skips VCS/build output (`crates/brain-brew-cli/src/commands/lock.rs:796-831`). Registry construction invokes discovery, and finding 5 shows registry construction repeats per target.

**Likely impact.** A broad package root can traverse millions of irrelevant files, and a symlink cycle can recurse until an OS/error limit. This was not benchmarked against the 24 GiB repository root to avoid an intentionally pathological audit run; magnitude is therefore unmeasured, but the traversal behavior is confirmed.

**Measurement/fix test.** Build a fixture containing a large `target/`, nested VCS dirs, inaccessible dirs, and `loop -> ..`; assert bounded visits, no cycle, deterministic manifest order, and one discovery pass per command. Support explicit manifest indexes or prune known build/VCS trees and canonicalize/track visited directories.

#### P3 / Low now, scale risk — 8. Translation lookup can become quadratic with path-scoped overlay size

**Evidence.** Every extracted non-empty source occurrence scans all contextual path maps before direct/no-change fallback (`crates/brain-brew-core/src/translation.rs:240-309`), scans all stale records on fallback (`:1804-1821`), and scans every ignored glob (`:1823-1827`). Coverage walks all deck fields (`:48-135`), and applying a translation additionally clones the whole source deck (`:1108-1121`). Thus a per-note contextual overlay can approach `O(extracted occurrences × contextual paths)`; stacked translation coverage plus composition repeats the lookup.

**Likely impact.** No isolated UG regression was demonstrated—the measured cold Workbench selections include parsing/composition and remained below 0.74 s—so the practical severity at current UG sizes is low. A generated overlay with thousands of path-specific contexts/stale records is the unmeasured risk.

**Measurement/fix test.** Add synthetic 1k/10k/100k occurrence benchmarks with proportional contextual/stale/ignore entries and verify slope. Index exact paths and path prefixes once per dictionary, and index stale records by `new_source`; preserve longest-context semantics and deterministic reporting.

### Notes / residual risks

- CrowdAnki export holds a rendered deck clone, typed export tree, and final JSON string simultaneously (`crates/brain-brew-formats/src/crowdanki.rs:18-80`). Golden verification additionally holds expected/actual strings and both parsed `serde_json::Value` trees (`crates/brain-brew-cli/src/commands/verify.rs:321-340`). This is a plausible multi-copy memory issue for very large note text, but it was not observed at UG size and is not ranked above the measured Workbench/media issues.
- No dedicated benchmark suite or large-media fixture was found. Existing UG tests are strong semantic/determinism regressions but have no time, allocation, parse-count, I/O-count, or cache-bound assertions.
- The production paths are intentionally sequential. This supports deterministic diagnostics but leaves safe target/media parallelism unused. Optimize shared parsing/composition and bounded memory first; only parallelize after preserving ordered output and avoiding multiplied peak RSS.
- The direct CLI probes used the current `target/debug/brainbrew`; its timestamp postdates all reviewed performance-critical source files. Timings are comparative local observations, not release-mode service-level objectives.

```acceptance-report
{
  "criteriaSatisfied": [
    {
      "id": "criterion-1",
      "status": "satisfied",
      "evidence": "Eight ranked findings include severity, concrete file/line references, impact, and measurement/test suggestions; Workbench cache growth/media latency, UG verify timing/RSS, and pagination behavior were directly probed."
    }
  ],
  "changedFiles": [],
  "testsAddedOrUpdated": [],
  "commandsRun": [
    {
      "command": "devenv shell cargo test -p brain-brew-formats --test ultimate_geography_fixture ultimate_geography_fixture_manifest_composes_all_targets -- --exact",
      "result": "passed",
      "summary": "UG all-target composition test passed: 1 passed in 13.66 s."
    },
    {
      "command": "devenv shell cargo test -p brain-brew-formats --test ultimate_geography_fixture ultimate_geography_fixture_formatting_is_byte_idempotent -- --exact",
      "result": "passed",
      "summary": "UG deterministic formatting test passed: 1 passed in 1.19 s."
    },
    {
      "command": "TIMEFORMAT=...; time target/debug/brainbrew verify --manifest fixtures/ultimate-geography/brainbrew.yaml --target de-hardcore-extended --skip-content-validation",
      "result": "passed",
      "summary": "One complex target verified in 0.500 s (0.490 s user CPU)."
    },
    {
      "command": "TIMEFORMAT=...; time target/debug/brainbrew verify --manifest fixtures/ultimate-geography/brainbrew.yaml --all-targets --skip-content-validation",
      "result": "passed",
      "summary": "All 74 targets verified in 18.910 s (18.803 s user CPU)."
    },
    {
      "command": "sample /proc/<pid>/status while target/debug/brainbrew verify ... --all-targets --skip-content-validation",
      "result": "passed",
      "summary": "All-target verification completed with sampled maximum VmRSS 24,308 KiB."
    },
    {
      "command": "start Workbench on UG and request note-list limit=1 for all 93 valid language/target/overlay selections while sampling /proc/<pid>/status",
      "result": "passed",
      "summary": "All requests succeeded; retained RSS grew from 20,228 KiB to 448,672 KiB in 32.368 s."
    },
    {
      "command": "request the same declared UG media path twice from a fresh Workbench server",
      "result": "passed",
      "summary": "First request took 14.630 s while composing all target media catalogs; the cached second request took 0.002 s."
    },
    {
      "command": "warm UG Workbench context and compare note/card/source list requests at limit=1 and limit=50",
      "result": "passed",
      "summary": "Responses were byte-bounded, but warm median compute stayed about 51-71 ms independent of page size."
    },
    {
      "command": "fixture/source inspection with du, find, wc, rg, jj status, and line-numbered source reads",
      "result": "passed",
      "summary": "Confirmed UG/source sizes, deterministic collection use, recursive walking, clone/parse paths, and pre-existing audit-only working-copy changes."
    }
  ],
  "validationOutput": [
    "UG fixture: 74 targets, 16 languages, 319 base notes, 546 media references, 92 overlays, 119 files, 1.4 MiB source.",
    "Verify all targets: elapsed=18.910 user=18.803 sys=0.028; sampled_maxrss_kb=24308.",
    "Workbench 93-selection probe: baseline_rss_kb=20228 final_rss_kb=448672 elapsed_s=32.368 failures=0.",
    "Workbench first/warm media fetch: 14.630 s / 0.002 s for the same declared path.",
    "Warm pagination probe: note 0.0666/0.0710 s, card 0.0598/0.0588 s, source 0.0513/0.0541 s for limit 1/50."
  ],
  "residualRisks": [
    "Real Ultimate Geography media bytes were unavailable, so repeated media I/O and peak media memory were established from code rather than measured on release assets.",
    "No release-mode benchmark or allocator profile was run; debug timings are comparative only.",
    "Contextual-translation quadratic behavior and broad package-root traversal magnitude were not stress-run; both are explicitly marked as unmeasured scale risks.",
    "plan.md and progress.md requested by the task were absent."
  ],
  "noStagedFiles": true,
  "notes": "Review-only: no product source or tests were edited. audit/12-performance.md is the requested report artifact and is excluded from changedFiles. Jujutsu has no Git staging area; no commit/staging operation was performed."
}
```
