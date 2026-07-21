---
title: Ultimate Geography output oracles
---

# Ultimate Geography output oracles

The maintained Ultimate Geography regression gate is fully committed and
offline. It does **not** fetch a public release, read a user cache, or pass by
skipping an absent oracle.

`fixtures/ultimate-geography-expected/crowdanki/<target>/deck.json` contains the
complete accepted parsed output for all 74 main and 26 companion targets. The
normal formats integration test composes every target and requires exact parsed
JSON equality. `fixtures/ultimate-geography.lock.json` binds those files to the
pinned source and Brain Brew revisions with a canonical semantic digest. The
same test performs strict hash/byte verification against the one vendored real
media tree. See [Ultimate Geography regression fixture](./ultimate-geography-fixture.md).

This fixture-level output gate is separate from the typed
[normalized CrowdAnki equivalence oracle](./crowdanki-equivalence-oracle.md).
The fixture gate detects regression from explicitly accepted complete adapter
output; the normalized oracle documents supported canonical/import semantics.

## Historical external-release helper

`scripts/fetch_ug_release_oracle.py` remains available for an explicit migration
investigation of a public UG release such as v5.3:

```bash
scripts/fetch_ug_release_oracle.py --tag v5.3
```

It writes downloaded evidence below the ignored `.cache/` tree. That helper is
network-dependent, is not invoked by normal tests or fixture checking, and does
not update the committed expected outputs. Its output must not be treated as
accepted merely because it exists. To change the maintained oracle, review the
pinned source/tool delta and use the explicit `--accept-expected` boundary
documented with the fixture sync tooling.
