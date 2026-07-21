# Brain Brew audit synthesis

> **Current fixture resolution:** This historical synthesis predates the full
> pinned Ultimate Geography fixture now described in
> `documentation/docs/reference/ultimate-geography-fixture.md`. The repository
> now has an exact UG `1017a399...` source/media snapshot, locked alpha.3
> provenance, 100 mandatory parsed outputs (74 + 26), strict real-media tests,
> and separate sync/accept/read-only-check boundaries. Historical live-consumer
> and unrelated architecture/release findings below remain unchanged.

## Executive assessment

Brain Brew has a coherent domain model, deterministic happy-path composition, unusually broad integration coverage, and a credible local-first architecture. The primary crate direction is sound (`brain-brew-core` → `brain-brew-formats` → `brainbrew`), the maintained fixture composes 74 main and 26 Hardcore targets, and the default test and browser gates are green.

It is **not release-ready or safe to describe as fail-closed**. The combined audit found independently reproduced paths for silent source loss, stale or partial Workbench writes, lock inputs escaping their authenticated tree, incomplete destructive-change checks, and publication from an immutable already-used version. The production Ultimate Geography checkout—the first consumer—is not the same state as the fixture and cannot currently pass its documented release gate.

### Bottom line

- **Strengths:** deterministic ordered models/emitters; strict rejection of many unsupported CrowdAnki fields; broad command/API/browser coverage; real 74+26 target fixture; loopback Workbench; content-addressed cache validation when hashes are present; fresh embedded Workbench assets.
- **Release blockers:** immutable crates.io version collision; broken Nix package path; mutable/unhashed package escape paths; silent YAML duplicate/union loss; unsafe Workbench write/conflict behavior; production UG noncanonical and unhashed; no mandatory independent release oracle.
- **Confidence caveat:** 404 non-browser tests and 13 browser tests pass, but several central oracles are incomplete or optional. Green tests prove the maintained fixture and happy paths, not production UG readiness, full round-trip equivalence, or adversarial filesystem safety.
- **Recommended posture:** freeze release claims and mutating-workflow expansion; first restore source/integrity invariants and package/release gates, then complete product workflows, then optimize/deepen architecture.

### Reconciled facts and audit disagreements

1. **Fixture media:** `fixtures/ultimate-geography/media.yaml:1-9` has empty `sha256` values. The fixture is canonical and all 74 targets verify *without* `--media-root`, because that path checks references but not hash presence. It is therefore incorrect to infer asset integrity from the green fixture verification. `crates/brain-brew-formats/tests/ultimate_geography_fixture.rs:258-328` proves a 546-entry hoisted map and semantic equivalence to inline declarations, not non-empty hashes.
2. **Fixture versus production:** the checked-in fixture is a migrated future/derivative state with structured `!image`, hoisted `media.yaml`, and a language/profile catalog. The live checkout at `/home/jmo/Development/external/ultimate-geography` remains noncanonical, has inline/blank-hash media, and lacks the language catalog. Fixture success is not production-consumer acceptance.
3. **Test count:** the maintained default gate lists and passes **404 non-browser tests**. The WebDriver executable has **13 test cases**; those cases contain many sub-workflows, which explains artifacts describing a larger workflow count.
4. **Media validation wording:** `verify` always checks reference consistency, but only `--media-root` invokes byte/hash validation (`crates/brain-brew-cli/src/commands/verify.rs:47-79`; `crates/brain-brew-cli/src/media_assets.rs:8-45`). Export is weaker: without `--media-root`, it skips the reference validator as well (`crates/brain-brew-cli/src/commands/export.rs:86-100`).
5. **External checkout availability:** one CLI artifact reported no external checkout in its local probe, while the flow-map and dedicated consumer audit inspected `/home/jmo/Development/external/ultimate-geography`. The dedicated consumer evidence is more specific and is used here.
6. **Working-copy status:** individual artifacts differ in whether they list their allowed report as a changed file. Current `jj status` shows only `audit/01-flow-map.md` through `audit/15-release-security.md` before this synthesis; no product/source file is modified.

## End-to-end flows

### 1. Maintainer source → canonical in-memory deck

```text
Canonical deck YAML + scalar/media !include files
  -> CLI read and include resolution
  -> strict format decoder
  -> CanonicalDeck validation
```

- Entry points: `crates/brain-brew-cli/src/io.rs:47-96`.
- Canonical codecs and deterministic emitters: `crates/brain-brew-formats/src/canonical_yaml.rs:23-182`.
- Include constraints/resolution: `crates/brain-brew-formats/src/source_includes.rs:13-78,321-566`.
- Domain validation: `crates/brain-brew-core/src/validate.rs:12-227`.

The intended boundary is clean at the top level, but source mutation is fragmented: Workbench, media, and translation commands each manipulate YAML in CLI code. Dynamic-map duplicate keys and malformed union branches are not fail-closed, and conversion errors frequently lose file/schema location.

### 2. Manifest/target/package → ordered composition plan

```text
root manifest
  -> ManifestRegistry::load
  -> explicit includes / package-root discovery / sibling lock packages
  -> target extends expansion
  -> overlay dependency expansion and de-duplication
  -> base and ordered overlay reads
  -> ManifestTargetPlan
```

- Package-aware spine: `crates/brain-brew-cli/src/io.rs:151-180,196-435`.
- Local-only duplicate expansion engine: `crates/brain-brew-formats/src/manifest.rs:22-151`.
- Package dependency checks: `crates/brain-brew-cli/src/package_resolver.rs:9-61,64-93`.

Ordering and cycle checks are deterministic for the paths they cover. However, dependency validation is bypassed for explicit `--include`, package-qualified `targets --json` uses the wrong local expansion path, catalog ID/kind is not checked against loaded overlay content, and package/base/overlay paths lack package-root containment.

### 3. Base + overlays → composed target

```text
clone base
  -> resolve structured messages
  -> for each ordered overlay:
       translation dictionary
       deck metadata
       note types/templates/fields
       notes/field fills
       media
       resolve messages
  -> final validation
```

- Main algorithm: `crates/brain-brew-core/src/compose.rs:10-43,76-151`.
- Conflict attribution and override rules: `compose.rs:1594-1640`.
- Structured messages: `crates/brain-brew-core/src/messages.rs:5-196`.

The standard intent/conflict matrix is well tested, but complete entity replacement often checks only that an expected base exists, structured image/message values are not treated as semantic field values during blank checks, flat tombstones alias entity kinds, and message references are resolved from one stale snapshot.

### 4. Translation extraction/application → localized target

```text
incremental deck at translation overlay
  -> extract source units/context
  -> precedence: adaptation / ignored / variable / contextual / direct /
                 no-change / stale / fallback
  -> coverage report
  -> apply dictionary
  -> continue composition
```

- Extraction and precedence: `crates/brain-brew-core/src/translation.rs:28-157,240-310`.
- CLI reporting/rewrite: `crates/brain-brew-cli/src/commands/translations.rs:23-121,975-1335,2682-2920`.
- Workbench mutation has a second policy implementation: `crates/brain-brew-cli/src/commands/workbench.rs:4243-4393,4682-4795`.

Stale/direct/contextual behavior is mostly explicit. Major gaps are field-name translation without Mustache rewrite/validation, strict coverage evaluated independently per split overlay, blank direct translations acting as implicit global deletion, and UG-specific hidden `target_additions` syntax.

### 5. Verify → release-readiness report

```text
canonical root source checks
  -> target planning
  -> incremental translation policy
  -> composition + validation
  -> media reference checks
  -> optional media bytes/hash checks
  -> render variables + HTML/CSS checks
  -> optional CrowdAnki golden comparison
```

- Command: `crates/brain-brew-cli/src/commands/verify.rs:18-90,94-350`.
- Exact CrowdAnki comparison: `crates/brain-brew-formats/src/crowdanki.rs:92-568`.

This is conceptually the right release gate. It is weakened by optional media-byte validation, optional and absent UG goldens, root-biased canonical checks, repeated parsing/composition, and a strict translation policy that cannot model split overlay responsibility.

### 6. CrowdAnki import → canonical source bootstrap

```text
folder/deck.json
  -> deny-unknown strict JSON
  -> reject unsupported adapter fields/defaults
  -> derive suggested stable IDs
  -> preserve model UUID/note GUID adapter IDs
  -> media declarations with empty hashes
  -> optional strict whole-field image conversion
  -> validate
  -> write canonical YAML
```

- CLI: `crates/brain-brew-cli/src/commands/import.rs:8-37`.
- Adapter: `crates/brain-brew-formats/src/crowdanki.rs:788-925,936-1218`.

This is a bootstrap, not a safe round-trip workflow. It requires blanket `--accept-suggested-ids`, has no review/override artifact, ASCII-only note slugging collides on non-Latin content, accepts duplicate GUIDs, normalizes malformed template ordinals, imports no media bytes, and overwrites output without `--force` or atomic replacement.

### 7. Composed target → CrowdAnki export/media folder

```text
compose
  -> variable/message/image lowering
  -> adapter validation/defaults/identity
  -> deterministic deck.json
  -> optional asset validation and declared-file copy
```

- Export adapter: `crates/brain-brew-formats/src/crowdanki.rs:13-82,571-764`.
- CLI/output: `crates/brain-brew-cli/src/commands/export.rs:17-120`.
- Asset handling: `crates/brain-brew-cli/src/media_assets.rs:8-76`.

Ordering is deterministic and unsupported adapter state is broadly rejected. Without `--media-root`, export may accept undeclared raw references and emits no media. Existing output directories are reused, stale files survive, and a copy failure can leave a mixed old/new tree.

### 8. Package source → lock/cache → federated plan

```text
path / GitHub git / tarball
  -> snapshot/filter/extract
  -> NAR SHA-256
  -> content-addressed cache
  -> package metadata validation
  -> deterministic lock YAML
  -> planner auto-loads sibling lock packages
```

- Update/verify/fetch/cache: `crates/brain-brew-cli/src/commands/lock.rs:25-120,321-601,675-845,924-970`.
- Lock schema: `crates/brain-brew-formats/src/lockfile.rs:12-42,148-229`.

Warm-cache rehashing is a good invariant. It is defeated by optional hashes and by manifests/bases/overlays or symlinks escaping the hashed tree. Network fetches also lack transport, timeout, download, and decompression budgets.

### 9. Workbench browser → staged drafts → source write

```text
loopback server + embedded/dev Leptos UI
  -> workspace/language selection
  -> paginated list + selected detail/pivot
  -> localStorage drafts
  -> apply preview / apply
  -> source and translation mutation
  -> temp files + sequential renames
  -> cache invalidation/refresh
```

- Server/routes: `crates/brain-brew-cli/src/commands/workbench.rs:156-231`.
- Selection/cache: `workbench.rs:464-599,1162-1360`.
- Apply: `workbench.rs:878-1064,3878-4017,4050-4937`.
- UI state/staging: `crates/brain-brew-workbench-ui/src/lib.rs:24-298,1629-1694,3873-4196`; `staging.rs:33-146`.

The real browser happy paths are broad. Safety and completeness are not: fingerprints are informational, preview is bypassable/unbound, multi-file writes can partially commit, include-bearing deck edits can become noncanonical, off-screen staged scopes are omitted, rows after 50 are unreachable, and compatibility pivots still return broad payloads.

### 10. Tag/version → crates/Nix/binaries → consumer release

```text
workspace version
  -> crate package verification/publication
  -> cargo-dist plan/build
  -> GitHub release/Homebrew
  -> consumer pins tool
  -> consumer verify/export/package
```

- Version/package metadata: `Cargo.toml:11-21`.
- Crate publisher: `scripts/publish_crates.sh:53-60,106-122`.
- Nix derivation: `flake.nix:25-41,72-75`.
- Release workflow: `.github/workflows/release.yml:41-166,215-343`.

The local path release smoke passes, but the actual packaged dependency chain does not: `1.0.0-alpha.1` already exists with older APIs. Nix includes E2E without preparing its harness. Release jobs are not gated on CI/package smoke and execute mutable third-party action/installer inputs with publication credentials.

## Critical findings

### C1. Canonical YAML can silently lose accepted maintainer data

- Dynamic maps deserialize directly into `BTreeMap`, so duplicate IDs/fields/targets/packages/media entries overwrite earlier values (`crates/brain-brew-formats/src/canonical_yaml.rs:1406-1428,2122-2135,2316-2325`; `manifest.rs:624-639`; `lockfile.rs:148-154`; `media_map.rs:13-21`).
- Context grouping can itself emit duplicate YAML; a second format pass then deletes a valid translation (`canonical_yaml.rs:819-874`).
- Overlay field/media alternatives are independent options and malformed combinations are accepted then discarded (`canonical_yaml.rs:2176-2225,2249-2275`; `crates/brain-brew-core/src/compose.rs:1178-1205`).

**Reproduced evidence:** temporary `brainbrew fmt` probes retained only the second duplicate overlay/media key; a contextual `denmark` collision emitted duplicate keys and lost one value on the second pass; partial media and orphan/conflicting structured-message payloads were dropped. Existing duplicate coverage tests only a repeated struct field (`crates/brain-brew-formats/tests/manifest_yaml.rs:724-734`).

**Gate:** reject duplicate keys at every dynamic map level and malformed unions before any source-writing workflow is considered safe.

### C2. Locked packages can read mutable content outside the authenticated source tree

- Absolute `package.manifest` causes `Path::join` to discard the fetched root; base/overlay paths are similarly uncontained (`crates/brain-brew-cli/src/commands/lock.rs:48,68,107-108,474-475`; `crates/brain-brew-cli/src/io.rs:94-96,298-310,415-420`).
- Snapshot extraction/copy preserves symlinks (`lock.rs:786-845`).
- `nar_hash` is optional, so manually edited mutable locks verify (`crates/brain-brew-formats/src/lockfile.rs:32-40,176-229`; `lock.rs:94-108,430-454`).

**Reproduced evidence:** lock verification remained green after an out-of-tree host deck changed, and downstream compose emitted the changed name. A tarball whose `brainbrew.yaml` was an absolute symlink outside the archive was accepted and retained in the cache. A hashless path lock verified both before and after mutation.

**Gate:** require typed source-specific lock fields and canonical SRI SHA-256; reject absolute/parent/symlink escapes for manifest, base, overlay, include, and media paths.

### C3. Workbench can overwrite concurrent work, partially commit, or make canonical source noncanonical

- Apply requests have no expected fingerprint/generation (`crates/brain-brew-cli/src/commands/workbench.rs:1636-1661`), despite fingerprints being returned (`workbench.rs:601-618,4929-4952`).
- All temp files are prepared, then targets are renamed sequentially without rollback (`workbench.rs:3878-4017`).
- Any include in a deck sends source edits through generic `serde_yaml::Value` serialization (`workbench.rs:4112-4238`), validated only as parseable YAML (`:4528-4547`).
- Confirm is independent of preview and no preview token binds edits/fingerprints (`crates/brain-brew-workbench-ui/src/lib.rs:1629-1689`; server `workbench.rs:338-359,878-1035`).

**Reproduced evidence:** an externally changed German translation was overwritten by a stale Workbench draft with HTTP 200 and `validation.ok=true`; editing a non-included UG field in an include-bearing deck returned success but subsequent `verify` failed because `deck.yaml` was noncanonical. The checked test explicitly accepts partial rename state (`crates/brain-brew-cli/tests/cli.rs:1153-1184`).

**Gate:** compare-and-swap all affected file fingerprints, bind Confirm to a successful preview, canonicalize through one source-document API, and implement rollback/recovery or reject batches that cannot be transactional.

### C4. Release publication is blocked by immutable version reuse and broken artifact gates

- All crates still declare exact `1.0.0-alpha.1`, already published with older APIs (`Cargo.toml:11-21`).
- `release:crates` downloaded published core and failed formats with 40 missing-symbol errors; path workspace tests cannot detect this.
- Package smoke installs from a workspace path, not extracted `.crate` artifacts (`.github/workflows/package-smoke.yml:14-21`).
- Nix runs all workspace targets, including browser E2E, without building/provisioning the harness (`flake.nix:25-41`; `devenv.nix:53-82`).
- The release workflow does not depend on CI/package smoke and uses mutable action tags/installers with broad credentials (`.github/workflows/release.yml:17-18,56-72,112-145,222-325`).

**Command evidence:** `devenv shell release:smoke` passed; `devenv shell release:crates` failed with 40 API mismatch errors; `nix build .#checks.x86_64-linux.brainbrew -L --no-link` failed all 13 E2E tests because `target/debug/brainbrew` was absent.

**Gate:** bump all publishable crates, verify extracted packages in registry dependency order, fix Nix check separation, and require a green reusable quality/artifact workflow before publication.

### C5. Production Ultimate Geography cannot execute its documented release flow

- Current source fails canonical checks: `deck.yaml` and `overlays/extensions/hardcore.yaml` are not formatter-canonical.
- After temporary formatting, all 546 base media hashes and five Experimental hashes are empty (`/home/jmo/Development/external/ultimate-geography/deck.yaml:5515-7153`; `overlays/variants/experimental.yaml:664-684`).
- Main export with `--media-root` creates no `deck.json`.
- UG CI points to a moving Nix branch, and the current Nix command cannot build (`/home/jmo/Development/external/ultimate-geography/.github/workflows/integrity-check.yml:13-15,25-45`).
- The migration branch is divergent from upstream and parity baseline paths are not immutable.

**Command evidence:** real `verify --all-targets --media-root media` failed on canonical formatting; a formatted temporary main copy then failed on 546 empty hashes; real `en-standard` export failed with no output. A canonicalized temporary companion workspace verified 26 targets and exported a 45-note/56-media target, isolating the main-source problem.

**Gate:** pin a buildable tool revision, reconcile branch history, canonicalize actual source, hash/migrate all declared media, then run mandatory independent goldens before release.

## High findings

### H1. Destructive composition checks are not semantically fail-closed

`has_expected_base` checks only `Option::is_some()` (`crates/brain-brew-core/src/compose.rs:1619-1633`). Complete card-template, field-definition, note, and note-type replacement therefore does not compare the supplied prior value (`compose.rs:296-327,406-516,838-899,961-990`). Media replacement accepts presence-only baselines (`:1223-1293`). Structured image fields use an empty raw placeholder, so add/merge/field-fill can erase images while treating the field as blank (`:1124-1205`).

Typed/path-addressed tombstones are also absent: one `BTreeSet<StableId>` can alias note and note-type IDs, while template/field/media removals are not tombstoned (`crates/brain-brew-core/src/model.rs:674-685`; `compose.rs:251-293,472-487,862-872,923-958,1258-1268`; `validate.rs:203-227`).

### H2. Structured messages and semantic diff can return wrong or false-equal state

Messages render all references against one snapshot and do not detect cycles (`crates/brain-brew-core/src/messages.rs:95-113,133-190,300-307`). Multi-hop references can retain stale values.

`semantic_diff` omits deck ID/adapter IDs and structured note messages/images (`crates/brain-brew-core/src/compose.rs:45-67,1972-2052`). CrowdAnki round-trip tests rely on this incomplete oracle (`crates/brain-brew-formats/tests/crowdanki.rs:25-34,163-205`; `crates/brain-brew-cli/tests/ug_style_fixture.rs:113-124`).

### H3. Field-definition translation can break every template silently

Coverage exposes field-definition names as translatable and application renames them (`crates/brain-brew-core/src/translation.rs:73-82,1167-1176`) but does not rewrite `{{Field}}` references in template HTML (`translation.rs:1178-1203`). Content validation checks HTML, not Mustache field references (`crates/brain-brew-core/src/content_validation.rs:93-125`). A probe translated `Capital` to `Hauptstadt`; export succeeded with field `Hauptstadt` while templates still used `{{Capital}}`.

Decision required: make field names structural/non-translatable, or implement atomic reference rewrite plus Anki-template validation.

### H4. Translation completeness and UG ordering are not compositional

- Strict coverage judges each translation overlay against the entire intermediate deck (`crates/brain-brew-core/src/translation.rs:48-154`; `crates/brain-brew-cli/src/commands/verify.rs:136-187`), making split base/extension dictionaries impractical.
- UG main Hardcore stacks apply base translation, then English extension/fills, but do not select companion content translations. The separate companion manifest demonstrates the correct order (`/home/jmo/Development/external/ultimate-geography/brainbrew.yaml:55-60,367-370`; `brainbrew-hardcore.yaml:70-79`). A fresh `cs-hardcore-standard` export left American Samoa prose in English.
- Duplicated capital-hint variables are only half translated in 13 languages (`deck.yaml:14-15`; representative `overlays/languages/cs.yaml:712-724`; templates at `templates/ultimate-geography/capital-country/{question,answer}.html:8`).
- Blank direct translations count as successful global replacement/deletion (`crates/brain-brew-core/src/translation.rs:510-519,1507-1511,1549-1558`).

### H5. CrowdAnki import/identity handling is incomplete

- Note IDs slug only ASCII from the first field; two Cyrillic notes become `note.unnamed` (`crates/brain-brew-formats/src/crowdanki.rs:1181-1187,1289-1307`). The diagnostic mentions a suggested-ID override path that does not exist (`:1212-1214`; CLI `crates/brain-brew-cli/src/commands/import.rs:8-35`).
- Duplicate note GUIDs are not rejected and are re-exported (`crowdanki.rs:838-882,641-649,686-691`).
- Template ordinals are sorted but not required to be unique/contiguous and are renumbered on export (`crowdanki.rs:597-613,1008-1023,1085-1100`).

**Reproduced:** Russian import collided; duplicate GUID import exited 0; ordinal `[99,1,2,3]` was accepted and emitted as `[0,1,2,3]` in changed order.

### H6. Media/source mutators and release checks are not transactional or ownership-aware

- `media hash` and `images-to-refs` write one source at a time (`crates/brain-brew-cli/src/commands/media.rs:40-116`), so later failure leaves earlier changes.
- Source collection includes locked dependency overlays and can rewrite content-addressed cache entries using the root package's media root (`media.rs:285-379`; `crates/brain-brew-cli/src/io.rs:395-420`).
- Verify without media root accepts empty/malformed hashes; export without media root skips reference validation (`verify.rs:47-79`; `export.rs:86-100`).
- All-target media verification rereads and retains every asset per target (`verify.rs:51-79`; `media_assets.rs:23-45`).

A missing later asset reproduction changed the base hash before exiting 1. Mutation commands must plan/validate fully, retain package ownership, and commit only root-workspace files transactionally.

### H7. Package registry behavior differs by discovery route

Dependency validation runs for package roots/locks but not explicit `--include` (`crates/brain-brew-cli/src/io.rs:202-241`; `commands/targets.rs:27-38`). It does not detect package cycles or enforce `compatible_base_versions` (`package_resolver.rs:16-61`; manifest `crates/brain-brew-formats/src/manifest.rs:159-165,670-688`). `targets --json` uses local-only expansion and fails on package-qualified overlays (`commands/targets.rs:40-59`; `io.rs:460-508`). Manifest catalog ID/kind is not checked against loaded overlay identity/kind (`io.rs:314-325,382-421`).

Reproductions showed missing dependencies accepted via `--include` and package-qualified JSON listing failing despite successful composition.

### H8. Workbench is incomplete beyond its write-safety blockers

- List APIs cap at 50 by default, but the UI sends no offset and offers no pagination controls (`crates/brain-brew-workbench-ui/src/lib.rs:530-599,1013-1044,1287-1345`). Items 51+ are unreachable.
- Detail mounting overwrites deck-wide progress with selected-note DOM counts (`lib.rs:2275-2287,3743-3805`).
- Apply collects current and mounted prefixes, omitting staged edits from unmounted language/target scopes (`lib.rs:3895-3921`; `staging.rs:82-105`).
- Card/source/metadata still depend on broad compatibility pivots; comparison returns all notes/strings/cards (`server workbench.rs:191-220,771-812,2437-2635`; UI `lib.rs:1813-1961,4073-4195`).
- The unauthenticated API accepts arbitrary Host/Origin and rendered deck HTML enters `inner_html` without CSP (`workbench.rs:191-229`; UI `lib.rs:1085-1091`; embedded `index.html:1-25`).

A foreign `Host: attacker.example` / `Origin: https://attacker.example` health request returned 200 and disclosed the manifest path. Loopback binding is useful but insufficient for a local file-writing service.

### H9. Workbench caches have correctness and measured scale failures

- `selected_contexts` is unbounded and each entry retains three full decks, overlays, and coverage (`crates/brain-brew-cli/src/commands/workbench.rs:464-493,1186-1284,1746-1754`). Across 93 UG selections RSS rose from 20,228 KiB to 448,672 KiB.
- The first media request composes every target (`workbench.rs:1127-1183`): measured 14.630 s cold versus 0.002 s warm.
- Freshness signatures omit scalar/media includes and locked source dependencies (`workbench.rs:517-598,4929-4952`), so cached contexts can remain indefinitely stale.

### H10. Installation/onboarding claims are not executable

- GitHub tag/release/installer URLs return 404 and the Homebrew tap has no formula, while docs advertise them (`README.md:42-69`; `documentation/docs/getting-started/install.md:9-72`). The available crates.io alpha lacks documented `workbench`/`media` commands.
- The quickstart omits directory creation, uses noncanonical snippets, and compose does not create output parents (`documentation/docs/getting-started/quickstart.md:23-87`; `crates/brain-brew-cli/src/commands/compose.rs:31-34,74-76`).
- The documented import omits mandatory `--accept-suggested-ids` (`documentation/docs/reference/cli.md:93-99`; `commands/import.rs:15-18`).

## Medium findings

1. **CLI destructive/output UX:** import overwrites existing files and ignores unknown options (`crates/brain-brew-cli/src/args.rs:295-302`; `commands/import.rs:27-36`); export reuses dirty directories (`commands/export.rs:94-101`); compose does not create parents; parse errors often omit the source filename (`io.rs:68-96`); `translations --json` emits plain-text errors because JSON dispatch excludes it (`main.rs:82-87`).
2. **Media path safety:** lexical checks are duplicated and not symlink-safe; HTML-breaking path characters can enter rendered `<img src>` (`crates/brain-brew-cli/src/media_assets.rs:29-76`; `commands/media.rs:618-627`; core `compose.rs:1782-1804`).
3. **YAML typing/diagnostics:** serde coercion turns YAML booleans/null/numbers into strings; public overlay/manifest/lock emitters can panic on constructible invalid map keys; conversion errors lack schema/source location (`canonical_yaml.rs:810,1431-1455,2371-2380,2525-2569`; `manifest.rs:281-315`; `lockfile.rs:54-85`).
4. **Core error shape:** final composition flattens validation categories to `ValidationFailed` (`crates/brain-brew-core/src/compose.rs:27-39`), forcing machine clients to parse English.
5. **Translation schema debt:** `target_additions` uses the magic reason `"target addition from upstream UG"` and is undocumented (`crates/brain-brew-formats/src/canonical_yaml.rs:20,730-744,1754-1771`). Deck and note-type display identity also use inconsistent variable strategies.
6. **Verification throughput:** one UG complex target measured 0.500 s; 74 targets measured 18.910 s/18.803 s user CPU because registry/source/compose prefixes are repeated (`verify.rs:51-70,136-185`; `io.rs:173-325`; core `compose.rs:10-40`). Memory was good at sampled 24,308 KiB.
7. **Pagination computes full collections:** note/card/source list endpoints build full rows before slicing (`workbench.rs:2119-2300`); measured warm latency was similar for limit 1 and 50.
8. **Package discovery:** recursive package-root traversal does not prune `.git`, `.jj`, `target`, outputs, or symlink cycles (`crates/brain-brew-cli/src/package_resolver.rs:7-13,64-86`).
9. **Lock network budgets:** HTTP is accepted and response/extraction sizes, redirects, timeouts, and archive expansion are unbounded (`commands/lock.rs:493-537,604-686`).
10. **Dependency/release hygiene:** no Cargo/npm advisory/license gate; `npm audit --omit=dev` reported 1 high, 24 moderate, 1 low; crate archives omit LICENSE/README; release has no SBOM/provenance/signatures (`Cargo.toml:11-17`; publishable crate manifests `:1-11`).
11. **Docs/schema/recovery:** YAML reference omits variables, sparse overlay shapes, manifest fields, and stable-ID grammar; stale-resolution and lock-update recovery are underdocumented; Workbench docs describe legacy pivots (`documentation/docs/reference/yaml.md:7-47,224-244`; `reference/workbench.md:50-122`).
12. **UG source duplication/packaging:** companion identity preservation duplicates shells/catalog/translation data; docs/CI recopy all 607 media into every output and have no automated archive naming/validation; release version text is duplicated across six descriptions.

## Low findings

1. `diff` has no optional change-sensitive exit code (`crates/brain-brew-cli/src/commands/diff.rs:20-35`).
2. Help/version bypass strict trailing-argument validation (`crates/brain-brew-cli/src/main.rs:38-47,67-70`).
3. Include-format documentation says scalar content materializes, while implementation/tests preserve directives (`documentation/docs/reference/yaml.md:220-222`; `crates/brain-brew-formats/src/source_includes.rs:46-80`). Invalid configured include roots are silently skipped during canonicalization (`source_includes.rs:340-359`).
4. `format_source_at` carries an unused path argument (`crates/brain-brew-cli/src/io.rs:45-47`).
5. Translation contextual/stale/ignore scans may become quadratic at very large dictionary size (`crates/brain-brew-core/src/translation.rs:240-309,1804-1827`); no current UG regression was demonstrated.
6. Native UI testing covers only `WorkspaceSummary`; staging-key/storage failure behavior lacks deterministic unit tests (`crates/brain-brew-workbench-ui/tests/workspace_summary.rs:1-17`; `src/staging.rs:1-163`).
7. Accessibility semantics, live/busy status, labels, active-row state, and keyboard-only coverage are incomplete (`crates/brain-brew-workbench-ui/src/lib.rs:366-369,667-688,1553-1560,1685-1693`).

## Undone product workflows

These are product journeys that are absent, incomplete, or advertised beyond current behavior—not merely implementation bugs.

1. **Reviewable CrowdAnki import:** no inspectable suggested-ID plan, selective override file, duplicate-GUID remediation, or media-byte handoff. Current flow is blanket acceptance into a new full deck.
2. **Anki-to-existing-source reconciliation:** no merge-back into an existing canonical deck/overlay stack. UG previously supported Anki-to-source recipes; import is not a replacement.
3. **Workspace initialization/scaffolding:** no `init` command or tested manual scaffold. The published quickstart is not executable.
4. **Safe source-wide mutation transaction:** media migrations, import, format, and Workbench writes do not share dry-run/plan/atomic commit/recovery semantics.
5. **Complete Workbench editing:** no reachable pagination beyond 50, no complete detail endpoints, no workspace-wide draft manager, no mandatory preview/CAS, no discard-all UX, no graceful shutdown/restart recovery.
6. **Strict compositional translation release gate:** split extension dictionaries cannot jointly satisfy policy, and structural/hidden source units dominate strict fallback counts.
7. **Federated media ownership:** no per-package media-root provenance; mutators can touch locked dependency cache sources.
8. **Independent consumer parity:** goldens are optional/absent; representative historical evidence does not compare complete template/schema/config/media state.
9. **Automated release packaging:** no reliable archive naming/layout validation or declared-media-only release transaction for UG.
10. **UG domain maintenance:** deleted flag similarity analysis has no replacement; its deprecation is not explicitly accepted.
11. **Installable documented release:** release/tag/installer/Homebrew claims do not point to the same build/schema described by current docs.
12. **Supported reusable Rust API:** crates are advertised reusable but lack consumer examples/stability commitments.

## Architecture deepening opportunities

### A1. One source-document mutation boundary

Create pure, include-preserving `CanonicalSourceDocument` / `OverlaySourceDocument` operations in `brain-brew-formats`; keep authorization, bytes, transactions, and filesystem writes in CLI. Migrate Workbench, media hash/image migration, and translation insertion away from direct `serde_yaml`/line surgery (`commands/media.rs:48-65,101-115`; `commands/workbench.rs:4112-4509`; `commands/translations.rs:2682-2820`). This restores the intended crate boundary and gives duplicate/union/canonical checks one implementation.

### A2. Honest workspace transaction service

Replace `write_files_atomically` with a journaled backup/rollback/recovery module, or explicitly constrain operations to cases where all-or-nothing can be guaranteed. Apply the same plan/validate/commit discipline to Workbench, media mutations, import, fmt, lock update, compose, and clean-directory export.

### A3. Centralize translation mutation policy in core

Add domain commands such as `set_direct`, `set_contextual`, `set_no_change`, `record_source_change`, and `resolve_stale`. Today public maps leak cross-map invariants and Workbench/translation CLI independently implement precedence and cleanup (`crates/brain-brew-core/src/model.rs:700-747`; Workbench and translation ranges above).

### A4. Typed, versioned Workbench contract

Introduce shared request/response DTOs and enums for the independently compiled server/UI crates. Current ad hoc `serde_json::Value` indexing silently defaults on contract drift (`crates/brain-brew-cli/src/commands/workbench.rs:233-360,601-779`; UI `lib.rs:947-1385,3872-4229`). Add an explicit API version, JSON error envelope, API 404 behavior, request limits, and per-process capability.

### A5. Complete ADR-015 list/detail architecture, then delete pivots

Implement `card-detail`, `source-string-detail`, and `metadata-detail`; make comparison selected-item scoped; migrate UI/E2E; remove compatibility pivots after a bounded window. This bounds payload/DOM work and makes pagination meaningful.

### A6. Deepen `DeckPath` and safe path authorization

Expose typed parent/entity/field accessors and descendant matching instead of CLI string parsing (`crates/brain-brew-core/src/model.rs:75-620`; translations `:3015-3061`; Workbench `:4396-4430`). Separately create one CLI-owned `SafeRelativePath` plus canonical-root containment for package/media/include writes.

### A7. Consolidate manifest planning

Use one registry-aware planner for compose, verify, explain, targets JSON, translations, and Workbench. Remove or clearly constrain local-only expansion (`formats/src/manifest.rs:22-151`) after registry callers migrate. Retain source provenance and package ownership in plans.

### A8. Split oversized modules along behavior boundaries

Without proliferating public crates, split Workbench server (`routes/contracts/cache/apply/source_io/media/new_language`), UI (`api/state/views/staging/preview`), translations (`report/resolve/source_editor`), and core compose (`compose/render/semantic_diff`). Current sizes are approximately 4,985, 4,243, 3,346, and 2,169 lines respectively.

### A9. Make performance caches bounded and dependency-complete

Use bounded LRU/single-active selection with `Arc` sharing; include the full transitive include/package dependency set in fingerprints; cache compact list indexes; precompute declared media without all-target composition; load registry/source once and memoize composition prefixes.

### A10. Repair architecture records

ADR-0011/0014 still name Iced/WASM while implementation is Leptos; ADR-0015 detail migration remains incomplete. Supersede/amend ADRs, update the five-crate workspace map, and distinguish stable versus internal Workbench APIs (`documentation/docs/reference/decisions/0011-*.md`; `0014-*.md`; `0015-*.md`; `documentation/docs/reference/project-scope.md:32-40`).

## Ultimate Geography consumer blockers

### Immediate release blockers

1. **Tool:** contributors pin `1.0.0-alpha.1`, CI follows a moving Nix branch, and neither maps reliably to current docs/source behavior (`CONTRIBUTING.md:24-45`; `.github/workflows/integrity-check.yml:13-15`).
2. **Canonical source:** actual `deck.yaml` and Hardcore overlay fail current formatting; a temporary format changed 20 YAML files, dominated by quoted UG tags.
3. **Media:** 546 base and five Experimental declarations have empty hashes; main verify/export with media cannot pass.
4. **Branch integration:** migration and upstream are divergent; parity baseline references are moving/machine-specific.
5. **Oracle:** no manifest target configures `exports.crowdanki.golden`; optional fixture parity silently returns success when absent (`crates/brain-brew-formats/tests/ultimate_geography_fixture.rs:536-548,1798-1855`).

### Consumer correctness blockers

6. Main localized Hardcore targets omit companion content translation overlays and can ship English extension content.
7. Capital-hint duplicate variables produce mixed-language card faces in 13 languages.
8. Strict translation policy fails every translated target, including 1,226 hidden German fallbacks; it cannot certify release readiness.
9. Production manifests lack `languages`/`translation_profile`, so the documented Workbench starts but `language=de` returns HTTP 400 (`crates/brain-brew-cli/src/commands/workbench.rs:1294-1319`).
10. Historical parity checks only 12/100 targets and its model signature is `(name, template count)`; templates, CSS, field schema/order, config, and media content are not complete oracles (`scripts/collect-pr736-equivalence-evidence.py:300-355,450-476`).

### Consumer workflow/product debt

11. The fixture must stop being a feature delta. Sync should reproduce actual UG plus an explicit, reviewed migration step—not silently append catalog/profile state (`scripts/sync-ug-fixture.sh:49-66`).
12. Companion identity safety currently requires duplicated shells/catalog/translation files; this needs either accepted duplication with consistency checks or a deeper identity-sharing model.
13. Release docs/CI copy every media file after Brain Brew already copies declarations, masking declaration gaps and multiplying output size.
14. Version text is duplicated in six localized description fragments; release instructions point to the wrong source.
15. Anki-to-source, flag analysis, and ZIP packaging need explicit restore/deprecate decisions before “fully migrated” can be claimed.

## Test-confidence map

| Surface | What green tests genuinely establish | False-confidence gap / required addition |
|---|---|---|
| Core | 79 core tests; paths/invariants; broad intent/conflict and translation examples | Wrong entity expected bases; existing structured-value overwrite; typed tombstones; message graphs/cycles; complete semantic-diff mutation table |
| YAML/formats | Deterministic positive round trips, hostile scalar corpus, includes, unknown fields | Duplicate dynamic keys; malformed union matrix; strict scalar types; direct-emitter panic resistance; generated/fuzz laws |
| CrowdAnki | 42 focused tests; deterministic JSON; many unsupported fields fail closed | Unicode/repeated IDs; duplicate/effective GUID collisions; ordinal gaps/duplicates; complete normalized canonical comparison |
| Manifest/lock/media | Local cycles, path drift, cache rehash, include containment, reference scanning | Package path/symlink escape; hashless locks; discovery parity/cycles; per-package media ownership; network/resource limits |
| CLI | Real binary exercised for every principal command; JSON contracts for selected commands | Destructive overwrite/transaction failure matrix; dirty output; filename-rich parse errors; all JSON modes; cross-platform/interruption behavior |
| Workbench API | 32 named server/API tests; pagination bounds; real writes; two-request serialization | CAS stale files; canonical include write; transaction rollback/restart; preview binding; Host/Origin/capability; complete detail APIs |
| Workbench browser | 13 Chromium cases pass; broad happy paths; real UG smoke; localStorage persistence; lazy views | Item 51+ reachability; progress correctness; stale conflicts; unmounted scopes; failure UX; embedded release bundle; accessibility |
| UG fixture | 74 + 26 composition/validation; canonical/idempotent migrated source; propagation | Production checkout drift; media bytes/hashes; complete old-tool/release goldens; experimental/Hardcore parity |
| Release | Local path `release:smoke`; embedded asset freshness | Extracted `.crate` dependency chain fails; Nix package fails; no pre-tag multi-platform build/quality dependency; no SBOM/provenance |
| Docs | Docusaurus build and local links pass | Command snippets not executed; install channels dead; quickstart/import examples fail; no schema completeness check |

### Executed aggregate evidence

- `cargo test --workspace --exclude brain-brew-workbench-e2e --all-targets`: **404 passed, 0 failed**; UG fixture dominated runtime (~162 s of ~189 s in one audit run).
- `devenv shell e2e`: **13 passed, 0 failed** (reported runs of 16.57–22.02 s).
- `devenv shell workbench-ui-embed-check`: passed; embedded assets byte-match release build.
- No ignored tests, snapshots, property framework, fuzz target, or mutation-testing gate was found.
- The optional UG release-oracle test reports success by early return when the oracle is absent; it is not an actual skip/failure.

## Recommended sequenced roadmap

### Now — restore integrity and establish a releasable baseline

1. **Freeze publication and release claims.** Bump all publishable crates from the immutable alpha; update exact internal dependencies, changelog/docs/dist tag; verify extracted packages in registry order.
2. **Make source decoding fail closed.** Reject duplicate dynamic keys and malformed overlay unions; add byte-unchanged CLI failure tests and path/location diagnostics.
3. **Close package trust boundaries.** Require valid hashes/source-specific lock fields; reject absolute/parent/symlink escapes; add HTTPS/time/size/extraction budgets.
4. **Fix core destructive invariants.** Representation-aware field values, real expected-base comparisons/fingerprints, typed tombstones, unknown structured-media validation, message DAG/cycle errors, complete semantic diff.
5. **Disable or harden unsafe Workbench writes.** Until CAS + canonical source mutation + preview binding + rollback/recovery land, clearly mark Apply experimental or read-only for release builds.
6. **Repair release gates.** Separate Nix package tests from prepared E2E; make release depend on CI/package/artifact smoke; pin actions/installers; minimize tokens; add advisory/license gates.
7. **Bring actual UG to a pinned baseline.** Reconcile branch, canonicalize source, migrate/hash all media, add language/profile catalog if Workbench remains promised, and remove fixture-only divergence.
8. **Commit mandatory representative CrowdAnki goldens.** Include source language, non-Latin, RTL/CJK, Extended, Experimental, standalone and companion Hardcore; use narrow explicit allowlists.

**Now exit criteria:** no silent parse/format loss; no out-of-tree locked reads; all destructive core regressions covered; Workbench cannot stale/partial-write; extracted crates and Nix package build; actual UG verifies/exports with media under the pinned tool; independent goldens execute in CI.

### Next — complete maintainer and consumer workflows

1. Build a reviewable Unicode-safe import plan/override flow; reject GUID/ordinal identity defects; add media-byte handoff and safe output semantics.
2. Define compositional translation ownership/strictness; fix UG Hardcore overlay order and shared hint variable; decide field-name translation policy and blank deletion intent.
3. Complete Workbench pagination/detail/comparison, workspace-wide drafts, discard/recovery, graceful shutdown, capability/Host/Origin/CSP policy, and embedded-bundle E2E.
4. Introduce shared source-document operations and the workspace transaction service; migrate media/import/fmt/Workbench mutators.
5. Make manifest planning registry-consistent and provenance-aware; protect locked dependency sources and support per-package media roots.
6. Restore or explicitly deprecate UG Anki-to-source, flag-analysis, and archive-packaging workflows; single-source release version metadata.
7. Execute quickstart/import/install documentation in CI and publish a complete schema/migration/recovery reference.

### Later — deepen and scale

1. Split monoliths behind behavior-oriented private modules and typed Workbench contracts.
2. Add bounded shared Workbench caches, transitive fingerprints, compact list indexes, and cheap media catalogs.
3. Memoize target composition prefixes and stream/deduplicate media hashing before considering bounded parallelism.
4. Add property tests, fuzz targets, panic-resistance corpora, mutation testing, and scheduled large-media/package-root benchmarks.
5. Add SBOM, provenance, signatures, artifact attestations, pre-tag macOS/Windows/Linux target smoke, and explicit ARM Linux policy.
6. Publish supported Rust API examples/stability commitments or reclassify crates as implementation packages.

## Open decisions

1. **Field names:** structural and non-translatable, or translatable with complete Anki reference rewrite/validation?
2. **Expected bases:** canonical entity summaries, stable fingerprints, sparse property-only replacement, or removal of complete replacement APIs?
3. **Tombstones:** typed entity/path variants and compatibility migration format?
4. **Strict translation:** combined target-stack completeness, source-unit ownership by introducing overlay, or another compositional policy?
5. **Blank translation:** reject, warn as a distinct reviewed category, or require path-scoped target adaptation/deletion intent?
6. **Workbench threat model:** are untrusted workspaces/browser origins in scope? Regardless, should every process require a capability token and CSP/sanitization?
7. **Workbench transaction boundary:** are external include roots/cross-filesystem writes allowed in one Apply, or rejected when atomic recovery cannot be guaranteed?
8. **Workbench maturity/API:** stable versioned product surface now, or explicitly internal/experimental until detail migration and safety work finish?
9. **Media verification:** should declared media make `--media-root` mandatory for release verify/export, and can manifests configure it?
10. **Package compatibility:** exact versions only, real semver ranges, or remove inert `compatible_base_versions` until implemented?
11. **Symlinks:** reject all fetched-package symlinks, or permit only links canonically contained within the package root?
12. **UG fixture:** exact production snapshot, production plus explicit generated migration patch, or intentionally future-facing fixture with a separately mandatory live-consumer gate?
13. **UG lost workflows:** restore Anki reconciliation/flag analysis/ZIP packaging, or formally deprecate each with replacement guidance?
14. **Release channels:** crates.io only for the next preview, or synchronize crates/tag/GitHub installers/Homebrew/Nix before advertising any channel?
15. **Public Rust crates:** supported semver API surface with docs, or internal crates published only to support the CLI?

## Speculative or unmeasured items

The following are plausible from direct control flow but were not reproduced at production scale; they should not be presented as confirmed incidents:

- All-target media hashing could reach 40,404 reads for 74 targets × 546 common assets and high peak asset memory; no real UG media-byte benchmark was available.
- Recursive package discovery can traverse huge irrelevant trees or symlink loops; the magnitude was not stress-tested.
- Contextual translation lookup can approach quadratic behavior with very large path-scoped dictionaries; current UG did not demonstrate a regression.
- CrowdAnki export/golden comparison may hold several complete deck/JSON copies; no UG-size memory failure was measured.
- Cross-platform rename/replacement and Windows path behavior remain untested.
- DNS rebinding is a verified acceptance surface (foreign Host/Origin returns 200), but exploit priority depends on the stated threat model and browser conditions.
- Cold remote Git/tarball lock behavior and external service failure modes were inspected, not exercised broadly against live services.

## Acceptance report

```acceptance-report
{
  "criteriaSatisfied": [
    {
      "id": "criterion-1",
      "status": "satisfied",
      "evidence": "Synthesized all 15 requested audit artifacts into audit/16-synthesis.md only, with reconciled evidence, required sections, sequenced roadmap, and no project or external-consumer source modification."
    },
    {
      "id": "criterion-2",
      "status": "satisfied",
      "evidence": "The report preserves exact file:line and command evidence, distinguishes reproduced from speculative issues, maps all end-to-end flows and test confidence, and records residual risks for independent review."
    }
  ],
  "changedFiles": [
    "audit/16-synthesis.md"
  ],
  "testsAddedOrUpdated": [],
  "commandsRun": [
    {
      "command": "read audit/01-flow-map.md through audit/15-release-security.md",
      "result": "passed",
      "summary": "Read all 15 artifacts in full (3,142 lines total) and reconciled overlapping findings and disagreements."
    },
    {
      "command": "read fixtures/ultimate-geography/media.yaml and crates/brain-brew-formats/tests/ultimate_geography_fixture.rs:250-339",
      "result": "passed",
      "summary": "Confirmed the fixture media map is hoisted/canonical but has empty hashes; tests assert entry count/equivalence rather than hash completeness."
    },
    {
      "command": "jj status && jj diff --summary",
      "result": "passed",
      "summary": "Confirmed only requested audit artifacts are working-copy additions and no product/source files are changed."
    },
    {
      "command": "python3 synthesis structure and acceptance-report validation",
      "result": "passed",
      "summary": "Validated required section headings and parsed the fenced acceptance-report as JSON."
    }
  ],
  "validationOutput": [
    "All 15 source artifacts read: 3,142 lines.",
    "Required synthesis sections present, including a separately labeled speculative/unmeasured section.",
    "Acceptance report JSON parsed successfully.",
    "Current working-copy changes are audit artifacts only; no project/source or Ultimate Geography source file is modified."
  ],
  "residualRisks": [
    "This is a synthesis/review artifact; no product fixes or regression tests were added.",
    "Several source audits used temporary adversarial fixtures and code-path reasoning; speculative or unmeasured items are explicitly separated.",
    "The live Ultimate Geography checkout is divergent and not a clean production baseline, so its release state must be revalidated after reconciliation.",
    "External release-channel availability and dependency advisory counts are time-sensitive as of 2026-07-09.",
    "Review gate remains required."
  ],
  "noStagedFiles": true,
  "notes": "Jujutsu has no staging index. The allowed output is audit/16-synthesis.md; audit/01 through audit/15 were pre-existing inputs from the parallel audit."
}
```
