# 0084 — Static array constants bypassed carrier conversion

Status: **fixed** — associated slice constants convert to ndarray carriers when
used by rectangular 2D APIs, while direct constant indexing remains slice based.

## Root causes

Static-final field lookup searched the current, support, and summary programs as
one candidate set. The current declaration can also appear in the support
program, so an exact current owner was falsely treated as ambiguous. In
addition, the parser's normalized `c_o_e_f` spelling had to be matched against
the Java constant declaration.

Lookup now prefers the type in the program currently being lowered and retains
the ambiguity-refusing fallback for context-only owners. Qualified variable
spellings use their terminal field component. The call-argument fast path also
refuses to call two representations equal when a static nested-slice constant
must cross into an ndarray slot, allowing the existing structured
`Array2::from_rows` projection to run.

## Verification

The 2D table test covers row extraction, readonly helper calls, and owned clone
returns. The existing primitive static-array constant test also becomes green.
The ambiguity unit test remains green and now covers a qualified field spelling.
