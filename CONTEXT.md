# Brain Brew

Brain Brew exists to help flashcard deck maintainers compose, evolve, and redistribute decks without losing the structure or history that makes those decks useful.

## Language

**Brain Brew**:
A local-first deck federation tool for flashcard decks.
_Avoid_: universal note sync service, SaaS sync platform

**Deck**:
A shareable flashcard collection including notes, note types, card templates, styling, metadata, and media references.
_Avoid_: note list, CSV file, Anki export

**Deck Maintainer**:
A person responsible for evolving and publishing a shared flashcard deck.
_Avoid_: learner, reviewer, end user

**Deck Workbench**:
An ergonomic authoring and review interface for working with a Federated Deck workspace without treating the interface as canonical storage.
_Avoid_: web app source of truth, generated HTML report, translator-only app

**Learner**:
A person who studies with a shared deck and may have private changes to preserve.
_Avoid_: deck maintainer, publisher

**Shared Deck**:
A flashcard deck intended to be installed, updated, or extended by people other than its maintainer.
_Avoid_: personal deck, private notes

**Federated Deck**:
A composable source package for a shared deck contribution, containing a base deck, overlays, or both, intended to be composed with other Federated Decks.
_Avoid_: Anki subdeck, resolved deck, full deck copy

**Canonical Deck**:
The format-independent representation of a deck's notes, note types, card templates, styling, metadata, and media references.
_Avoid_: canonical note list, CrowdAnki JSON

**Canonical Deck File**:
The maintainer-owned source file containing a canonical deck.
_Avoid_: generated artifact, adapter export

**Note**:
A deck entity containing field values and tags for one learnable fact or item.
_Avoid_: card, row

**Note Type**:
A deck entity that defines the fields and card templates shared by a group of notes.
_Avoid_: template, card type

**Card Template**:
A deck entity that defines how a note becomes a study card.
_Avoid_: note type, card

**Card**:
A study item produced from a note through a card template.
_Avoid_: note, template

**Review History**:
A learner's scheduling and study progress for cards in a spaced repetition system.
_Avoid_: deck content, note metadata

**Media Asset**:
An external file used by deck content or presentation.
_Avoid_: embedded YAML data, field text

**Media Reference**:
A deck entity that identifies and verifies a media asset.
_Avoid_: raw media file, HTML snippet

**Deck Entity**:
An identifiable part of a deck, such as a note, note type, card template, media reference, or deck metadata item.
_Avoid_: file, row, JSON object

**Stable ID**:
A human-readable identifier that says a deck entity is the same entity across source files, overlays, exports, and releases.
_Avoid_: content hash, row number, display name, adapter GUID

**Adapter ID**:
An identifier used by an external deck format or tool for the same deck entity.
_Avoid_: stable ID, content hash

**Suggested Stable ID**:
A proposed stable ID generated during import that must be accepted or corrected before becoming canonical.
_Avoid_: stable ID, adapter ID

**Content Hash**:
A fingerprint of deck content used to detect changes, not to identify entities.
_Avoid_: note ID, canonical ID

**Overlay**:
A bounded set of changes applied to a base deck without replacing the base deck.
_Avoid_: fork, duplicate deck

**Source Language**:
The language of the base deck text used as the reference for translation work.
_Avoid_: hard-coded English assumption, target language

**Target Language**:
A language produced from the source language by a Translation Overlay.
_Avoid_: separate translated deck, independent deck language

**Translation Overlay**:
An overlay that changes deck language or localized text.
_Avoid_: separate translated deck

**Extension Overlay**:
An overlay that adds new deck content or structure.
_Avoid_: patch, translation

**Patch Overlay**:
An overlay that corrects or adjusts existing deck content or structure.
_Avoid_: extension, fork

**Personal Overlay**:
An overlay containing learner-specific deck content or structure that should survive shared deck updates.
_Avoid_: upstream deck, maintainer patch, study state

**Overlay Fragment**:
The sparse deck-shaped content of an overlay, containing only the deck entities and properties the overlay changes.
_Avoid_: full deck copy, command script

**Source Variable**:
A named text value defined on a deck, note type, card template, or note and referenced from source text with `${variable.name}` before adapter export.
_Avoid_: recipe variable, runtime Anki field

**Translation Dictionary**:
A translation overlay section for faithful target-language text: direct source text, contextual source text, explicit no-change decisions for reviewed unchanged text, source variables, and adapter IDs, with non-blank source keys acting as implicit expected bases.
_Avoid_: CSV importer, global localization database, target adaptation storage

**Target Adaptation**:
A path-scoped target-language value that intentionally diverges from, explains, or supplements the source while declaring the expected source value it adapts.
_Avoid_: faithful translation, extension field fill

**Stale Translation**:
A translation review item that applies a prior target text to changed source text while warning that the translation needs review.
_Avoid_: stale key only, automatic silent migration

**Field Fill**:
An overlay shorthand for filling existing blank note fields with new content while requiring the upstream field to still be blank.
_Avoid_: translation addition, field definition addition

**Change Intent**:
The declared meaning of an overlay change, such as add, merge, replace, remove, or override.
_Avoid_: implicit overwrite, accidental merge

**Tombstone**:
A record that a deck entity was deliberately removed.
_Avoid_: missing data, accidental deletion

**Expected Base**:
The prior deck value or fingerprint an overlay declares before making a destructive or conflict-resolving change.
_Avoid_: current value guess, unchecked overwrite

**Overlay Stack**:
An ordered set of overlays applied to a base deck.
_Avoid_: unordered overlay set, dependency graph

**Overlay Catalog**:
A named collection of overlays available in a Federated Deck.
_Avoid_: raw file list, package solver

**Language Catalog**:
A named collection of source and target languages available in a Federated Deck, connecting target languages to their Translation Overlays and Build Targets.
_Avoid_: target naming convention, hard-coded language list

**Overlay Dependency**:
A requirement that one overlay include and apply another overlay before itself.
_Avoid_: implicit conflict resolution, automatic content merge

**Build Target**:
A named composition goal that resolves a base deck and selected overlays into a Resolved Deck.
_Avoid_: Anki export, source file, recipe step

**Resolved Deck**:
The deck produced by applying an overlay stack to a base deck.
_Avoid_: build artifact, export format

**Compose**:
To produce a resolved deck by applying an overlay stack to a base deck.
_Avoid_: build, export

**Semantic Diff**:
A comparison of decks by stable IDs and deck entities rather than raw source lines.
_Avoid_: text diff, file diff

**Federation Conflict**:
A situation where overlays make incompatible changes to the same deck entity.
_Avoid_: validation warning, last write wins

**Deck Federation**:
The composition of a base deck with translations, extensions, patches, or personal overlays without copying the whole deck.
_Avoid_: fork, duplicate deck, one-off conversion

**Canonicalized Source**:
A source representation after Brain Brew has applied its deterministic formatting rules.
_Avoid_: arbitrary original bytes, hand-formatted source

**Adapter Equivalence**:
A test projection that compares the supported data shared by Canonical Deck and a distributable adapter format.
_Avoid_: promise of an Anki-to-source merge or source ownership recovery

## Relationships

- **Brain Brew** primarily serves **Deck Maintainers**.
- A **Deck Maintainer** publishes one or more **Shared Decks**.
- A **Deck Maintainer** may publish a **Federated Deck** as a composable shared-deck source package.
- A **Deck Workbench** helps people review or author a **Federated Deck** while preserving source files as canonical storage.
- A **Federated Deck** may contribute a base **Deck**, **Overlays**, or both.
- **Federated Decks** compose through **Deck Federation** to produce **Resolved Decks**.
- A **Learner** studies a **Shared Deck** and may have a **Personal Overlay**.
- **Brain Brew** works on **Decks**.
- A **Canonical Deck** represents one **Deck** without binding it to a source or distribution format.
- A **Canonical Deck File** is the source of truth for a **Canonical Deck**.
- A **Deck** contains **Deck Entities**.
- A **Note** belongs to one **Note Type**.
- A **Note Type** has one or more **Card Templates**.
- A **Card** is produced from one **Note** and one **Card Template**.
- **Review History** is preserved by stable identity, not stored as **Deck** content.
- A **Media Reference** points to one **Media Asset**.
- A **Stable ID** identifies a **Deck Entity** across source, overlays, exports, and bootstrap imports.
- An **Adapter ID** preserves identity in a specific external format or tool.
- A **Suggested Stable ID** becomes a **Stable ID** only after maintainer review.
- A **Content Hash** describes current content for change detection.
- **Deck Federation** combines one base **Deck** with zero or more **Overlays**.
- An **Overlay** contains an **Overlay Fragment**.
- An **Overlay** may use a **Translation Dictionary** to translate extracted source text without repeating per-field replacement boilerplate.
- An **Overlay** may contain **Stale Translations** when source text changes invalidate existing target-language decisions.
- An **Overlay** may use **Field Fills** to add content to existing blank note fields without misclassifying that content as a translation.
- A **Source Variable** lets shared card template structure refer to phrase values translated by a **Translation Dictionary**.
- An **Overlay** uses **Change Intents** to change **Deck Entities** by **Stable ID**.
- Replace, remove, and override **Change Intents** require an **Expected Base**.
- A remove **Change Intent** creates a **Tombstone**.
- An **Overlay Catalog** names the overlays available in a **Federated Deck**.
- A **Language Catalog** names the **Source Language** and **Target Languages** available in a **Federated Deck**.
- A **Target Language** is produced by applying a **Translation Overlay** to source-language content.
- An **Overlay Dependency** constrains the order of an **Overlay Stack**.
- An **Overlay Stack** applies overlays in declared or dependency-expanded order.
- A **Build Target** selects overlays from an **Overlay Catalog** for composition.
- **Compose** applies an **Overlay Stack** to a base **Deck** to produce a **Resolved Deck**.
- A **Semantic Diff** compares **Decks** through **Deck Entities** and **Stable IDs**.
- A **Federation Conflict** must be resolved explicitly.
- **Translation Overlays**, **Extension Overlays**, **Patch Overlays**, and **Personal Overlays** are kinds of **Overlay**.
- **Adapter Equivalence** compares only the data shared by Canonical Deck and a distributable form; it does not recover maintainer source structure.

## Example dialogue

> **Dev:** "If a translator adds German text to Ultimate Geography, are they creating a new independent deck?"
> **Domain expert:** "No — they are participating in **Deck Federation** by applying a translation overlay to the base **Deck**."

## Flagged ambiguities

- "sync tool" previously meant live bidirectional note-system synchronization; resolved: **Brain Brew** is first a local-first **Deck Federation** tool, not an Anki-to-source merge system.
- "canonical note" previously meant the central federation object; resolved: the central object is the **Canonical Deck**, because a **Deck** includes more than notes.
- "content hash" previously meant identity; resolved: a **Content Hash** detects change, while a **Stable ID** defines sameness.
- "Anki GUID" could mean canonical identity; resolved: Anki/CrowdAnki GUIDs are **Adapter IDs**, while human-readable **Stable IDs** identify canonical deck entities.
- "personal overlay" was used to mean both learner workflow and derivative change; resolved: a **Personal Overlay** is a derivative change, while a full learner workflow is not implied.
- "preserve Anki history" could mean storing review data; resolved: **Review History** remains outside **Canonical Deck** content and is preserved through stable identity.
- "media in the deck file" could mean embedded bytes; resolved: **Canonical Deck** stores **Media References**, while **Media Assets** remain external files.
- "overlay order" could imply last-write-wins; resolved: an **Overlay Stack** is ordered, but conflicting changes fail unless explicitly resolved.
- "byte-for-byte adapter equivalence" could mean preserving arbitrary input formatting; resolved: byte stability applies to **Canonicalized Source**.
- "CSV source" previously implied the maintainer source of truth; resolved: the **Canonical Deck File** is the source of truth, while CSV is an adapter format.
- "subdeck" could mean Anki deck hierarchy; resolved: composable source packages in a deck federation are **Federated Decks**, not Anki subdecks.
- "translated deck identity" could mean a separate stable identity per language; resolved: translations use language-neutral **Stable IDs** for the same conceptual **Deck Entities** and language-specific external identities remain **Adapter IDs**.
- "Ultimate Geography support" could mean product-specific application behavior; resolved: Ultimate Geography is a demanding case study and parity fixture for general Brain Brew federation behavior, not a special-purpose application feature.
- "migration import" could mean Brain Brew should convert every legacy source layout; resolved: initial migration means refactoring into **Canonical Deck Files** and proving output parity, not building public legacy source importers.
- "webview" and "Iced server" were used for an interactive editing surface; resolved: the product concept is a **Deck Workbench**, and it must not become a separate source of truth.
- "language" could mean either source text language or translated output language; resolved: use **Source Language** for the base text and **Target Language** for languages produced by **Translation Overlays**.
