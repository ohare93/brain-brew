---
title: Verify and export
---

# Verify and export

Verification is the CI gate for a Federated Deck workspace.

## Verify one target

```bash
brainbrew verify --manifest brainbrew.yaml --target de-standard
```

## Verify every target

```bash
brainbrew verify --manifest brainbrew.yaml --all-targets
```

Verification checks:

1. manifest parsing and formatting;
2. base deck parsing and formatting;
3. overlay parsing and formatting;
4. lock-file package resolution and hashes;
5. dependency expansion;
6. target composition;
7. Canonical Deck validation;
8. configured CrowdAnki golden checks.

## Verify media

```bash
brainbrew verify --manifest brainbrew.yaml --all-targets --media-root media/
```

With `--media-root`, Brain Brew checks that referenced media files exist and match their declared SHA-256 hashes.

## Export CrowdAnki

```bash
brainbrew export crowdanki \
  --manifest brainbrew.yaml \
  --target de-standard \
  --out build/crowdanki/de-standard
```

With media copied into the CrowdAnki folder's `media/` subdirectory:

```bash
brainbrew export crowdanki \
  --manifest brainbrew.yaml \
  --target de-standard \
  --media-root media/ \
  --out build/crowdanki/de-standard
```

## Default and configured export paths

```yaml
targets:
  de-standard:
    overlays:
      - overlay.translation.de
    exports:
      crowdanki:
        out: build/crowdanki/de-standard
        golden: goldens/de-standard/deck.json
```

When `--out` is omitted, Brain Brew uses `exports.crowdanki.out` when configured; otherwise it defaults to `build/crowdanki/<target>`. For example:

```bash
brainbrew export crowdanki --manifest brainbrew.yaml --target de-standard
```

## Golden checks

When `golden` is configured, `verify` compares generated CrowdAnki JSON against the golden as parsed JSON.

Use `golden_allowlist` only after reviewing concrete differences:

```yaml
golden_allowlist:
  - note_models[0].latex_pre
```
