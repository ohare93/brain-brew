# Agent Guidance

## Project Aim

Brain Brew is a Rust-based, local-first deck federation and round-trip engine for shared Anki-compatible decks. The first milestone is not a web app, SaaS, live sync tool, legacy Python recipe compatibility, or full Ultimate Geography clone.

Read these before making design changes:

- `CONTEXT.md` — domain glossary
- `documentation/docs/reference/project-scope.md` — current scope and architecture boundaries
- `documentation/docs/reference/decisions/README.md` — active ADR index

Use the project skill `skills/federated-deck-extensions/SKILL.md` whenever creating, reviewing, or refactoring Federated Deck source, translation overlays, extension overlays, field fills, or UG-style variant targets. It captures the variable-first/shared-extension workflow and common mistakes to avoid.

## Development Method

Use TDD with Red-Green-Refactor:

1. Add a failing test for the next behavior.
2. Implement the smallest change that passes.
3. Refactor with tests still green.

Scaffolding can exist without tests, but domain behavior, format behavior, adapter behavior, and CLI behavior should enter through a failing test.

## Crate Boundaries

- `brain-brew-core`: pure domain only. No YAML, CrowdAnki, filesystem, terminal, or CLI dependencies.
- `brain-brew-formats`: reusable YAML/CrowdAnki codecs over core types.
- `brain-brew-cli`: thin command-line wrapper, filesystem access, prompts, and report rendering.

## Commands

Use Devbox:

```bash
devbox run fmt
devbox run test
devbox run clippy
devbox run ci
```

Run `devbox run ci` before committing meaningful code changes.

## Version Control

This repo uses Jujutsu. Use `jj status`, `jj diff`, and `jj commit`; do not use direct `git` workflow commands.
