---
title: Architecture depth and performance
slug: architecture-performance
status: active
created_at: 2026-07-09
completed_at:
---

## Goal

Increase locality and leverage after correctness is restored: split oversized implementations behind stable interfaces, deepen typed path and planning modules, memoize target work, bound collection costs, and retain deterministic behavior.

## Sequence

Do not begin broad structural splitting before source/core/Workbench safety fixes establish behavior. Then split along behavior seams, optimize measured hot paths with budgets, and verify no interface or deterministic-output regressions.
