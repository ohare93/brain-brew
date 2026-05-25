# ADR-009: Federated Deck Architecture Through CanonicalNote Composition

**Date**: 2025-08-06  
**Status**: Accepted  
**Deciders**: Project Lead  

## Context

Users need to create extensions, translations, and modifications of existing flashcard decks without duplicating the entire deck definition. Similar to how Ultimate Geography has multiple language variants and community extensions, users should be able to:

1. **Extend decks** - Add new fields (population data, economic data, etc.)
2. **Translate decks** - Replace content and field names for different languages  
3. **Modify decks** - Filter, correct, or enhance existing content
4. **Compose extensions** - Build on other people's extensions

The challenge is maintaining clean, declarative recipes while enabling complex deck federation without format-specific dependencies.

## Decision

Implement federated decks through **CanonicalNote composition** with simple input declaration and explicit pipeline joins.

**Core Principles:**
- **CanonicalNote as federation format** - All federation happens at the canonical level, not format-specific (CSV, markdown, etc.)
- **Simple input declaration** - Inputs only declare data sources and types
- **Explicit pipeline operations** - Pipeline transforms handle joining, extending, and overriding logic
- **Composable chain** - Each federated deck outputs CanonicalNotes for the next extension

## Rationale

### Federation Pattern

```yaml
# Base deck (original)
name: "Ultimate Geography Base"
inputs:
  - csv_file: "./countries.csv"
outputs:
  - anki_deck: "Ultimate Geography"
  - canonical_notes: "./canonical/geography-base.json"  # For federation

# Extension deck (adds population data)  
name: "Geography + Population"
inputs:
  - canonical_notes: "./canonical/geography-base.json"  # Base deck
  - csv_file: "./population-data.csv"                   # Extension data
pipeline:
  - join_notes:
      source: "population-data.csv"
      match_on: "Country" 
      strategy: "extend"       # Add new fields
outputs:
  - anki_deck: "Geography Extended"
  - canonical_notes: "./canonical/geography-population.json"

# Translation deck (replaces content and field names)
name: "Geography German"
inputs:
  - canonical_notes: "./canonical/geography-base.json"
  - csv_file: "./translations/german.csv"
pipeline:
  - join_notes:
      source: "german.csv"
      match_on: "Country"
      strategy: "override"     # Replace existing values
outputs:
  - anki_deck: "Geography Deutsch"
  - canonical_notes: "./canonical/geography-german.json"
```

### Join Strategies

```gren
type Transform = JoinNotes JoinNotesConfig

type alias JoinNotesConfig =
    { source : String              -- Input identifier
    , matchOn : String             -- Field to match notes on
    , strategy : JoinStrategy      -- How to handle data conflicts
    }

type JoinStrategy
    = Extend        -- Add new fields, preserve existing values
    | Override      -- Replace existing field values with new ones  
    | Merge         -- Combine values (for future advanced merging)
```

**Pros:**
- **Format agnostic** - Federation works regardless of original deck format (markdown, CSV, Anki, etc.)
- **Clean separation** - Base recipes stay simple, extensions are separate
- **Composable** - Extensions can build on other extensions in a chain
- **Declarative** - Users declare data sources and join intent, not imperative steps
- **Reversible** - Can trace federation chain through CanonicalNote outputs
- **Publishable** - Each step can be released with CanonicalNotes for community building

**Cons:**
- **Storage overhead** - CanonicalNote files add to release size
- **Chain complexity** - Long federation chains may be hard to debug
- **Join limitations** - Simple field matching may not handle all edge cases

## Alternatives Considered

**Complex Recipe Syntax:**
```yaml
# Rejected: Too imperative
inputs:
  - extend_with: { source: "...", join_on: "..." }
  - override_with: { source: "...", match_on: "..." }
```
*Rejected because it complicates the input declaration and mixes concerns.*

**Format-Specific Federation:**
```yaml
# Rejected: Format coupling
inputs:
  - csv_file: "base.csv"
  - extend_csv: "population.csv"  # CSV-specific extension
```
*Rejected because it creates format dependencies and doesn't work across different source formats.*

**Direct Recipe Dependencies:**
```yaml
# Rejected: Too complex
dependencies:
  - recipe: "base-geography.yaml"
    output: "anki_deck"
```
*Rejected because it creates complex dependency resolution and doesn't enable fine-grained control.*

**Pipeline-Only Approach:**
```yaml
# Rejected: Too imperative  
pipeline:
  - load_base: "geography-base.json"
  - load_extension: "population.csv"
  - join_on: "Country"
```
*Rejected because it makes the pipeline imperative rather than declarative.*

## Implications

**Recipe Structure:**
- All federated decks must output `canonical_notes` for chaining
- Input declarations remain simple (just data sources)
- Pipeline transforms handle all federation logic explicitly

**Data Flow:**
```
Original Deck → CanonicalNotes → Extension → CanonicalNotes → Translation → Final Deck
```

**Community Ecosystem:**
- Base deck maintainers publish CanonicalNotes with releases
- Extension creators reference published CanonicalNotes
- Translation teams can work from any point in the federation chain
- Clear provenance through the CanonicalNote chain

**Implementation Requirements:**
- `join_notes` transform with configurable strategies
- CanonicalNote serialization/deserialization
- Input processing that converts all formats to CanonicalNotes
- Release publishing that includes CanonicalNote artifacts

**Migration from Brain Brew:**
- Existing Brain Brew recipes with derivative CSVs can be converted to this pattern
- Multiple language builds become separate federated recipes
- Complex recipe repetition eliminated through composition