# 0069 — Explicit source-summary `super` calls fell through to `compile_error!`

Status: **fixed** — explicit calls into a non-emitted summary superclass now
lower to the established source-summary boundary fallback, including throwing
methods and direct returned results.

## Root cause

Summary declarations are intentionally cloned into the combined support
program. Summary membership lookup treated those identical copies as an
ambiguous owner and returned `None`, so `CallTargetPlan::SuperBoundary` reached
the generic unsupported-inheritance fallback. Primary current-type lookup had
the same cross-program ambiguity, preventing the return-statement fast path.

Exact and alias matches are now selected from the authoritative summary
program, while genuinely ambiguous bare simple names still fail closed. Direct
super-call lookup prefers the primary program for the current emitted type.
When a support source and a summary declare the same owner, an implemented
support method remains authoritative; the summary copy cannot hijack that
emitted `super` dispatch.

## Verification

`source_summary_super_calls_lower_to_boundary_defaults_not_raw_super_receiver`
and `source_summary_super_returning_throwing_method_is_not_double_wrapped` both
compile their generated Rust crates. The existing ambiguous-summary-owner unit
guard and the emitted-support-versus-summary-peer isolation guard remain green.
