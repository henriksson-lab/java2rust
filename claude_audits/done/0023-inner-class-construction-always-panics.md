# 0023 — Non-static inner-class construction no longer always panics

Status: **fixed**
(captured-inner callers, qualified `other.new Inner()` calls, and ordinary
outer calls on an implemented shared outer handle now route through associated
`_with_outer` companions, including a static-reachable `Arc<Mutex<Outer>>`
outer)
Severity: closed for the checked `InnerCapture`/`QualifiedInnerCapture` and
static-reachable shared-outer shapes. Remaining unsupported cases are outside
this construction lowering path.

Current proof: `output_markers::inner_capture_routes_shared_outer_handle_companion`
verifies the generated shared-handle companion route, while
`output_markers::top_level_constructor_does_not_report_same_simple_inner_capture_panic`
guards against a false construction diagnostic caused by a same-simple-name
inner type.

Codex recheck 2026-09-18: the companion marker guard, the static-reachable
`Arc<Mutex<Outer>>` identity runtime guard, and the qualified-inner Java/Rust
differential all pass on the current worktree.
Kind: fixed lowering gap

## Symptom

Non-static inner classes compile, and paths that already possess an outer handle
now have non-panicking `_with_outer` constructor and method companions. Ordinary
outer calls on implemented SharedLocal handles also dispatch through those
companions, so the promoted differential fixtures execute without the previous
runtime-loud constructor path. The same route now selects the enclosing type's
actual implemented handle placement, including `Arc<Mutex<Outer>>` for a
static-reachable outer; the remaining fallback is only for callers that have no
implemented outer handle to pass.

```java
public class InnerCapture {
    private int base = 10;
    class Adder {
        int add(int x) { return base + x; }
    }
    public int run() {
        Adder a = new Adder();
        base = 20;
        return a.add(1);       // Java: 21
    }
}
```

Generated:

```rust
pub struct InnerCapture_Adder {
    __outer: std::rc::Rc<std::cell::RefCell<InnerCapture>>,
}

impl InnerCapture_Adder {
    pub fn add(&self, x: i32) -> i32 {
        return self.__outer.borrow().base + x;          // correct
    }

    pub fn new() -> InnerCapture_Adder {
        return InnerCapture_Adder {
            __outer: panic!("unsupported Java inner-class outer capture construction \
                             for InnerCapture.Adder; pass an outer InnerCapture handle"),
        };
    }

    pub fn new_with_outer(
        __outer: std::rc::Rc<std::cell::RefCell<InnerCapture>>,
    ) -> InnerCapture_Adder {
        return InnerCapture_Adder { __outer: __outer.clone() };
    }
}

impl InnerCapture {
    pub fn compute(&mut self) -> i32 {
        let a: InnerCapture_Adder =
            panic!("unsupported Java inner-class outer capture construction \
                    for InnerCapture.Adder; pass an outer InnerCapture handle");
        self.base = 20;
        return a.add(1);
    }

    pub fn compute_with_outer(
        &mut self,
        __java_self: std::rc::Rc<std::cell::RefCell<InnerCapture>>,
    ) -> i32 {
        let a: InnerCapture_Adder = InnerCapture_Adder::new_with_outer(__java_self.clone());
        self.base = 20;
        return a.add(1);
    }
}
```

Running it:

```
thread 'main' panicked at ic.rs:1676:45:
unsupported Java inner-class outer capture construction for InnerCapture.Adder;
pass an outer InnerCapture handle
```

## What is and is not in place

The *access* side is done and correct: the `__outer` field exists with the right
`Rc<RefCell<Outer>>` type, and `add` reads `self.__outer.borrow().base`, which
would give Java's by-reference capture semantics — `base` mutated after
construction would be visible, returning 21.

Only the *construction* side for call sites without an existing outer handle is
missing, and it is missing for a structural reason. The bounded captured-inner
method-call route is covered by
`captured_inner_method_call_routes_outer_method_with_outer_companion`: a
receiverless call from a captured inner method to an outer method now lowers as
`self.__outer.borrow().make_with_outer(self.__outer.clone())` instead of
recursing on the inner receiver.

## Root cause: this is 0002 with the consequence made visible

To construct `InnerCapture_Adder`, `compute` must hand it an
`Rc<RefCell<InnerCapture>>`. But `compute`'s receiver is `&mut InnerCapture` — the
outer object is represented **by value**, and there is no way to obtain a shared
handle to it.

That is exactly the remaining mutable shared-receiver side of
[0002](0002-object-aliasing-has-no-representation.md). `SharedImmutable`
storage now exists for immutable aliasing, but ordinary captured-inner
construction needs a mutable outer handle that method receivers can actually
provide. The `Rc<RefCell<…>>` used for `__outer` is still hand-written for this
one field, not a representation the planner can choose end to end. This issue is
what that one-off costs: the field type can be written, but the ordinary outer
method path cannot produce a value for it.

So 0002 is not only an aliasing-correctness issue. It **blocks completion of a
language feature**. That materially raises its priority: `SUGGESTIONS.md`
places 0002b last partly because `E0382` is rare in the corpora, but inner
classes are common in Java and the remaining failure is invisible to
compile-error counts (it compiles; it panics).

## Fix direction

The remaining work requires the shared receiver side of 0002b. Storage planning
already marks constructed non-static inner-class outer types as `SharedLocal`;
what is still missing is implementing that placement and routing calls through
it.

1. Implement the `SharedLocal`/`Rc<RefCell<T>>` placement and borrow policy for
   source receivers.
2. Complete the remaining instance-method routes that start from shared
   receivers. The captured-inner route with an existing `self.__outer` handle is
   implemented; ordinary outer methods still need a receiver representation that
   can produce the enclosing handle.
3. Route qualified object creation such as `outer.new Inner()` through the same
   shared-handle path. The parser/capture identity half is now covered by the
   2026-09-10 follow-up below; the remaining work is the mutable shared receiver
   representation needed to pass the enclosing handle.

The old narrower interim option — mark only outer classes as shared — is
superseded by the current storage planner. The planner already selects
`SharedLocal`; the blocker is that mutable shared placement is still deliberately
unimplemented.

## Current guardrail

```java
class Outer { int base = 10; class In { int get(){ return base; } }
              int run(){ In i = new In(); base = 20; return i.get(); } }   // 20, not 10
```

The mutate-after-construction step is the point: it distinguishes by-reference
capture from a copy, and a copy-based implementation would return 10.
This shape is now covered by `tests/differential/known-gaps/InnerCapture.java`
and the known-gap harness. Promote that fixture when shared outer receivers land.

---

## Update 2026-08-05 — the interim requirement is met; the repair still needs 0002b

Status: **still open** for the feature. What changed is that it is no longer
silent. Later 2026-08-29 work kept the known-gap mode runtime-loud while making
the translator diagnostic and fallback marker explicit; the old panic-marker
discussion below records the first interim guardrail, not a claim that ordinary
inner construction is fixed.

This file asked: *"Until then, this should be a translator diagnostic, not a
runtime `panic!` embedded in generated code. As written, a corpus reports zero
compile errors for a program guaranteed to abort."* Both halves are now covered:

- **The translator reports it.** `src/inner_class_capture.rs` walks the program
  for types with an outer-`this` capture and emits
  `inner_class_construction_panics`, naming the inner type, the enclosing type,
  and why:

  ```
  Warning inner_class_construction_panics: IC.Adder captures an enclosing IC
  instance, but its constructor cannot obtain a shared handle to it. Needs the
  shared object representation (0002b).
  ```

- **The corpus metric sees it.** The unsupported-construction text is the
  `UNSUPPORTED_INNER_CLASS_OUTER_CAPTURE` constant in `src/output_markers.rs`,
  read by both the emitter and `count_invalid_patterns`, with a fixture in
  `tests/output_markers.rs` that fails if the lowering stops emitting it. So an
  inner class no longer contributes zero to every quality signal the project has.

The diagnostic remains a warning so batch translation can still report other
issues in the file; the current known-gap harness expects a loud runtime panic
at the unsupported construction site plus the translator diagnostic.

### Why the repair is still not here

Unchanged, and confirmed while doing the above. To construct the inner class a
method of `Outer` must hand over an `Rc<RefCell<Outer>>`, and its receiver is
`&mut Outer`. No local change produces that handle: it requires `Outer` itself to
be *represented* as a shared handle, which is
[0002b](0002b-shared-representation-design.md)'s `SharedLocal` tier.

The narrower option this file floats — apply the shared representation only to
classes that are outer types of inner classes — remains the right first consumer,
and is now better supported: the diagnostic above enumerates exactly that set of
types, so the blast radius can be measured before the representation work
starts rather than guessed at.

### Update 2026-08-06 — the narrow option does not shrink the change as much as it reads

Checked while working 0002b step 3. Restricting the shared representation to
outer types *does* shrink the set of types affected — `src/inner_class_capture.rs`
enumerates them — but it does not shrink the *kind* of change required, and the
difference matters for sequencing.

The construction site is inside an instance method of the outer class:

```rust
impl InnerCapture {
    pub fn run(&mut self) -> i32 {            // <-- receiver is `&mut InnerCapture`
        let a = InnerCaptureAdder::new(/* needs Rc<RefCell<InnerCapture>> */);
```

Declaring `InnerCapture` "shared" changes how it is stored in *slots*. It does
not change this receiver: an inherent method still takes `&mut Self`, and no
shared handle can be recovered from a `&mut` — that is the same direction
0002b measured as impossible (`Unwrap`) and for the same reason. Producing the
handle here needs the *receiver* to be the handle (`self: &Rc<RefCell<Self>>`,
or a stored weak self-reference), which is a strictly larger change than storing
the type behind a handle.

So the narrow option is a good first *consumer* of the shared representation, as
this file says, but it is not a way to get the feature before that representation
exists. The order is fixed: handle in `RustSlotShape` → shared slots → shared
receivers → inner-class construction.

### Codex follow-up 2026-08-06 — batch diagnostics used a narrower path

The direct translator reported `inner_class_construction_panics`, but batch
translation did not run the same post-lowering diagnostic reporters. That left
batch artifacts with the runtime panic marker but no diagnostic code for the
status/reporting layer to consume.

`src/lib.rs` now funnels aliasing, storage-disagreement, and inner-class panic
warnings through one post-lowering helper used by both single-file and batch
translation. `tests/batch_translation.rs` adds a regression for the non-static
inner-class case, and `scripts/gate.sh --all` is clean with the updated
`batch_translation 10/1` baseline.

`tests/differential/known-gaps/InnerCapture.java` now makes the unsupported
path executable known-gap coverage as well: Java must print `21`, the current
generated Rust must panic loudly with the inner-construction message, and the
translator must report `inner_class_construction_panics`.

### Codex follow-up 2026-08-11 — captured outer-field writes use the existing handle

The full construction problem is still blocked by shared receivers, but one
piece of the access side was still incomplete: a non-static inner method could
read an outer field through `self.__outer.borrow().field`, while a bare write
like `limit = value` fell back through the read projection.

`outer_this_field_mut_projection` now lowers that assignment target to
`self.__outer.borrow_mut().limit`, using the `Rc<RefCell<Outer>>` handle that is
already emitted for `__outer`. The borrow is statement-local, matching 0002b's
RefCell rule. `non_static_inner_class_outer_fields_lower_through_outer_capture_handle`
now covers both outer reads and outer writes and compile-checks the generated
crate.

This does not solve construction: `new Inner()` still cannot obtain the outer
handle from an ordinary `&mut self` receiver.

### Codex follow-up 2026-08-11 — handle-taking constructor companion is emitted

The panic constructor is no longer the only constructor body shape emitted for
captured inner classes. When lowering can see the `__outer` struct-field
initializer, it now also emits a companion constructor named
`new_with_outer` (or `new_N_with_outer`) that takes the existing
`Rc<RefCell<Outer>>` handle and stores it directly.

Example shape:

```rust
pub fn new_with_outer(
    __outer: std::rc::Rc<std::cell::RefCell<Outer>>,
) -> OuterInner {
    return OuterInner { __outer };
}
```

This is deliberately not wired to ordinary `new Inner()` calls yet: those calls
still originate from `&self`/`&mut self` receivers and cannot manufacture the
handle. The companion gives the shared-receiver work a concrete non-panicking
target and keeps the existing diagnostic/panic fallback until that receiver
representation exists.

### Codex follow-up 2026-08-11 — sibling inner construction can reuse the captured handle

One construction path was not blocked by shared outer receivers: a method on an
already constructed captured inner class already has `self.__outer`. If that
method constructs another captured inner class with the same enclosing type, the
constructor call can pass `self.__outer.clone()` to the companion instead of
calling the panic constructor.

`Expr::New` lowering now detects that narrow case in both the generic and
concrete-slot constructor paths, switches `new`/`new_N` to
`new_with_outer`/`new_N_with_outer`, and prepends the cloned outer handle. The
regression `sibling_inner_class_construction_reuses_captured_outer_handle`
compile-checks:

```rust
return SiblingInnerConstructionB::new_with_outer(self.__outer.clone());
```

Ordinary `Outer::make() { return new Inner(); }` remains blocked for the same
reason as before: an outer method receiver is still `&self`/`&mut self`, not a
shared handle.

### Codex follow-up 2026-08-12 — the remaining outer-method case is classified

The call-site decision is no longer a bare boolean. `Expr::New` still rewrites
only the renderable captured-inner case, but the helper now names the three
outcomes:

- `CapturedInnerSelfOuterClone`: available today as `self.__outer.clone()`;
- `CurrentOuterReceiverNeedsSharedHandle`: the ordinary
  `Outer::make() { return new Inner(); }` blocker;
- `Unavailable`: unrelated constructors or missing capture facts.

The existing raw-rewrite unit coverage now also pins that classification. This
does not implement shared outer method receivers; it makes the remaining
implementation point explicit instead of folding it into a generic false result.

### Codex follow-up 2026-08-12 — ordinary outer methods get a narrow companion body

The classified outer-method case now has a generated target. Non-static methods
are lowered once normally and once with `current_self_handle = "__java_self"`;
if that changes the body, the second function is emitted as `method_with_outer`
with an explicit `Rc<RefCell<Outer>>` parameter.

For:

```java
Inner make() {
    return new Inner();
}
```

the original method still preserves the old lowering and remains a diagnostic
surface:

```rust
pub fn make(&self) -> InnerConstructionCompanionInner {
    return InnerConstructionCompanionInner::new();
}
```

but the companion is non-panicking when a caller already has the outer handle:

```rust
pub fn make_with_outer(
    &self,
    __java_self: std::rc::Rc<std::cell::RefCell<InnerConstructionCompanion>>,
) -> InnerConstructionCompanionInner {
    return InnerConstructionCompanionInner::new_with_outer(__java_self.clone());
}
```

`non_static_inner_class_constructor_has_outer_handle_companion` compile-checks
the generated crate. The remaining 0023 work is now call-site routing into this
companion once shared receivers exist; the ordinary-method body itself no longer
lacks a non-panicking lowering.

### Codex follow-up 2026-08-11 — captured constructor field initializers reuse the handle

The previous sibling rewrite covered method bodies but missed constructor field
initializers: those lower with no `current_receiver`, so a captured inner class
like `A` could still initialize `B b = new B();` through `B::new()`.

Captured inner constructors now get a narrow post-lowering pass over their
constructor body. If a field initializer constructs another captured inner class
with the same enclosing outer type, the value is stored behind the implemented
shared handle and the generated `_with_outer` companion rewrites the nested call
to pass `__outer.clone()`:

```rust
return OuterA {
    __outer: __outer.clone(),
    b: std::rc::Rc::new(OuterB::new_with_outer(__outer.clone())),
};
```

The pass is deliberately scoped to captured-inner constructor bodies. It does
not turn ordinary outer methods into shared receivers, so `Outer::make() { return
new Inner(); }` remains the direct remaining construction gap.

### Codex follow-up 2026-08-11 — companion constructor raw fallback skips literals

The narrow captured-inner constructor-body rewrite still has a raw-expression
compatibility path for generated text it cannot see structurally. That fallback
looked for `::new` anywhere in the raw string before deciding whether the owner
was a same-outer captured inner type. A diagnostic string or raw string naming
`OuterB::new()` could therefore be rewritten to
`OuterB::new_with_outer(__outer.clone())` even though it was literal data.

The fallback now searches for constructor markers only outside string, char, and
raw-string literals, then applies the existing same-outer companion-path check.
The regression
`same_outer_inner_constructor_raw_rewrite_ignores_literals_and_keeps_real_calls`
pins that quoted constructor names are inert while real zero-arg and one-arg
fallback calls still receive `__outer.clone()`.

### Codex follow-up 2026-08-11 — delegating inner constructors get companions too

Captured inner constructors whose body is only `this(...)` did not contain the
`__outer` panic field initializer, so the handle-taking companion was skipped.
Sibling construction could still be rewritten to `B::new_with_outer(...)`, which
left a missing Rust function.

The companion pass now emits `_with_outer` when either the panic initializer is
replaced or a same-outer constructor delegation is rewritten. Inside companions,
`Self::new...(...)` is recognised as the current captured inner type and becomes
`Self::new..._with_outer(__outer.clone(), ...)`.

`sibling_inner_delegating_constructor_reuses_captured_outer_handle` pins the
whole path: sibling construction calls `B::new_with_outer(self.__outer.clone())`,
and the generated `B::new_with_outer` delegates to
`Self::new_int_with_outer(__outer.clone(), 1)`.

### Codex follow-up 2026-08-11 — raw fallback handles spacing and turbofish calls

The raw compatibility path above skipped literals, but it still treated the text
between `::` and `(` as the constructor name. That missed valid generated
spellings such as `OuterB::new ()`, and would place the `_with_outer` suffix
after generic arguments for `OuterB::new_1::<T>(value)`.

The fallback now parses the constructor token separately from optional turbofish
generic arguments and whitespace before the call. Resolution still asks about
the plain constructor path (`OuterB::new_1`), while rendering preserves the
suffix order Rust expects:

```rust
OuterB::new_1_with_outer::<T>(__outer.clone(), value)
```

The same unit regression that protects literal skipping now covers the spaced
and turbofish forms.

### Codex follow-up 2026-08-11 — diagnostic is gated on the actual remaining representation blocker

`inner_class_construction_panics` used to report every captured inner class
unconditionally. That matched the original state, but the tree now has
non-panicking `_with_outer` companions for paths that already possess an outer
handle, and `SharedImmutable` is implemented for ordinary shared values.

The reporter now asks the storage plan whether the enclosing type has an
implemented mutable shared placement before calling the class unconstructable.
Today this still reports all ordinary outer-method cases: `SharedImmutable`
would only give `Rc<Outer>`, while captured inners need
`Rc<RefCell<Outer>>`, and the `RefCell`/`Mutex` rungs remain unimplemented.
When `SharedLocal`/`SharedSync` and shared receivers land, this diagnostic has a
single predicate to relax instead of remaining as a stale unconditional warning.

### Codex follow-up 2026-08-28 — diagnostic is gated on construction sites

The warning was still emitted once per captured inner class, even when the class
was only declared and never constructed. That overstated the current runtime
panic surface: a captured inner type can require an outer handle, but no program
path can hit the panic constructor until a `new Inner(...)` expression exists.

`inner_class_capture` now first collects constructed type names from field
initializers, static blocks, method and constructor bodies, local types,
lambdas, switches, try resources, and nested expression bodies. A captured
inner class is reported only if that constructed-name set includes the inner
type and the enclosing type still lacks an implemented mutable shared placement.

Pinned by:

- `unused_inner_class_outer_capture_is_not_reported_as_construction_panic`
- `unsupported_inner_class_outer_capture_is_reported_as_a_translation_warning`
- `batch_translation_reports_inner_class_constructor_panics`

The real construction fix is unchanged: ordinary outer methods still need
shared receivers and the mutable shared representation tier before they can call
the non-panicking outer-handle companion.

### Codex follow-up 2026-08-29 — construction-site gating uses exact inner identity

The constructed-name gate above still inserted simple tails into one global set.
That let an unrelated top-level `new B()` count as construction of a captured
`Outer.B`, producing a false `inner_class_construction_panics` diagnostic even
though the panic constructor was unreachable.

Construction matching now uses exact constructed type names. Resolved
constructor targets still mark real captured-inner construction sites such as
`new Inner()`, but same-simple top-level types no longer satisfy nested captured
classes. Regression:
`top_level_constructor_does_not_report_same_simple_inner_capture_panic`.

### Codex follow-up 2026-08-29 — missing outer-handle construction stays runtime-loud

An intermediate pass recorded this path as compile-visible, but the current
checked-in harness keeps the unsupported constructor fallback runtime-loud. The
diagnostic code remains `inner_class_construction_panics` for continuity, and
`tests/differential/known-gaps/InnerCapture.java` now expects
`KnownGapExpectation::LoudRuntimePanic` with the outer-capture message.

This does not implement ordinary `Outer::make() { return new Inner(); }`: the
feature still needs a receiver representation that can pass an
`Rc<RefCell<Outer>>` handle to `new_with_outer`.

### Codex follow-up 2026-08-29 — same-outer constructor raw statement recovery is constructor-specific

The same-outer compatibility pass still needs to recover legacy raw constructor
statements so it can route already-supported outer-handle cases through
`new_with_outer`, but it no longer lets that pass structure arbitrary raw
statements. `rewrite_recovered_same_outer_inner_constructor_raw_stmt` now uses
a constructor-specific recovery gate: only assignment, return, or expression
statements containing a plain `new*` constructor call are recovered. Calls such
as `helper(value);` and `return helper(value);` remain raw when this pass walks
them.

This reduces the `0007` raw-text blast radius around the `0023` compatibility
path. It does not implement ordinary captured-inner construction from outer
methods; shared mutable outer receivers remain the real blocker.

### Codex follow-up 2026-08-29 — companioned fallback bodies no longer poison generated crates

The compile-visible refusal above was too broad for methods and constructors
that also have a supported `_with_outer` companion. Rust still typechecks the
ordinary fallback body even when all reachable supported calls route to the
companion, so a `compile_error!` in that fallback poisoned generated crates
whose useful path was otherwise correct.

Companioned ordinary method bodies now rewrite only the unsupported
inner-constructor `compile_error!` marker to the same explicit runtime
`panic!` handoff used by fallback constructor fields. Call sites that do not
have an outer handle still fail loudly at runtime and still emit the
`inner_class_construction_panics` translator diagnostic; paths that already
carry `self.__outer` or `__java_self` can be cargo-checked through
`new_with_outer`/`*_with_outer`.

The same-outer constructor compatibility matcher also now recovers emitted
nested Rust owners such as `Outer_B` back to source capture keys like
`Outer.B`, so field initializers inside `_with_outer` constructors route nested
inner construction through `B::new_with_outer(__outer.clone())` and apply the
planned shared storage wrapper.

Pinned by:

- `non_static_inner_class_constructor_has_outer_handle_companion`
- `captured_inner_method_call_routes_outer_method_with_outer_companion`
- `sibling_inner_class_construction_reuses_captured_outer_handle`
- `sibling_inner_delegating_constructor_reuses_captured_outer_handle`
- `sibling_inner_field_initializer_reuses_captured_outer_handle`
- `unsupported_inner_class_outer_capture_is_reported_as_a_translation_warning`

The feature remains open for ordinary outer methods, because `&self`/`&mut self`
receivers still cannot synthesize the `Rc<RefCell<Outer>>` handle needed by a
non-static inner constructor.

### Codex follow-up 2026-09-07 — known-gap harness rechecked

A read-only triage suggested `InnerCapture.java` had become compile-loud, but a
focused known-gap run disproved that: ordinary `new Inner()` from an outer
instance method still lowers the missing captured receiver to a `panic!`
expression while reporting `inner_class_construction_panics`. The
`LoudRuntimePanic` expectation remains the accurate current harness mode until
ordinary inner construction can route through a shared outer handle.

### Codex follow-up 2026-09-09 — storage planning now sees the outer-handle requirement

The remaining construction failure is now visible in the storage plan, not only
in `inner_class_capture`. `plan_representations` and lowering diagnostics call a
fact-aware storage planner; when capture facts say a constructed non-static inner
captures `Outer.this`, the planner marks `Outer` as stored and mutated, which
selects `SharedLocal`.

That still does not implement ordinary `new Inner()` from an outer method:
`SharedLocal` remains an unimplemented placement, so the known-gap fixture keeps
the same loud runtime handoff and `inner_class_construction_panics` diagnostic.
The improvement is that 0023's direct consumer now feeds the same 0002b work
queue as other mutable shared-representation requirements. Regression:
`storage_strategy::constructor_mutation_tests::constructed_inner_class_outer_capture_forces_mutable_shared_outer_plan`.

Follow-up cleanup: `inner_class_capture::UnconstructableInnerClass`,
`unconstructable_inner_classes`, and `report_unconstructable_inner_classes` are
crate-local. The translator still reports the same
`inner_class_construction_panics` diagnostic, but the helper surface is no
longer public crate API while the real SharedLocal/outer-handle construction
work remains open.

Follow-up 2026-09-12: the `inner_class_capture` module itself is now
crate-internal as well. The diagnostic path and 0002b work queue are unchanged,
but the unconstructable-inner-class analysis no longer appears as public crate
API. Guard: `layering::declaration_context_usage_lookups_use_java_callable_keys`.

Codex follow-up 2026-09-14: qualified construction is no longer a known gap.
`Expr::New` now preserves the qualifier from `other.new Inner()`, resolver and
analysis traversals visit that qualifier, and lowering passes the already-shared
outer local to `Inner::new_with_outer(other.clone())`. Unshaped constructor
expressions for source types with shared storage are wrapped through the same
`Rc<RefCell<T>>` construction policy used by slots, so
`new Outer().method()` no longer emits a raw value followed by `.borrow()`.
`inner_class_capture` still feeds qualified construction into storage planning
but no longer reports `inner_class_construction_panics` for the supported
qualified path. `tests/differential/QualifiedInnerCapture.java` is promoted and
matches Java (`21`). Ordinary unqualified `new Inner()` from an outer method
remains blocked on shared receivers and stays covered by the known-gap
`InnerCapture.java`.

Codex follow-up 2026-09-10: the ordinary `InnerCapture.java` shape now has a
full-translation marker guard tying this gap to the mutable shared-storage work
queue. `output_markers::inner_capture_gap_reports_mutable_shared_storage_work_queue`
requires the generated Rust to keep the loud unsupported outer-capture handoff,
requires `inner_class_construction_panics`, and also requires the paired
`storage_strategy_surface_not_forwardable` message to name `InnerCapture` as a
shared type that cannot yet take the handle-forwarded tier. The test is
deliberately evidence only; it prevents the inner-constructor runtime gap from
drifting away from the 0002b receiver/borrow-policy blocker.

Codex follow-up 2026-09-10: that full marker sweep also caught a regression in
the construction detector. The detector kept a global simple-name fallback, so
a top-level `new B()` could satisfy nested `Outer.B` when exactly one captured
inner class had the same simple name. `inner_class_capture` now relies on exact
constructed type names, including resolver-provided `Outer.B` names, rather
than rejoining by simple name. Regression:
`output_markers::top_level_constructor_does_not_report_same_simple_inner_capture_panic`
and the full `output_markers` suite.

Codex follow-up 2026-09-14 later: ordinary unqualified `new Inner()` from an
outer instance method now routes through the shared outer handle when the outer
type is already on the implemented `SharedLocal` tier. The source method still
emits its ordinary `&mut self` body, but lowering also emits the existing
`_with_outer` companion that receives `Rc<RefCell<Outer>>`; calls on a shared
outer receiver now bind the handle once and dispatch
`handle.borrow_mut().method_with_outer(handle.clone(), ...)`. The
`inner_class_capture` diagnostic gate now treats implemented `SharedLocal` as
constructable instead of reporting `inner_class_construction_panics`.
`tests/differential/InnerCapture.java` is promoted and matches Java (`21`);
the known-gap harness is down to the single Java-String reference-identity
fixture, `StringEqualsVsEq.java`.
