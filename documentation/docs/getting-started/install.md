---
title: Install the CLI
---

# Install the CLI

Brain Brew is a Rust CLI named `brainbrew`.

## Run from a Nix flake

From the Brain Brew checkout:

```bash
nix run . -- --help
```

From another deck workspace:

```bash
nix run /path/to/brainbrew -- targets --manifest brainbrew.yaml
```

## Build a local binary

```bash
cd /path/to/brainbrew
nix build .#brainbrew
./result/bin/brainbrew --help
```

For repeated commands:

```bash
NN=/path/to/brainbrew/result/bin/brainbrew
$NN verify --manifest brainbrew.yaml --all-targets
```

## Install into a Nix profile

```bash
cd /path/to/brainbrew
nix profile install .#brainbrew
brainbrew --help
```

After the flake is available remotely, use the flake URL:

```bash
nix profile install github:jeprecated/brain-brew#brainbrew
```

## Developer shell

The repository uses Devbox for day-to-day development:

```bash
devbox run test
devbox run ci
```

The flake also exposes a Rust development shell:

```bash
nix develop
cargo test --workspace --all-targets
```

## Runtime dependencies

The CLI is intended to run natively. Nix is an install/build option, not a runtime requirement for package locking. `brainbrew lock update` computes NAR hashes and fetches path/tarball/GitHub inputs in Rust.
