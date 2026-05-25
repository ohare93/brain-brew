---
title: Ultimate Geography release oracle
---

# Ultimate Geography release oracle

The Ultimate Geography fixture can compare against public release artifacts when those artifacts are available locally.

The repository does not check in release ZIPs or extracted `deck.json` files.

## Fetch an oracle

```bash
scripts/fetch_ug_release_oracle.py --tag v5.3
```

By default this writes:

```text
.cache/brainbrew/ug-release-oracle/v5.3/
  oracle-manifest.json
  crowdanki/
    Ultimate Geography [EN]/deck.json
    Ultimate Geography [EN] [Extended]/deck.json
    ...
```

## Fetch one target

```bash
scripts/fetch_ug_release_oracle.py --tag v5.3 --target en-standard
```

## Use a custom oracle path

```bash
BRAINBREW_UG_CROWDANKI_ORACLE=/path/to/oracle/crowdanki \
  cargo test -p brain-brew-formats --test ultimate_geography_fixture
```

If no oracle is present, the parity test prints a skip message. Normal offline development still works.

## What this proves

The oracle is a fixture-level parity aid for Ultimate Geography migration work. It is not required for ordinary Federated Deck packages.
