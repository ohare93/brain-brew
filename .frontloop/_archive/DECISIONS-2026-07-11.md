# Remediation decision record — 2026-07-11

These decisions close the Frontloop clarification gates created by the audit-remediation plan.

- **Release**: next preview is `1.0.0-alpha.2`. Supported channels are crates.io (manually published once gates pass), pinned GitHub release artifacts, and a pinned Nix flake/tag channel.
- **Workbench**: local user-selected workspaces are trusted; the experimental local API may be reached by any local browser page; Apply rejects cross-root and cross-filesystem batches. This is not a release-safe browser trust boundary.
- **Translations**: field-name translations are optional display labels and do not count toward required coverage by default. True Anki identifier renames require an explicit atomic validated operation. Ownership belongs to the overlay that introduces a unit; completeness is checked across the final target stack. Blank direct translations are invalid; explicit path-scoped deletion/adaptation intents are required.
- **Ultimate Geography fixture**: Brain Brew maintains an exact pinned production snapshot plus explicit reviewed migration transform and requires fixture plus live-consumer gates. UG legacy workflow choices are out of scope for Brain Brew and belong to the external UG migration PR.
- **Rust crates**: `brain-brew-core` and `brain-brew-formats` are implementation packages for alpha.2, without a supported public Rust API commitment.
- **Core replacement safety**: complete replacements use canonical stable fingerprints; sparse changes use typed property-level expectations.
- **Core tombstones**: typed entity/path variants remain canonical, with a compatibility reader for the prior flat representation.
- **CrowdAnki pull-back**: defer the optional Anki/CrowdAnki-to-source workflow until a real maintainer requests it; it is not current Brain Brew scope.
