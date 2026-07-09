# Federated Deck overlays, translations, variables, and field fills audit

## Review

- **Correct:** The migrated UG workspace follows the shared-extension pattern for card structure: `overlays/variants/extended.yaml:1-27` adds the two templates once, while language residues such as `overlays/variants/extended/da.yaml:1-11` only preserve adapter identity. The base templates use source variables rather than copied localized HTML (`deck.yaml:11-20`, `templates/ultimate-geography/country-flag/answer.html:6-8`).
- **Correct:** `field_fills` lowers to checked replacements with an empty expected base (`crates/brain-brew-formats/src/canonical_yaml.rs:1611-1661`), and the core rejects a fill once upstream is nonblank (`crates/brain-brew-core/src/compose.rs:997-1056`). UG uses that shorthand appropriately for extension-owned blank content (`overlays/extensions/hardcore/field-fills.yaml:1-29`).
- **Correct:** Direct/contextual/no-change keys are stale-checked, contextual matching is boundary-aware, and stale records remain explicit review debt (`crates/brain-brew-core/src/translation.rs:240-309`, `1268-1366`, `1748-1752`, `1804-1821`). Core/formats tests passed, including overlay compose, translation coverage, field-fill shorthand, and UG fixture suites.
- **Note:** The requested `plan.md` and `progress.md` do not exist at the project root; this review used the skill, docs, implementation, tests, checked fixtures, and real UG workspace directly.

## Ranked findings

### 1. Blocker — translating a field definition can silently break every card template that references it

**Evidence.** Translation extraction explicitly treats field-definition names as translatable (`crates/brain-brew-core/src/translation.rs:73-82`), and composition mutates those names (`crates/brain-brew-core/src/translation.rs:1167-1176`). It translates template names and template variables, but never rewrites Anki `{{Field name}}` references in `question_format` or `answer_format` (`crates/brain-brew-core/src/translation.rs:1178-1203`). Content validation only parses templates as HTML fragments and does not validate Mustache references against the final field names (`crates/brain-brew-core/src/content_validation.rs:93-125`). This conflicts with the documented translation-profile concept, which explicitly presents field labels as translator-reviewable metadata (`documentation/docs/authoring/manifests-targets.md:83-89`).

A minimal export using the checked `fixtures/ug-style/deck.yaml` and `translations.direct: { Capital: Hauptstadt }` succeeded, but emitted a field named `Hauptstadt` while retaining `{{Capital}}` in two templates. There was no diagnostic.

**Consumer impact.** A translator following the exposed coverage report can produce an export whose cards are blank or invalid in Anki. `verify` and `export` both accept it. UG currently avoids the failure by leaving field names untranslated and translating visible labels through variables, but strict coverage then treats those field names as missing work.

**Required direction.** Either field-definition names must be structural/non-translatable by default, or translation must atomically rewrite and validate all Anki field references (including conditionals, type answers, and any other supported syntax). Add a regression test that translates `Capital` and proves exported template references remain valid.

### 2. High — UG's localized Hardcore targets apply content translation before adding the content, leaving the extension and fills in English

**Evidence.** A main-workspace Hardcore translation depends on the base language translation and then the extension (`brainbrew.yaml:55-60`). Dependency expansion therefore produces base translation → `overlay.extension.hardcore` → the adapter-ID-only Hardcore translation (`overlays/extensions/hardcore/translations/cs.yaml:1-12`). The target then applies English `field_fills` afterward (`brainbrew.yaml:367-370`; values at `overlays/extensions/hardcore/field-fills.yaml:22-29`). Core composition applies overlays strictly in expanded order (`crates/brain-brew-core/src/compose.rs:76-115`).

The translations that should localize those extension notes do exist, for example Czech country/content entries at `overlays/extensions/hardcore/companion-translations/cs.yaml:1-15`, but the main `brainbrew.yaml` neither catalogs nor selects that overlay. The separate companion manifest demonstrates the correct sequencing by making it depend on field fills (`brainbrew-hardcore.yaml:70-79`).

A fresh export of `cs-hardcore-standard` confirmed the duplicate Hardcore American Samoa note remains:

- `American Samoa`
- `Unincorporated territory of the United States.`
- `Pago Pago`

while only its GUID and note-type identity are localized. The translation summary likewise reported 2,202 untranslated fallbacks for `overlay.translation.hardcore.cs`.

**Consumer impact.** Every non-English standalone Hardcore target can ship visibly mixed-language extension notes; Czech, Italian, Dutch, Polish, Portuguese, Russian, Swedish, and Chinese also retain English fill prose such as “Not a sovereign country” and “The capital is shared…”. Localized fill override files mitigate only German, Spanish, French, and Norwegian fill values, not the unselected extension-note translations.

**Required direction.** Reuse the companion content translation overlays in the main manifest, or merge their dictionaries into a translation overlay that runs after both the shared extension and fills. Verify representative overlapping notes, not only note/GUID counts.

### 3. High — duplicated hint variables have already produced mixed-language card faces in 13 UG languages

**Evidence.** The base defines two variables with the identical source (`deck.yaml:14-15`) and uses them independently on question and answer (`templates/ultimate-geography/capital-country/question.html:8`, `answer.html:8`). Except for Hebrew, overlays translate only `label.capital-hint.answer`; Czech is representative (`overlays/languages/cs.yaml:712-724`) and Danish is the same (`overlays/languages/da.yaml:695-705`). Because variable translations are keyed by the exact variable key before direct fallback (`crates/brain-brew-core/src/translation.rs:259-267`), translating the answer key does nothing for the question key.

The current Danish export contains `Hint: {{Capital hint}}` on the question and `Ledetråd: {{Capital hint}}` on the answer; Czech similarly mixes `Hint` and `Náznak`. Hebrew alone duplicates the same target under both keys (`overlays/languages/he.yaml:668-672`). Norwegian translates neither.

**Consumer impact.** Standard, Extended, Experimental, and Hardcore outputs expose English UI text on one face despite having the target translation. The duplicated key also forces every translator to maintain two entries that are identical in almost every language.

**Required direction.** Use one shared `label.capital-hint` variable unless a demonstrated language needs side-specific wording; side-specific languages can override at template scope. At minimum, add a parity/coverage guard requiring both keys when both remain in source.

### 4. High — strict translation coverage is not compositional for split translation overlays

**Evidence.** Coverage extracts the entire current deck for each translation overlay (`crates/brain-brew-core/src/translation.rs:48-154`). Verification checks each report independently and fails on any fallback (`crates/brain-brew-cli/src/commands/verify.rs:136-168`), then composes that overlay before checking the next (`crates/brain-brew-cli/src/commands/verify.rs:182-187`). Consequently, an extension-specific translation overlay is judged responsible for all already-translated base text plus every unrelated string in the current deck.

On the real `cs-hardcore-standard` stack, summary output was:

- base Czech overlay: 1,269 total fallbacks, 43 actionable text fallbacks;
- extension adapter overlay: 2,202 total fallbacks, 844 actionable text fallbacks.

No UG target declares `translation_coverage: strict`, so normal all-target verification cannot catch findings 2 or 3 (`brainbrew.yaml:355-605`).

**Consumer impact.** Maintainers cannot make a release gate strict while keeping translations split by base/extension responsibility. They must either ignore broad unrelated paths in every overlay, merge all dictionaries, or remain lenient. This undermines the documented language-specific extension-overlay workflow.

**Required direction.** Define strict completeness for the target's combined translation stack (or attribute source units to the overlay that introduces them), rather than requiring every translation overlay to cover the whole intermediate deck.

### 5. Medium — blank direct translations erase content globally while being reported as successfully translated

**Evidence.** The model describes direct keys as replacements for exact non-empty source text (`crates/brain-brew-core/src/model.rs:700-710`), while docs reserve intentional divergence/supplementation for path-checked target adaptations (`documentation/docs/authoring/translations.md:11-21`). The implementation accepts an empty target, categorizes it as `DirectTranslation` without qualification (`crates/brain-brew-core/src/translation.rs:510-519`), and applies it as an ordinary replacement (`crates/brain-brew-core/src/translation.rs:1507-1511`, `1549-1558`).

The real Danish dictionary relies on this to delete multiple source sentences globally (`overlays/languages/da.yaml:15-23`).

**Consumer impact.** An accidentally blank YAML value silently deletes every occurrence and satisfies strict coverage; there is no path, reason, or explicit deletion intent to distinguish an intentional omission from unfinished translator input. Because `direct` is global, a later reuse of that sentence is also erased.

**Required direction.** Reject blank direct/contextual/variable targets, or represent deletions as exact-path target adaptations with an expected source and optional reason. If blank faithful translations are intentionally supported, surface a distinct review category and warning.

### 6. Medium — the YAML codec contains an undocumented UG-specific translation subtype

**Evidence.** The generic formats crate hard-codes `"target addition from upstream UG"` (`crates/brain-brew-formats/src/canonical_yaml.rs:20`). Undocumented `translations.target_additions` values are converted into normal target adaptations carrying that magic reason (`crates/brain-brew-formats/src/canonical_yaml.rs:1754-1771`), then the formatter switches them back to the hidden shape only when that exact reason matches (`crates/brain-brew-formats/src/canonical_yaml.rs:730-744`). The public translation docs list only direct, contextual, no-change, target adaptations, and stale translations, and say top-level adaptations are emitted after the dictionary (`documentation/docs/authoring/translations.md:11-21`, `283-295`). Real UG language files depend on the hidden syntax (`overlays/languages/da.yaml:687-694`).

**Consumer impact.** Core/API consumers see a product-branded synthetic reason that source authors never wrote. Changing or normalizing that reason changes the canonical YAML shape. Other deck authors cannot discover when to use this shorthand, and Brain Brew now has UG-specific behavior despite the stated general-purpose boundary.

**Required direction.** Remove the alias after migrating UG to documented `target_adaptations`, or promote a product-neutral, documented `target_additions` concept with explicit stable semantics and no magic reason string.

### 7. Medium — the actual UG workspace currently fails its documented verification gate and lags the checked fixture's authoring model

**Evidence.** `brainbrew verify --manifest .../ultimate-geography/brainbrew.yaml --all-targets` stops immediately with `deck.yaml is not in canonical format`. Formatting a temporary copy showed the first delta at `deck.yaml:89-91` (unnecessary quoting of tags), followed by many equivalent deltas. The real manifest also lacks the `languages:`/`translation_profile:` metadata and any strict target policy through its end at `brainbrew.yaml:605`, while the checked fixture has already adopted those controls.

**Consumer impact.** Contributors cannot run the skill/docs-prescribed all-target verification successfully against canonical source, and language-first tooling has to infer meaning from the large target/overlay naming convention. More importantly, a green fixture test does not establish that the actual consumer workspace is ready.

**Required direction.** Canonically format the real workspace under its pinned Brain Brew version, adopt or explicitly reject the language catalog/profile, and run actual-workspace verification in CI. Keep fixture sync structural rather than maintaining a feature delta.

### 8. Low — deck and note-type display identity use inconsistent variable strategies

**Evidence.** The note-type name is variable-first (`deck.yaml:11-20`), but deck name/config name remain hardcoded (`deck.yaml:3-7`). Spanish then repeats the language suffix in both a `note-type.name` variable and a separate checked deck-name replacement (`overlays/languages/es.yaml:667-670`, `995-1005`); several other languages translate only the note-type name, so exported deck names and model names intentionally diverge.

**Consumer impact.** Renames/suffix changes require two mechanisms and can drift. Current outputs already differ by language: e.g. Czech exports deck `Ultimate Geography` with model `Ultimate Geography [CS]`, while Spanish exports both with `[ES]`.

**Required direction.** Decide whether deck and note-type identities are intentionally distinct. If not, introduce deck-level name/suffix variables and translate those rather than mixing variable translation with sparse property replacement.

## Validation notes

- `devenv shell -- cargo test -p brain-brew-core -p brain-brew-formats` passed all core/formats tests, including 55 overlay-compose tests, 21 overlay-YAML tests, and 18 UG fixture tests.
- Actual UG `verify --all-targets` failed on canonical formatting before composition.
- A fresh actual-workspace `cs-hardcore-standard` export succeeded and reproduced the untranslated extension note.
- A temporary minimal export reproduced the translated-field/broken-template-reference defect.
- No project/source files were modified; only this requested audit report was written.

```acceptance-report
{
  "criteriaSatisfied": [
    {
      "id": "criterion-1",
      "status": "satisfied",
      "evidence": "Review-only audit stayed within overlay/translation/source-variable/field-fill behavior and wrote only audit/03-overlays-translations.md."
    },
    {
      "id": "criterion-2",
      "status": "satisfied",
      "evidence": "Eight ranked findings include implementation, documentation, fixture, and real Ultimate Geography file:line evidence plus reproduced export/coverage behavior."
    }
  ],
  "changedFiles": [
    "audit/03-overlays-translations.md"
  ],
  "testsAddedOrUpdated": [],
  "commandsRun": [
    {
      "command": "devenv shell -- cargo test -p brain-brew-core -p brain-brew-formats",
      "result": "passed",
      "summary": "All core and formats unit/integration/doc tests passed, including UG fixture and overlay/translation suites."
    },
    {
      "command": "devenv shell -- cargo run -q -p brainbrew -- verify --manifest /home/jmo/Development/external/ultimate-geography/brainbrew.yaml --all-targets",
      "result": "failed",
      "summary": "Actual UG verification stopped because deck.yaml is not in canonical format."
    },
    {
      "command": "devenv shell -- cargo run -q -p brainbrew -- translations --manifest /home/jmo/Development/external/ultimate-geography/brainbrew.yaml --target cs-hardcore-standard --summary --json",
      "result": "passed",
      "summary": "Reported 1,269 fallbacks for the base Czech overlay and 2,202 for the Hardcore translation overlay."
    },
    {
      "command": "devenv shell -- cargo run -q -p brainbrew -- export crowdanki --manifest /home/jmo/Development/external/ultimate-geography/brainbrew.yaml --target cs-hardcore-standard --out /tmp/cs-hc-current",
      "result": "passed",
      "summary": "Fresh export showed the overlapping Hardcore American Samoa note and its country-info remained English."
    },
    {
      "command": "temporary ug-style export with translations.direct Capital: Hauptstadt",
      "result": "passed",
      "summary": "Export emitted field Hauptstadt while templates still referenced {{Capital}}, reproducing the semantic defect."
    }
  ],
  "validationOutput": [
    "Core/formats test suites passed.",
    "Actual UG all-target verify: deck.yaml is not in canonical format.",
    "Czech Hardcore fresh export: extension-owned American Samoa fields remained English.",
    "Translated field-name reproduction: final field Hauptstadt with unchanged {{Capital}} template references."
  ],
  "residualRisks": [
    "The actual UG verify failure prevents an end-to-end green all-target validation of the external workspace.",
    "Only Czech was freshly exported for the dependency-order reproduction; manifest structure shows the same ordering for the other localized Hardcore targets.",
    "No source fixes or regression tests were applied because the task was explicitly review-only."
  ],
  "noStagedFiles": true,
  "notes": "plan.md and progress.md were absent. Jujutsu has no staging area; the working copy contains this and other requested audit reports, with no Brain Brew source changes from this review."
}
```
