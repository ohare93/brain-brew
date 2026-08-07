# ADR-017: Enforce Semantic Version Package Compatibility

**Date**: 2026-07-10  
**Status**: Accepted  
**Deciders**: Project Lead

## Context

Federated manifests exposed package versions and `compatible_base_versions`, but versions were previously opaque strings and compatibility metadata was not enforced. Dependency checks differed by discovery route. This could allow malformed versions, incompatible extensions, missing packages, or dependency cycles to reach target planning.

Federation needs reproducible exact inputs while still letting an extension state the range of base-package releases it supports.

## Decision

Use the maintained Rust `semver` crate and its requirement grammar.

- `package.version` is a valid full Semantic Version.
- Every `depends_on` item is an exact `<package-id>@<SemVer>` pin. Unversioned dependencies and ranges in `depends_on` are invalid.
- A package that is an extension of another package declares `base_package`. That package must also appear in `depends_on` with an exact pin.
- `compatible_base_versions` is required and non-empty when `base_package` is present, and is forbidden without `base_package`.
- Comma-separated comparators inside one compatibility string are AND conditions. Separate list items are OR alternatives.
- Requirement matching follows the `semver` crate's prerelease rule: a prerelease matches only when a comparator contains a prerelease with the same major, minor, and patch tuple. Build metadata does not affect compatibility precedence.
- Manifest decoding canonicalizes requirement spacing and rejects invalid or empty versions/requirements with the declaring field and manifest path.
- Registry construction validates duplicate identities, exact dependencies, base compatibility, missing packages, and package cycles once over the complete registry assembled from the root, explicit includes, package roots, and sibling locks. Validation completes before target/source planning.

Package-cycle diagnostics include every package ID, version, manifest, and exact dependency edge in deterministic traversal order.

## Rationale

**Pros:**

- Exact dependency pins preserve reproducibility and lock identity.
- Compatibility ranges express an extension's support policy without becoming a package solver.
- One maintained grammar avoids ad hoc range behavior.
- Complete-registry validation gives every discovery route the same fail-closed result.

**Cons:**

- Previously accepted unversioned dependencies and inert compatibility declarations require migration.
- Extension manifests carry both an exact selected base version and a compatibility range.
- This remains validation, not automatic package version selection.

## Alternatives Considered

- **Opaque exact strings only**: rejected because compatibility ranges would remain inert.
- **Ranges in `depends_on`**: rejected because the current lock/registry model resolves exact package identities and is not a solver.
- **Ad hoc comparison syntax**: rejected in favor of the maintained `semver` implementation.
- **Infer the base package from target references**: rejected because mixed-package targets and overlays make inference ambiguous.

## Implications

- Base packages omit both `base_package` and `compatible_base_versions`.
- Extension packages migrate to an explicit base relationship and exact dependency pin.
- The package/lock schema remains experimental and this is an intentional compatibility break.
- Tools must preserve canonical requirement strings when formatting manifests.
