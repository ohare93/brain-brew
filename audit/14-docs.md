## Review

- Correct: The core product framing is coherent across `README.md:3-18`, `documentation/docs/intro.md:8-23`, and `documentation/docs/reference/project-scope.md:7-30`: local-first federation, no review-state storage, and UG as a general fixture rather than a special product mode.
- Correct: Package/lock functionality is consistently marked experimental in the welcome, scope, authoring, CLI, YAML, and lock reference pages (for example `documentation/docs/intro.md:23` and `documentation/docs/authoring/packages-locking.md:7`).
- Correct: The documented UG workflow is currently accurate. Both commands at `documentation/docs/examples/ultimate-geography.md:48-50` passed with the shown counts (74 and 26), and language-specific variant/translation files do not duplicate `card_templates`; this matches the variable-first/shared-extension guidance at `documentation/docs/examples/ultimate-geography.md:74-90`.
- Correct: The documentation site builds successfully and the repository-local Markdown link check found no missing targets.

### Ranked gaps

#### 1. Blocker — The recommended release/version story does not identify an installable binary that has the documented behavior

`README.md:42-69` and `documentation/docs/getting-started/install.md:9-72` present crates.io, a GitHub Release installer, Homebrew, and a Git tag as current normal install paths. In reality:

- crates.io `brainbrew 1.0.0-alpha.1` exists;
- the GitHub API returns 404 for both release `v1.0.0-alpha.1` and tag `v1.0.0-alpha.1`, so the shell/PowerShell installer URLs and tag-based Cargo command cannot work;
- `jeprecated/homebrew-tap/Formula` has no `brainbrew.rb`, so `brew install jeprecated/tap/brainbrew` cannot work.

This also contradicts itself inside the README: it calls the release installer the “easiest no-Rust install path” at `README.md:51-58`, then says Homebrew is available only “once the preview release is published” at `README.md:60-64`. `CHANGELOG.md:20` nevertheless claims that prebuilt archives, installers, and a formula shipped.

More seriously, the available crates.io alpha is older than the documentation. The downloaded published `brainbrew-1.0.0-alpha.1` source has neither `workbench` nor `media` commands and lacks current options such as `--skip-content-validation` and stale-translation `--resolve`; current docs advertise them at `documentation/docs/reference/cli.md:84-91,130-149`, `documentation/docs/reference/workbench.md:7-31`, and `documentation/docs/authoring/media.md:74-84`. The repository still reports the same alpha version (`Cargo.toml:11-21`) while `CHANGELOG.md:3-10` calls several documented semantics “Unreleased.” A user following the recommended pin therefore gets a different CLI/schema contract than the site describes.

**Required product/docs decision:** publish/tag the documented build, or version the docs and clearly label checkout-only/unreleased features. Until then, make Cargo alpha the only claimed released channel and remove/disable dead installer, tag, and Homebrew instructions.

#### 2. Blocker — The zero-to-first-deck quickstart is not executable as written

The promised first-run path at `documentation/docs/intro.md:34-39` points to `documentation/docs/getting-started/quickstart.md`. Reproducing that page from an empty temporary directory found three onboarding failures:

1. It asks users to create `overlays/languages/de.yaml` (`quickstart.md:50-61`) without creating its parent directories.
2. `brainbrew compose ... --out build/de-standard.yaml` (`quickstart.md:84-87`) fails with `No such file or directory`; compose writes directly and does not create parent directories (`crates/brain-brew-cli/src/commands/compose.rs:31-34`).
3. After creating `build/`, `brainbrew verify --all-targets` still fails because the pasted manifest and deck are not canonical. The manifest target keys are not formatter order (`quickstart.md:74-79`), and the deck has noncanonical field-map order/quoting/null-sequence spelling (`quickstart.md:23-47`). Verify checks exact canonical formatting before behavior (`crates/brain-brew-cli/src/commands/verify.rs:25-46`).

The page also says it builds “one target” (`quickstart.md:7`) but defines two (`quickstart.md:74-79`), and teaches `Paris: Paris` as a direct translation (`quickstart.md:57-60`) despite the translation guide reserving reviewed identity text for `translations.no_change` (`documentation/docs/authoring/translations.md:64-88`).

Add directory creation, use formatter-generated snippets, and execute this page in CI. This is currently the highest-impact onboarding failure after installation.

#### 3. Blocker — The CLI reference gives an import command that is guaranteed to fail

`documentation/docs/reference/cli.md:93-99` documents:

```bash
brainbrew import crowdanki build/crowdanki/en-standard --out deck.yaml
```

The implementation rejects any import without `--accept-suggested-ids` (`crates/brain-brew-cli/src/commands/import.rs:15-24`), and the actual built-in help correctly includes the flag (`crates/brain-brew-cli/src/help.rs:54-55`). Executing the reference command exited 1 with `non-interactive CrowdAnki import requires --accept-suggested-ids for now`.

The reference should use the required flag and explain its trust/review implication next to the command, rather than leaving that explanation only in `documentation/docs/concepts/media.md:59`.

#### 4. Note (high) — `fmt` include behavior is documented as the opposite of actual behavior

`documentation/docs/reference/yaml.md:220-222` says formatting a scalar `!include` “materializes the included scalar content into canonical YAML.” The formatter explicitly promises not to materialize includes and restores the directives after canonical ordering (`crates/brain-brew-formats/src/source_includes.rs:46-76`). A representative `brainbrew fmt` run preserved `description: !include content/description.md`.

This matters because `documentation/docs/authoring/workspace.md:81-87` recommends formatting every source as a review gate. Maintainers cannot tell whether that gate will preserve or destroy their file layout. Update the reference to distinguish read/compose resolution (inlines content in the resolved deck) from source formatting (preserves scalar and media includes).

#### 5. Note (high) — The “YAML format reference” is not complete enough to author or review the public schema

`documentation/docs/reference/yaml.md:7` calls itself compact, but it is the only schema reference and omits important accepted/public shapes:

- Canonical variables at deck, note-type, card-template, and note scopes (`brain-brew-core/src/model.rs:676-685,1089-1127`) are not represented in the deck shape at `reference/yaml.md:9-29`.
- The overlay section lists top-level buckets (`reference/yaml.md:31-47`) but does not specify the sparse schemas for deck, note type, card template, field, tag, adapter-ID, or media changes, nor the `expected_base: entity_present` form used for removals (`brain-brew-core/src/model.rs:982-1085`; emitter at `crates/brain-brew-formats/src/canonical_yaml.rs:1194-1198`).
- The manifest block (`reference/yaml.md:224-244`) omits `package.compatible_base_versions`, `languages`, `translation_profile`, export defaults/goldens, target `extends`, and overlay dependencies, even though these are public fields (`crates/brain-brew-formats/src/manifest.rs:159-220,235-253`). Some are described in authoring prose, but there is no single field/type/default/constraint reference.
- No docs explain stable-ID reserved path grammar, even though public path parsing rejects reserved markers/suffixes (`crates/brain-brew-core/src/model.rs:60-74`).

This gap is amplified by strict unknown-field rejection. Provide a complete schema table or generated schema/reference, including required/optional/default values and examples for every intent.

#### 6. Note (high) — Upgrade and recovery guidance does not cover the actual unreleased compatibility changes

There is no version-to-version migration page despite docs pinning alpha. The only release notes classify changed overlay fill semantics, stricter import rejection, key emission, path-lock behavior, and JSON errors as unreleased (`CHANGELOG.md:3-10`), while the site describes those changes as current. In particular, the new field-operation rule can invalidate existing overlays, but no migration recipe identifies affected syntax or replacement syntax.

Two implemented recovery paths are also missing or misleading:

- Stale-translation prose tells users to move/delete records manually (`documentation/docs/authoring/translations.md:163-165`), while the current CLI supports `--resolve confirm|replace`, `--old-source`, `--new-source`, `--stale-context`, and `--translation` (`crates/brain-brew-cli/src/commands/translations.rs:297-330`; built-in help at `crates/brain-brew-cli/src/help.rs:66-67`). The compact CLI reference at `documentation/docs/reference/cli.md:120-128` does not mention them.
- “When upstream changes,” package docs tell users to run `lock verify` first (`documentation/docs/authoring/packages-locking.md:76-86`). For a Git lock this continues verifying the old pinned revision and does not discover branch movement; for a changed path lock it fails on the old hash (`crates/brain-brew-cli/src/commands/lock.rs:94-108,411-454`). Accepting an upstream update requires rerunning `lock update` with the chosen source/ref, reviewing the lock diff, then verifying. That recovery/update sequence is absent.

Add a versioned migration guide and error-to-recovery table before changing the promised alpha contract.

#### 7. Note (high) — Workbench documentation describes legacy pivot APIs, not the APIs the browser now uses

`documentation/docs/reference/workbench.md:50-122` names aggregate endpoints such as `note-pivot`, `source-string-pivot`, and `card-pivot`, but omits the current lazy list/detail endpoints registered at `crates/brain-brew-cli/src/commands/workbench.rs:191-220`: `note-list`, `note-detail`, `card-list`, `source-string-list`, `metadata-list`, and `optional-metadata-list`. The current UI calls these endpoints (`crates/brain-brew-workbench-ui/src/lib.rs:4038-4061,4084-4102,4152-4169`), and the server exposes filter/content-group/status plus `limit`/`offset` contracts with 50 default and 200 maximum (`workbench.rs:1465-1596`). None of those request fields, response shapes, limits, aliases, or API error status rules is documented.

The page mixes end-user operation, internal development commands, architecture aspirations, and an implicit public JSON API without saying which parts are stable. It also presents Workbench as released (`reference/workbench.md:7-13`) even though the pinned crates.io alpha does not contain the command and the maintained-surface list omits it (`documentation/docs/reference/project-scope.md:9-21`). Mark its maturity explicitly and either document the API contract or label it internal/unstable.

#### 8. Note (medium) — Package compatibility metadata is public, used by UG, undocumented, and not enforced

The UG manifest declares `package.compatible_base_versions`, and the format API parses/emits it (`crates/brain-brew-formats/src/manifest.rs:159-166,288-296`), but `rg compatible_base_versions README.md documentation/docs` returns no documentation. Dependency validation only splits `depends_on` at `@` and compares the remainder for literal equality (`crates/brain-brew-cli/src/package_resolver.rs:34-58`); it never consults `compatible_base_versions`.

That makes a semver-looking value such as `>=0.1,<0.2` metadata with no explained consumer or enforcement. The experimental labels are good, but experimental users still need accurate semantics. Document it as inert metadata or implement/describe enforcement; also state that `depends_on` currently accepts exact literal versions rather than ranges.

#### 9. Note (medium) — README promises an edit/export onboarding loop that does not exist

`README.md:79` explicitly sends users to `getting-started/install.md` for “an edit/export loop.” That page ends after install, Nix, developer shell, and runtime dependency sections (`documentation/docs/getting-started/install.md:80-126`) and contains no workspace checkout, edit, verify, or export exercise. Combined with the broken quickstart, there is no tested path from installed binary to editing an existing deck and inspecting/importing the exported result.

Add either the promised loop (preferably against `fixtures/ug-style`) or remove the claim and link directly to a tested quickstart.

#### 10. Note (medium) — Public Rust crates are advertised as reusable without consumer documentation

The README calls `brain-brew-core` and `brain-brew-formats` reusable (`README.md:9-11,83-87`), and both are published packages, but the site has no Rust dependency/usage examples for `CanonicalDeck`, validation/composition, YAML codecs, CrowdAnki import/export, or include context. The only library symbol named in user docs is `import_deck_accept_suggested_ids` at `documentation/docs/concepts/media.md:59`. The modules are public (`crates/brain-brew-formats/src/lib.rs:7-14`) and core exports its entire model (`crates/brain-brew-core/src/lib.rs:10-12`), so this is undocumented public behavior, not merely CLI detail.

At minimum, link docs.rs and provide one supported API example plus a stability statement; otherwise describe these crates as internal implementation packages rather than a supported reusable surface.

### Residual observations

- `plan.md` and `progress.md`, requested as initial inputs, do not exist in the working directory; this did not block inspection.
- No broken local Markdown links were found, and `npm run build` passed. Docusaurus validates presentation/links but does not execute command snippets, which is why the quickstart and import regressions escaped.
- `brainbrew media images-to-refs` is a useful migration command and is documented at `documentation/docs/authoring/media.md:74-84`, but it rewrites files in place with no dry-run (`crates/brain-brew-cli/src/commands/media.rs:89-116`). Migration guidance should recommend a clean VCS state and review of the resulting diff.

```acceptance-report
{
  "criteriaSatisfied": [
    {
      "id": "criterion-1",
      "status": "satisfied",
      "evidence": "Performed a review-only documentation/product audit and wrote only audit/14-docs.md; no product or documentation source was edited."
    },
    {
      "id": "criterion-2",
      "status": "satisfied",
      "evidence": "Findings include exact documentation and implementation references plus reproduced command output for install availability, quickstart failures, import failure, include formatting, docs build, and both UG verification workflows."
    }
  ],
  "changedFiles": [
    "audit/14-docs.md"
  ],
  "testsAddedOrUpdated": [],
  "commandsRun": [
    {
      "command": "jj status && jj diff --stat",
      "result": "passed",
      "summary": "Confirmed the pre-existing audit files and later this review output; no product source edits were made."
    },
    {
      "command": "target/debug/brainbrew targets/compose/verify against a /tmp workspace copied from getting-started/quickstart.md",
      "result": "failed",
      "summary": "Targets listed, then compose failed because build/ did not exist; after creating it, verify failed on noncanonical manifest/deck snippets."
    },
    {
      "command": "target/debug/brainbrew import crowdanki fixtures/ug-style/goldens/en-standard --out /tmp/doc-import.yaml",
      "result": "failed",
      "summary": "Exited 1 with the documented command because --accept-suggested-ids is mandatory."
    },
    {
      "command": "target/debug/brainbrew fmt /tmp/brainbrew-doc-audit-include/deck.yaml",
      "result": "passed",
      "summary": "Formatting preserved description: !include, disproving reference/yaml.md's materialization claim."
    },
    {
      "command": "target/debug/brainbrew verify --manifest fixtures/ultimate-geography/brainbrew.yaml --all-targets",
      "result": "passed",
      "summary": "Verified 74 targets, matching the UG docs."
    },
    {
      "command": "target/debug/brainbrew verify --manifest fixtures/ultimate-geography/brainbrew-hardcore.yaml --all-targets",
      "result": "passed",
      "summary": "Verified 26 targets, matching the Hardcore companion docs."
    },
    {
      "command": "cd documentation && npm run build",
      "result": "passed",
      "summary": "Docusaurus production build completed successfully."
    },
    {
      "command": "Python local Markdown-link scan over documentation/docs/**/*.md",
      "result": "passed",
      "summary": "No missing repository-local Markdown targets found."
    },
    {
      "command": "GitHub/crates.io/Homebrew HTTP API checks for v1.0.0-alpha.1",
      "result": "failed",
      "summary": "crates.io package exists, but GitHub release/tag and installer asset return 404 and the Homebrew tap has no brainbrew formula."
    },
    {
      "command": "download and inspect static.crates.io brainbrew-1.0.0-alpha.1.crate",
      "result": "passed",
      "summary": "Published alpha help lacks current documented workbench/media commands and newer options."
    },
    {
      "command": "cargo doc --workspace --no-deps",
      "result": "passed",
      "summary": "Workspace Rust documentation generated successfully; this does not supply end-user API examples or a stability contract."
    }
  ],
  "validationOutput": [
    "Quickstart compose: build/de-standard.yaml: No such file or directory (os error 2)",
    "Quickstart verify after mkdir: brainbrew.yaml is not in canonical format; after formatting manifest: ./deck.yaml is not in canonical format",
    "Import reference: non-interactive CrowdAnki import requires --accept-suggested-ids for now",
    "UG main: verified 74 targets",
    "UG Hardcore: verified 26 targets",
    "Docusaurus: Generated static files in build",
    "Release checks: crates.io alpha HTTP 200; GitHub release/tag/installer HTTP 404; no Homebrew brainbrew.rb"
  ],
  "residualRisks": [
    "External release-channel availability was checked live on 2026-07-09 and can change after this audit.",
    "The full Workbench browser E2E suite was not rerun; this review compared docs to routes, query structs, and current UI call sites.",
    "No real remote package lock was updated because that would mutate source/lock state; lock recovery findings are based on verified implementation control flow."
  ],
  "noStagedFiles": true,
  "notes": "Review only. plan.md and progress.md were requested but absent. Other audit/*.md working-copy additions predated this task and were not modified."
}
```
