# Parked stdlib work (Wave 4, 2026-06)

Two complete, compile-tested, unit-tested runtime implementations that **measured as
NO-GO** (per-corpus regression) and are preserved here as patches against base
`3142580`. The runtime fragments themselves are sound — they are blocked by
**translator-core gaps**, not by the implementations. Resurrect once those land.

## `regex-pattern-matcher.patch` — ✅ RESURRECTED & LANDED (2026-06-22)
Both translator-core blockers below were fixed this session (return-type tracking via
`runtime_method_ret`; nullable-mapped-returns via `"Option<T>"` ret strings), so the
patch was re-based and landed: `src/runtime/regex.rs` + full wiring, net-zero / zero
per-corpus regression, golden 42/42, compilecheck 110/110, the 9 regex unit tests pass.
The parked +13 is gone. This patch file is now historical. (One refinement deferred:
`Matcher.group(n)` returns empty-`String` not `Option<String>` — see TODO.md §4.0 P1.)

### original entry (historical)
### `java.util.regex.Pattern`/`Matcher` (via `regex` crate)
- Real runtime `src/runtime/regex.rs` (9 unit tests, lookahead→never-match fallback so
  Rust-unsupported patterns compile & run without panic), full wiring (map_type_name,
  static_rule for `compile`/`quote`/`matches`, try_emit overloads, Cargo.toml `regex="1"`).
- **Measured jsoup +13** (jsoup is the only corpus using java.util.regex → sole beneficiary
  is sole regressor). Two blockers, both translator-core:
  1. **No return-type tracking for runtime-mapped methods** — `m.group(1).replaceFirst(..)`
     doesn't see `group()` returns `String`, so the String-method rewrite never fires
     (emits unstable `String::replace_first`). `recv_type_name` only handles
     NameExpr/FieldAccessExpr, not a method-call receiver.
  2. **`Matcher.group(n)` is nullable** — `if (m.group(3) != null)` needs `Option<String>`,
     but returning that breaks every `.group().trim()` chain. Same nullable-mapped-value-type
     quirk that blocks atomics.

## `collections-pq-enumset-weakref.patch` — PriorityQueue / EnumSet / WeakReference
- Real runtime `src/runtime/collections.rs` (4 unit tests). All three NO-GO, all jts-driven:
  - **PriorityQueue** jts +12: jts has its OWN `org.locationtech.jts.util.PriorityQueue`
    (simple-name collision — map_type_name clobbers the user type); raw-generic use emits
    `JavaPriorityQueue` with no `<T>` (E0107); `T: Ord` unmet at some sites.
  - **EnumSet** jts +55 (jahmm −3): the generic-keyed-collection-aliasing hazard
    (Hash needs `T: Ord`; collection-rewrite emits `.push()` on EnumSet receivers).
  - **WeakReference/SoftReference** jts +2: `new_1(referent: T)` rejects the `&T` the call
    passes; `.get() as Vec<_>` is a non-primitive cast (E0605).

## To resurrect
Fix the shared translator-core frontier first (return-type tracking for runtime-mapped
method calls + nullable-overlay for mapped value types; project-type-name shadowing &
raw-generic placeholders for the collections). Then `git apply` the patch (re-base as
needed) and re-measure. See memory `stdlib-stub-implementation`.
