# Agent Guidance

## Project Aim

Brain Brew is a Rust-based, local-first deck federation tool for shared Anki-compatible decks. The first milestone is not a web app, SaaS, live sync tool, legacy Python recipe compatibility, or full Ultimate Geography clone.

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
- `brainbrew`: thin command-line package in `crates/brain-brew-cli`, filesystem access, prompts, and report rendering.

## Commands

Use Devenv:

```bash
devenv shell fmt
devenv shell test
devenv shell clippy
devenv shell ci
```

Run `devenv test` before committing meaningful code changes.

The Devenv shell defaults Rust test execution and Cargo compilation to two
parallel workers (`RUST_TEST_THREADS=2` and `CARGO_BUILD_JOBS=2`). These
cooler defaults intentionally trade longer runtimes for lower CPU and thermal
load, and apply to the named scripts as well as focused Cargo commands run
through `devenv shell`. Override either setting for one invocation when more
throughput is appropriate:

```bash
RUST_TEST_THREADS=8 CARGO_BUILD_JOBS=8 devenv shell test
RUST_TEST_THREADS=1 CARGO_BUILD_JOBS=4 devenv shell -- cargo test -p brain-brew-core
```

Run `devenv shell check:rust-parallelism` to verify both the defaults and the
override path in real nested Devenv shells.

## Version Control

This repo uses Jujutsu. Use `jj status`, `jj diff`, and `jj commit`; do not use direct `git` workflow commands.
