# 0023 — Constructing a non-static inner class always panics at runtime

Status: **open** (diagnosed; runtime panic still emitted)
Severity: high — any Java program using a non-static inner class dies on first use
Kind: blocked by unimplemented `SharedLocal`/shared receiver construction ([0002](0002-object-aliasing-has-no-representation.md))

## Symptom

Non-static inner classes compile, but their constructor is a `panic!`. Every
`new Inner()` aborts at runtime.

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
pub struct InnerCaptureAdder {
    __outer: std::rc::Rc<std::cell::RefCell<InnerCapture>>,
}

impl InnerCaptureAdder {
    pub fn add(&self, x: i32) -> i32 {
        return self.__outer.borrow().base + x;          // correct
    }

    pub fn new() -> InnerCaptureAdder {
        return InnerCaptureAdder {
            __outer: panic!("unsupported Java inner-class outer capture construction \
                             for InnerCapture.Adder; pass an outer InnerCapture handle"),
        };
    }
}

impl InnerCapture {
    pub fn run(&mut self) -> i32 {
        let a: InnerCaptureAdder = InnerCaptureAdder::new();   // no outer handle passed
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

Only the *construction* side is missing, and it is missing for a structural
reason.

## Root cause: this is 0002 with the consequence made visible

To construct `InnerCaptureAdder`, `run` must hand it an
`Rc<RefCell<InnerCapture>>`. But `run`'s receiver is `&mut InnerCapture` — the
outer object is represented **by value**, and there is no way to obtain a shared
handle to it.

That is exactly the gap in
[0002](0002-object-aliasing-has-no-representation.md): `ObjectRepresentation`
(`src/object_model.rs:151`) has `ConcreteStruct | DynTrait | JavaEnum |
ClosedEnum | ErasedObject` and **no shared variant**. The `Rc<RefCell<…>>` used
for `__outer` (`src/item_assembly.rs:525`) is hand-written for this one field —
noted in 0002 as "a one-off, not a representation the planner can choose". This
issue is what that one-off costs: the field type can be written, but no
representation can produce a value for it.

So 0002 is not only an aliasing-correctness issue. It **blocks a language
feature outright**. That materially raises its priority: `SUGGESTIONS.md`
places 0002b last partly because `E0382` is rare in the corpora, but inner
classes are common in Java and this failure is invisible to compile-error counts
(it compiles; it panics).

## Fix direction

Requires the shared representation from 0002b. Once a type can be planned as
`Rc<RefCell<T>>`:

1. Mark any class that is the outer type of a non-static inner class as
   requiring a shared representation.
2. Give the inner class a constructor taking the outer handle
   (`InnerCaptureAdder::new(outer: Rc<RefCell<InnerCapture>>)`), and lower
   `new Adder()` inside an instance method to pass the enclosing handle.
3. `outer.new Inner()` (qualified creation from outside) uses the same path.

A narrower interim option, if 0002b is far off: restrict the shared
representation to *only* those classes that are outer types of inner classes.
That is a much smaller blast radius than the general aliasing analysis and would
unblock this feature on its own — worth considering as the first concrete
consumer of the new representation, since it has a clear, checkable success
criterion.

## Interim requirement

Until then, this should be a **translator diagnostic**, not a runtime `panic!`
embedded in generated code. As written, a corpus reports zero compile errors for
a program guaranteed to abort. Same principle as
[0018](done/0018-the-compile-error-metric-cannot-see-silent-failures.md).

## Regression tests worth adding

```java
class Outer { int base = 10; class In { int get(){ return base; } }
              int run(){ In i = new In(); base = 20; return i.get(); } }   // 20, not 10
```

The mutate-after-construction step is the point: it distinguishes by-reference
capture from a copy, and a copy-based implementation would return 10.

---

## Update 2026-08-05 — the interim requirement is met; the repair still needs 0002b

Status: **still open** for the feature. What changed is that it is no longer
silent.

This file asked: *"Until then, this should be a translator diagnostic, not a
runtime `panic!` embedded in generated code. As written, a corpus reports zero
compile errors for a program guaranteed to abort."* Both halves are now covered:

- **The translator reports it.** `src/inner_class_capture.rs` walks the program
  for types with an outer-`this` capture and emits
  `inner_class_construction_panics`, naming the inner type, the enclosing type,
  and why:

  ```
  Warning inner_class_construction_panics: IC.Adder captures an enclosing IC
  instance, but its constructor cannot obtain a shared handle to it, so
  `new IC.Adder(…)` panics at runtime. Needs the shared object representation
  (0002b); the generated code compiles either way.
  ```

- **The corpus metric sees it.** The panic text is now the
  `UNSUPPORTED_INNER_CLASS_OUTER_CAPTURE` constant in `src/output_markers.rs`,
  read by both the emitter and `count_invalid_patterns`, with a fixture in
  `tests/output_markers.rs` that fails if the lowering stops emitting it. So an
  inner class no longer contributes zero to every quality signal the project has.

Deliberately a **warning** rather than a `compile_error!`: refusing to emit would
hide the rest of a file behind one feature gap, and the generated code is exactly
what it was. The point is attribution, not obstruction.

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
