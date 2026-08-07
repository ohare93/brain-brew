---
title: Workbench safety and completeness
slug: workbench-hardening
status: active
created_at: 2026-07-09
completed_at:
---

## Goal

Turn the Leptos Workbench from a broad happy-path editor into a conflict-safe, recoverable, bounded, secure local maintainer tool with complete pagination/detail workflows and a typed server/UI contract.

## Sequence

Constrain unsafe Apply immediately, decide the threat model, establish typed contracts, add compare-and-swap/preview/transaction safety, then complete editing workflows, security, cache correctness, and remove compatibility pivots.
