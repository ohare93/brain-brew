---
title: Canonical source integrity and safe mutation
slug: canonical-source-integrity
status: active
created_at: 2026-07-09
completed_at:
---

## Goal

Make every canonical YAML read, format, and mutation path fail closed and preserve maintainer intent. Introduce one include-preserving source-document module and one recoverable workspace transaction module, then migrate all mutators to those interfaces.

## Sequence

Close decoder data-loss defects first; establish source-document and transaction modules next; migrate command families only after those invariants exist; finish with destructive-output and diagnostic consistency.
