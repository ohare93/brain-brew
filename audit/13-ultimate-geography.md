# Ultimate Geography production-consumer audit

## Review

- **Correct:** UG's migrated source follows the intended variable-first/shared-extension model in important places. The base note type defines reusable variables (`deck.yaml:11-21`), standard templates are external includes (`deck.yaml:53-74`), Extended templates exist once (`overlays/variants/extended.yaml:1-28`), and per-language variant overlays are normally identity-only 11-line files. Hardcore blank-field content uses `field_fills` (`overlays/extensions/hardcore/field-fills.yaml:1-84`).
- **Correct:** Native composition works once source formatting/media state is made compatible in a temporary copy. The current binary listed 74 main targets and 26 companion targets. A canonically formatted temporary copy of `brainbrew-hardcore.yaml` verified all 26 targets, and `de-hardcore-companion-standard` exported 45 notes, one note type, four templates, and 56 declared media assets.
- **Blocker:** The actual production checkout cannot pass its documented verification or export its main deck with the installed/current Brain Brew. Details are B1-B3 below.
- **Note:** No source file in either repository was modified. This report is the only requested output.

## Scope and baseline

Paths used below:

- **UG:** `/home/jmo/Development/external/ultimate-geography`
- **BB:** `/home/jmo/Development/projects/brain-brew`

The requested `BB/plan.md` and `BB/progress.md` do not exist, so there was no plan/progress state to inspect. UG is not currently its upstream production branch: `@-` is `cc886bac5f8e` (`docs: address PR 736 Brain Brew review concerns`), four commits above bookmark `brainbrew-hebrew-demo`, while `master`/`brainbrew-migration` remain at `af35d4d14867`; `master@upstream` is a divergent head at `fee657631c04`. The working copy also contains untracked `.frontloop/**` planning records and `scripts/__pycache__/...pyc`.

## Consumer journey map

| Journey | UG source/workflow | Brain Brew path | Observed result |
|---|---|---|---|
| Install/discover CLI | `CONTRIBUTING.md:24-45` pins `1.0.0-alpha.1`, while CI uses a moving Nix branch at `.github/workflows/integrity-check.yml:13-15` | CLI dispatch/help; Nix package in `BB/flake.nix:22-49,57-69` | Profile-installed alpha runs, but rejects current UG. The documented Nix command currently cannot build (B2). A devenv `brainbrew` wrapper also shadows the installed binary and runs `cargo` in the caller CWD (`BB/devenv.nix:29-31`). |
| Discover build matrix | `brainbrew.yaml:355-605`; `brainbrew-hardcore.yaml:282-373` | Manifest parsing/overlay dependency expansion at `BB/crates/brain-brew-formats/src/manifest.rs:22-42,124-155`; package planning at `BB/crates/brain-brew-cli/src/io.rs:265-360` | 74 standalone/main targets plus 26 Hardcore companion targets are discoverable. Overlay order is deterministic. |
| Edit English/source content | `CONTRIBUTING.md:104-116`; monolithic `deck.yaml` (7,154 lines) plus external templates/descriptions/styles | Canonical YAML/includes; format gate at `BB/crates/brain-brew-cli/src/io.rs:511-535` | Direct YAML editing is the only maintained path. Current checked-in source is not canonical under the current formatter (B1). The former CSV and Anki-to-source workflows were removed (H5). |
| Edit translations | `overlays/languages/*.yaml`; commands at `CONTRIBUTING.md:160-205` | Reports/apply at `BB/crates/brain-brew-cli/src/commands/translations.rs:23-94,982-1024` | Terminal reports work and detect no stale keys, but release-strict mode is unusable and translation debt remains (H4). |
| Use browser workbench | `CONTRIBUTING.md:185-199` | Server/routes at `BB/crates/brain-brew-cli/src/commands/workbench.rs:156-220`; selection requires catalog entries at `:1290-1388` | Server shell starts, but the real manifest returns `languages: {}` and translation APIs cannot select a language. The documented workflow is not usable (H2). |
| Build Standard/Extended/Experimental | Base variables/templates in `deck.yaml:11-74`; shared variants at `overlays/variants/extended.yaml:1-28` and `overlays/variants/experimental.yaml:1-684` | Compose plan then `CanonicalDeck::compose`; export starts at `BB/crates/brain-brew-cli/src/commands/export.rs:34-58` | Structure is clean and shared. Experimental adds one field across all notes and five interactive assets, but all five hashes are blank (`overlays/variants/experimental.yaml:664-684`). |
| Build Hardcore standalone/companion | Two manifests; minimal shell `deck-hardcore.yaml:1-79`; 45-note shared overlay; field fills and three translation overlay families | Same manifest planner/compose path | Companion identity preservation works, but requires duplicated shell/catalog/translation source (M1). |
| Verify for CI/release | `CONTRIBUTING.md:54-61`; CI `.github/workflows/integrity-check.yml:25-33` | Sequential format, translation, compose, validation, media, HTML/CSS and golden gates at `BB/crates/brain-brew-cli/src/commands/verify.rs:18-90` | Both documented main verification and CI are currently blocked (B1-B2). The side HTML/CSS script passes 22 files. |
| Export/package release | `CONTRIBUTING.md:63-90,488-498`; CI export loop at `.github/workflows/integrity-check.yml:35-45` | CrowdAnki conversion/media copy at `BB/crates/brain-brew-cli/src/commands/export.rs:86-120` and `media_assets.rs:23-64` | Main export fails on hashes. CI/docs then manually copy all 607 media into every target, bypassing declarations and duplicating about 1.62 GiB across 100 outputs (B1/M2). No replacement exists for old ZIP naming/packaging (M2). |
| Prove learner upgrade safety/parity | `docs/pr736-equivalence-evidence.md`; learner upgrade warnings at `README.md:119-178,220-294` | CrowdAnki adapter IDs plus optional manifest goldens (`BB/crates/brain-brew-cli/src/commands/verify.rs:303-351`) | Representative note GUID/content evidence exists, but it is not a sufficient release oracle and no golden is configured (H3). |
| Round-trip Anki edits | Former `recipes/anki_to_source*.yaml`; no current UG docs | Import only writes a new deck and requires accepting generated IDs at `BB/crates/brain-brew-cli/src/commands/import.rs:8-37` | No merge-back path into the canonical deck/overlays; a previously supported maintainer flow is gone (H5). |

## Severity-ranked findings

### B1 — Blocker: the actual checkout cannot verify or export the main deck

**Evidence**

1. Both the profile-installed `brainbrew 1.0.0-alpha.1` and `BB/target/debug/brainbrew` fail immediately:

```text
$ brainbrew verify --manifest brainbrew.yaml --all-targets --media-root media
./deck.yaml is not in canonical format

$ brainbrew verify --manifest brainbrew-hardcore.yaml --all-targets --media-root media
./overlays/extensions/hardcore.yaml is not in canonical format
```

`verify` deliberately checks manifest/base/each selected overlay before composition (`BB/crates/brain-brew-cli/src/commands/verify.rs:24-40,51-60`), comparing formatter output byte-for-byte (`BB/crates/brain-brew-cli/src/io.rs:523-535`). Formatting a temporary copy changed 20 source files. The dominant mismatch is 669 quoted `UG::*` tags in `deck.yaml` and 93 in `overlays/extensions/hardcore.yaml`; e.g. `deck.yaml:89-90` is rewritten from quoted to unquoted tags.

2. Canonicalizing a temporary copy only reveals the next blocker: all 546 base media declarations have `sha256: ''` (`deck.yaml:5515-7153`), as do all five Experimental assets (`overlays/variants/experimental.yaml:664-684`). Current verification/export validates hashes whenever `--media-root` is supplied (`BB/crates/brain-brew-cli/src/commands/verify.rs:47-75`; `export.rs:86-100`; `media_assets.rs:23-26`). The result is 546 errors such as:

```text
ug-flag-abkhazia.svg: media entry media.ug-flag-abkhazia-svg (...) has empty sha256
```

3. The release export itself therefore fails and creates no output:

```text
$ brainbrew export crowdanki --manifest brainbrew.yaml --target en-standard \
    --out /tmp/ug-export-en-standard --media-root media
# exits 1 on empty sha256; /tmp/ug-export-en-standard/deck.json is absent
```

By contrast, the Hardcore overlay's 56 declarations are hashed (`overlays/extensions/hardcore.yaml:823-1047`), so a canonicalized temporary companion workspace verifies all 26 targets and exports successfully. This isolates the failure to checked-in main source state, not general composition.

**Impact:** No main Standard, Extended, Experimental, or standalone Hardcore release target can pass the documented release gate or export with media. The release process at `CONTRIBUTING.md:488-498` is blocked.

### B2 — Blocker: UG CI tracks a moving Brain Brew branch that currently does not build

UG CI sets `BRAINBREW_FLAKE=github:jeprecated/brain-brew/rust-brainbrew` (`.github/workflows/integrity-check.yml:13-15`) and invokes it for every verify/export (`:28-41`). This is neither the contributor-pinned alpha (`CONTRIBUTING.md:28-42`) nor an immutable commit.

The exact documented command failed during this audit:

```text
$ nix run github:jeprecated/brain-brew/rust-brainbrew -- --version
error: Cannot build ... brainbrew-1.0.0-alpha.1.drv
...
13 workbench E2E failures: brainbrew test binary not found at /build/source/target/debug/brainbrew
```

The flake package runs `cargo test --workspace --all-targets` (`BB/flake.nix:38-41`), which includes the E2E crate, but that crate expects a separately built `target/debug/brainbrew`. The normal devenv test command explicitly excludes E2E (`BB/devenv.nix:34-36`) and its E2E script builds the binary first (`:53-81`).

**Impact:** A UG push can fail without a UG change; today it cannot even reach UG verification. The unpinned tool can also introduce formatter/hash behavior that contributors' pinned binary does not share.

### B3 — Blocker: the migration branch is divergent and not release-integrated

`jj log` shows:

- `master`/`brainbrew-migration` at `af35d4d14867`, while four later migration commits and the empty working-copy commit are above a different bookmark;
- `master@upstream` at `fee657631c04` on a divergent line;
- six upstream-only commits, including merged PRs #733 and #735 and four Hebrew commits;
- eleven migration-only commits.

The current migration includes its own Hebrew implementation (`ea27eb4a52b9`) while upstream independently merged Hebrew, so a non-trivial reconciliation is required. The equivalence report's old baseline is the moving expression `master@upstream` (`docs/pr736-equivalence-evidence.md:9,18-23`), not an immutable commit ID.

**Impact:** Even after source/tool fixes, this state cannot be released or merged without rebasing/reconciling upstream content and rerunning parity.

### H1 — High: Brain Brew's UG fixture is a forked future state, so green tests do not validate the production consumer

The fixture sync script says it copies UG and applies only an ADR-0012 catalog delta (`BB/scripts/sync-ug-fixture.sh:49-66`), but the checked-in fixture materially differs from UG:

- production `deck.yaml` has inline raw `<img>` fields and 546 blank inline hashes; fixture `deck.yaml` uses structured `!image` values and `media: !include media.yaml`;
- production Hardcore source has raw images; fixture Hardcore has structured images and populated hashes;
- every language overlay and multiple Hardcore files differ;
- fixture manifests add `languages:` and `translation_profile:` (`BB/fixtures/ultimate-geography/brainbrew.yaml:606-850`) absent from production.

Brain Brew's “real UG” composition, translation, CLI, workbench, and browser tests all target this fixture, e.g. `BB/crates/brain-brew-formats/tests/ultimate_geography_fixture.rs:141-179,297-331`, `BB/crates/brain-brew-cli/tests/cli.rs:3042-3114,3336-3405`, and `BB/crates/brain-brew-workbench-e2e/tests/workbench_smoke.rs:275-318`. The optional release-oracle test silently skips when no external oracle is installed (`ultimate_geography_fixture.rs:535-547`); no oracle is present in this checkout.

**Impact:** Brain Brew can be green while actual UG is non-canonical, unexportable, and unable to use the workbench. This is the strongest fixture-coupling failure found.

### H2 — High: the documented production Workbench is only a shell without a language catalog

UG documents `brainbrew workbench serve --manifest brainbrew.yaml` as a current translation UI (`CONTRIBUTING.md:185-199`), but neither real manifest contains `languages:` or `translation_profile:`. Runtime evidence:

```text
$ brainbrew workbench serve --manifest brainbrew.yaml --port 45671 --no-open
Workbench listening at http://127.0.0.1:45671

$ curl .../api/workspace
languages = {}
translation_profile = {all lists empty}

$ curl '.../api/workbench/note-list?language=de&target=standard...'
HTTP 400: unknown language "de"
```

The backend requires a configured target language and overlay (`BB/crates/brain-brew-cli/src/commands/workbench.rs:1294-1319`), and new-language preview requires a template language (`:815-825`). Brain Brew tests pass only because the fixture splices in 16 languages (`BB/fixtures/ultimate-geography/brainbrew.yaml:606-787`).

**Impact:** The advertised browser-based translator/maintainer journey is unavailable to the first production consumer.

### H3 — High: current parity evidence is representative, stale-path-dependent, and weaker than its claims

`docs/pr736-equivalence-evidence.md:40-51` covers only 12 of 100 configured targets: six main UG, three companion, and three standalone Hardcore. It omits all Danish, Hebrew, Italian, Norwegian, Dutch, Polish, Portuguese, Russian, Swedish, Chinese, and Traditional Chinese main parity except the selected German/French cases, and almost all Hardcore translations.

More importantly, the script's claimed “note model/template signature” is only `(model name, template count)` (`scripts/collect-pr736-equivalence-evidence.py:300-301`). It does **not** compare template names/order/HTML, CSS, field schema/order, deck configuration, or media contents. `compare_decks` checks note fields/GUIDs and only `name`, deck UUID, and description metadata (`:304-355`). Standalone Hardcore checks only unioned GUID/card counts (`:450-476`). The script also rewrites all `!include` directives into a materialized temporary tree (`:184-228`), despite native includes now working, and writes machine-specific paths into the report (`docs/pr736-equivalence-evidence.md:9-14`).

No target configures Brain Brew's stronger CrowdAnki golden support; real manifests contain no `exports.crowdanki.golden`, although the verifier supports it (`BB/crates/brain-brew-cli/src/commands/verify.rs:303-351`).

**Impact:** The report is useful GUID/note-content evidence, especially for the repaired Hardcore companion identity, but it cannot support “no unintended deck regressions” for the release matrix or templates/styles/media.

### H4 — High: release-strict translation verification cannot currently certify any translated target

UG intentionally defaults to lenient coverage (`CONTRIBUTING.md:201-205`). On a canonically formatted temporary copy:

```text
$ brainbrew verify --manifest brainbrew.yaml --target de-standard \
    --translation-coverage strict
translation coverage strict policy failed ... 1269 untranslated fallback(s)
```

The normal translator report separates 43 actionable missing strings from 1,226 hidden structural/media/tag fallbacks. The 43 visible German misses are structured message formats such as `{country_1} ({description_1})`. Strict mode, however, fails every `UntranslatedFallback`, including all hidden entries (`BB/crates/brain-brew-cli/src/commands/verify.rs:136-167`), so the documentation's suggested one-off release certification is not practically reachable without marking structural content explicitly.

The all-target summary also shows companion translation overlays with roughly 808-854 visible misses and 1,358+ hidden fallbacks per language because translation is split across multiple overlay stages. There are no stale/invalid keys, which is good, but “no stale keys” is much weaker than translation completeness.

**Impact:** There is no enforceable release-ready translation target today; maintainers must interpret lenient reports manually.

### H5 — High: migration removed two real maintainer workflows without replacement

Commit `4828fa512bb2` deletes the legacy CSV source, reverse recipes, and utilities: 38 files/4,450 lines, including `recipes/anki_to_source*.yaml`, `src/data/*.csv`, `utils/flag_similarity.py`, and `utils/zip_decks.py`. Current guidance replaces the multi-table content source with direct editing of a 7,154-line `deck.yaml` (`CONTRIBUTING.md:104-116`).

Two important losses are not covered by Brain Brew:

1. **Anki-to-source editing:** upstream docs explicitly supported editing English notes in Anki and pulling content into CSV. Current `brainbrew import crowdanki` only imports a complete folder into a new canonical file and requires `--accept-suggested-ids`; it does not reconcile into the existing deck/overlays (`BB/crates/brain-brew-cli/src/commands/import.rs:8-37`).
2. **Flag comparison:** the deleted 573-line utility calculated SVG geometry/color similarity and ΔE to support the domain workflow. Structured message fields improve storage but do not replace this analysis tool.

**Impact:** The migration narrows contributor workflows beyond “legacy recipe syntax”; it removes round-trip editing and a geography-specific maintenance tool. These need an explicit accepted deprecation or replacement before calling UG fully migrated.

### M1 — Medium: preserving Hardcore update identity requires substantial duplicated source/catalog wiring

The safe companion fix is valuable: 45 old HG note GUIDs are preserved and separate from standalone UG identity. But the price is visible duplication:

- `deck-hardcore.yaml:9-76` duplicates the entire UG note type from `deck.yaml:9-76`; only deck metadata and empty notes/media differ.
- Two manifests repeat the Hardcore, field-fill, translation, and variant catalog (`brainbrew.yaml:7-354`; `brainbrew-hardcore.yaml:5-281`).
- Twelve `companion-translations` files total 1,036 lines, twelve Hardcore translation files total 808 lines, and twelve note-type translation files add 222 lines.
- For German, Spanish, French, and Norwegian, every parsed source→target pair in the companion translation file (31/34/33/33 pairs respectively) is an exact duplicate of the main language dictionary. For example German `American Samoa`, `Autonomous region of Portugal.`, etc. appear both in `overlays/languages/de.yaml:24,38-45` and `overlays/extensions/hardcore/companion-translations/de.yaml:6-23`.
- Swedish needs a separate companion variant file (`brainbrew-hardcore.yaml:270-275`) to avoid the deck metadata overrides in `overlays/variants/extended/sv.yaml:3-19`.

**Impact:** A shared translation correction can require multiple files, and the two note-type shells can drift. This is an awkward workaround around preserving distinct adapter identity for conceptually overlapping notes.

### M2 — Medium: release packaging is manual, redundant, and internally inconsistent

Brain Brew already copies only declared media during export (`BB/crates/brain-brew-cli/src/commands/export.rs:99-101`), but UG CI and docs then run `cp media/*` (`.github/workflows/integrity-check.yml:43-45`; `CONTRIBUTING.md:72-73,87-88`). This masks declaration gaps and copies all 607 files (17,441,184 bytes) into every one of 100 outputs—about 1.62 GiB before ZIP compression—even for a 45-note Hardcore companion that natively needs 56 files.

The old `utils/zip_decks.py` derived `Ultimate_Geography_vX.Y_LANG[_EXTENDED].zip` and preserved display-name directories. The new release instructions merely say “Zip each exported CrowdAnki folder” (`CONTRIBUTING.md:493-496`), while the documented output folders are target IDs such as `build/crowdanki/en-standard` (`:66-90`) and no manifest target configures `exports.crowdanki.out` (`brainbrew.yaml:355-605`; supported at `BB/crates/brain-brew-formats/src/manifest.rs:245-250`). There is also no automated archive validation.

**Impact:** Release assembly is easy to misname/mispackage and unnecessarily expensive.

### M3 — Medium: release version text is duplicated and the release instructions point at the wrong source

The release checklist says to update the version in `deck.yaml` and `README.md` (`CONTRIBUTING.md:490-493`), but neither file contains the current `v5.3` literal. It lives independently in six description fragments:

- `descriptions/ultimate-geography/en.html:3`
- `es.html:3`
- `he.html:5`
- `sv.html:3`
- `zh.html:3`
- `zh-tw.html:3`

`deck.yaml:4` only includes the English fragment. There is no source variable for release version.

**Impact:** A release can silently ship mixed version labels across languages, and a maintainer following the checklist will not find the stated version field.

### M4 — Medium: CI duplicates native HTML/CSS validation with a weaker side script

`scripts/check-source-content.py:10-12,103-119` scans only hard-coded `descriptions`, `templates`, and `styles` directories. It passed 22 files, but it is maintained separately and cannot inspect inline/composed target content. Current Brain Brew `verify` already renders variables and validates every target's HTML/CSS unless explicitly skipped (`BB/crates/brain-brew-cli/src/commands/verify.rs:76-78,122-133`).

This duplication is already acknowledged in UG backlog, but today the stronger native gate never runs because formatting/media fail first.

## Positive implementation notes

- Repeated labels and note-type names are source variables rather than copied template text (`deck.yaml:11-21`), and translation overlays use `translations.variables` for them.
- Extended structure is genuinely shared; language overlays do not duplicate card template HTML.
- External includes reduce template/style duplication and the standalone source checker passes all 22 files.
- Hardcore uses one shared 45-note content overlay and default field fills; the companion shell preserves old deck/note GUID behavior instead of merging identities unsafely.
- Current media files and source references are internally exhaustive by filename: a static scan found exactly 607 referenced names and 607 files, with no missing/unreferenced filenames. The failure is declaration/hash state, not absent binaries.
- The PR evidence does prove exact note-field/GUID parity for six representative UG targets and documents intentional Hardcore deltas rather than hiding them.

## Recommended gate order

1. Pin a buildable Brain Brew artifact/commit and make Nix packaging stop running unprepared browser E2E tests.
2. Reconcile the migration branch with `master@upstream`; use immutable revisions in parity evidence.
3. Run current `brainbrew fmt` over actual UG, then migrate all 607 media references/declarations/hashes and remove manual `cp media/*`.
4. Move the language catalog/profile into the real consumer (or remove the unsupported Workbench claim); eliminate the fixture-only splice and resync fixture from actual UG.
5. Establish release goldens/oracles for all release targets, with exact template/CSS/schema/media identity checks and explicit allowlists for intentional differences.
6. Decide explicitly whether Anki-to-source, flag analysis, and automated ZIP packaging are required maintainer workflows; restore or document their deprecation.
7. Make release version metadata single-source and test every localized description.
8. Only then enable strict coverage for selected release-ready translations and run the complete two-manifest release/import smoke test.

## Reproduction command log

Commands were run from UG unless prefixed with `cd BB`:

```bash
# Repository state/history
jj status
jj log -r 'ancestors(@-, 30)' ...
jj log -r 'ancestors(master@upstream) ~ ancestors(@-)' ...
jj diff --from master@upstream --to @- --stat

# Installed/current binaries
command -v brainbrew
brainbrew --version
/etc/profiles/per-user/jmo/bin/brainbrew --version
/home/jmo/Development/projects/brain-brew/target/debug/brainbrew --version

# Source checks and target inventory
python scripts/check-source-content.py
brainbrew targets --manifest brainbrew.yaml
brainbrew targets --manifest brainbrew-hardcore.yaml

# Actual production gates (failed)
/etc/profiles/per-user/jmo/bin/brainbrew verify --manifest brainbrew.yaml --all-targets --media-root media
/home/jmo/Development/projects/brain-brew/target/debug/brainbrew verify --manifest brainbrew-hardcore.yaml --all-targets --media-root media
/home/jmo/Development/projects/brain-brew/target/debug/brainbrew export crowdanki --manifest brainbrew.yaml --target en-standard --out /tmp/ug-export-en-standard --media-root media

# Temporary-copy diagnosis; no repository file modified
find /tmp/ug-fmt-audit -name '*.yaml' -print0 | xargs -0 -n1 BB/target/debug/brainbrew fmt
BB/target/debug/brainbrew verify --manifest /tmp/ug-fmt-audit/brainbrew.yaml --all-targets --media-root UG/media
BB/target/debug/brainbrew verify --manifest /tmp/ug-fmt-audit2/brainbrew-hardcore.yaml --all-targets --media-root UG/media
BB/target/debug/brainbrew export crowdanki --manifest brainbrew-hardcore.yaml --target de-hardcore-companion-standard --out /tmp/ug-export-hc-de --media-root media

# Translation/workbench
BB/target/debug/brainbrew translations --manifest brainbrew.yaml --target de-standard
BB/target/debug/brainbrew translations --manifest brainbrew.yaml --all-targets --summary
BB/target/debug/brainbrew verify --manifest /tmp/ug-fmt-audit/brainbrew.yaml --target de-standard --translation-coverage strict
BB/target/debug/brainbrew workbench serve --manifest brainbrew.yaml --port 45671 --no-open
curl http://127.0.0.1:45671/api/workspace
curl 'http://127.0.0.1:45671/api/workbench/note-list?language=de&target=standard&limit=1'

# CI tool path (failed before UG execution)
nix run github:jeprecated/brain-brew/rust-brainbrew -- --version

# Fixture drift
cd BB
diff -qr --exclude=brainbrew.yaml --exclude=brainbrew-hardcore.yaml --exclude=media.yaml UG fixtures/ultimate-geography
rg -n 'fixtures/ultimate-geography' crates --glob '*.rs'
```

## Acceptance

```acceptance-report
{
  "criteriaSatisfied": [
    {
      "id": "criterion-1",
      "status": "satisfied",
      "evidence": "Review-only consumer audit completed without modifying either source repository; only audit/13-ultimate-geography.md was written as requested."
    },
    {
      "id": "criterion-2",
      "status": "satisfied",
      "evidence": "Findings include exact UG/Brain Brew file-line references, reproducible commands, observed pass/fail output, a 10-stage consumer journey map, and severity-ranked residual risks."
    }
  ],
  "changedFiles": [
    "audit/13-ultimate-geography.md"
  ],
  "testsAddedOrUpdated": [],
  "commandsRun": [
    {
      "command": "python scripts/check-source-content.py",
      "result": "passed",
      "summary": "Checked 22 external HTML/CSS source files."
    },
    {
      "command": "/etc/profiles/per-user/jmo/bin/brainbrew targets --manifest brainbrew.yaml; ... brainbrew-hardcore.yaml",
      "result": "passed",
      "summary": "Discovered 74 main and 26 Hardcore companion targets."
    },
    {
      "command": "/etc/profiles/per-user/jmo/bin/brainbrew verify --manifest brainbrew.yaml --all-targets --media-root media",
      "result": "failed",
      "summary": "Rejected deck.yaml as non-canonical."
    },
    {
      "command": "BB/target/debug/brainbrew verify --manifest brainbrew-hardcore.yaml --all-targets --media-root media",
      "result": "failed",
      "summary": "Rejected overlays/extensions/hardcore.yaml as non-canonical."
    },
    {
      "command": "format temporary copies, then verify both manifests",
      "result": "partial",
      "summary": "Hardcore verified all 26 targets; main failed on 546 empty base media hashes (and has five additional Experimental empty hashes)."
    },
    {
      "command": "BB/target/debug/brainbrew export crowdanki --manifest brainbrew.yaml --target en-standard --out /tmp/ug-export-en-standard --media-root media",
      "result": "failed",
      "summary": "Empty media hashes prevented deck.json creation."
    },
    {
      "command": "BB/target/debug/brainbrew export crowdanki --manifest brainbrew-hardcore.yaml --target de-hardcore-companion-standard --out /tmp/ug-export-hc-de --media-root media",
      "result": "passed",
      "summary": "Exported 45 notes, one note type, four templates, and 56 media assets."
    },
    {
      "command": "BB/target/debug/brainbrew translations --manifest brainbrew.yaml --target de-standard",
      "result": "passed",
      "summary": "Reported 43 actionable misses, 1,226 hidden fallbacks, and zero stale/invalid keys."
    },
    {
      "command": "BB/target/debug/brainbrew verify --manifest /tmp/ug-fmt-audit/brainbrew.yaml --target de-standard --translation-coverage strict",
      "result": "failed",
      "summary": "Strict policy rejected 1,269 untranslated fallbacks."
    },
    {
      "command": "BB/target/debug/brainbrew workbench serve ...; curl /api/workspace and /api/workbench/note-list",
      "result": "partial",
      "summary": "Server health passed, but workspace had no languages/profile and German note-list selection returned HTTP 400."
    },
    {
      "command": "nix run github:jeprecated/brain-brew/rust-brainbrew -- --version",
      "result": "failed",
      "summary": "Nix package build ran unprepared browser E2E tests; 13 failed because target/debug/brainbrew was absent."
    },
    {
      "command": "jj status/log/diff in both repositories; diff real UG against BB fixture",
      "result": "passed",
      "summary": "Confirmed divergent migration/upstream history, working-copy state, and substantial production-fixture drift."
    }
  ],
  "validationOutput": [
    "External HTML/CSS side check: 22 files passed.",
    "Target inventory: 74 + 26 = 100.",
    "Actual main verify/export: blocked by canonical formatting, then 546 base plus five Experimental blank hashes.",
    "Temporary canonical Hardcore verify: all 26 targets passed.",
    "Representative Hardcore export: 45 notes and 56 media assets passed.",
    "Workbench server health passed but production translation APIs are unconfigured.",
    "No source edits were made in UG or Brain Brew."
  ],
  "residualRisks": [
    "No Anki GUI import smoke test was possible in this non-interactive audit.",
    "The old-vs-new evidence generator was inspected but not rerun because it downloads/builds legacy dependencies and writes its report; existing evidence was instead validated against its implementation.",
    "All 100 exports were not attempted because the first main export deterministically fails on the same empty-hash gate and CI's Nix tool cannot build.",
    "UG migration must still reconcile six upstream-only commits and rerun parity from an immutable baseline."
  ],
  "noStagedFiles": true,
  "notes": "Jujutsu has no staging area. Pre-existing uncommitted audit/frontloop/cache files were observed and left untouched; this task added only the requested audit report."
}
```
