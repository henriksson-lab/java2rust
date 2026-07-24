# Corpus Fix Next Steps

Date: 2026-07-23

This is the current proceed-from-here plan for finishing the active corpus-fix
goal. It starts from the latest focused evidence:

- Focused corpus:
  `.tmp/corpus-fastqc-exception-subset-after-readint-facade`
- Translation: 9/9 files translated.
- Invalid-pattern scan: 0 files.
- Generated Rust: still fails with 19 reported errors.
- Local translator gates are green:
  `cargo fmt --check`, `cargo check --quiet`,
  `cargo test --test idiom_gaps --quiet`,
  `cargo test --test parser_gaps --quiet`,
  `cargo test --test batch_translation --quiet`,
  `cargo test --test type_coercion --quiet`, and
  `cargo build --release --quiet --bin new-java2rust`.

## Objective

Reduce the current corpus failures by adding minimized idiom fixtures and
general translator fixes. Keep package-specific behavior in `javalibs`; keep
GenomicsDB, DISQ/Spark distributed execution, and future C/C++ native behavior
as explicit handoff or facade boundaries rather than pretending Java lowering
can mechanically reproduce those runtimes.

## Immediate Work Order

### 1. Slot-Aware `keySet()` Set Conversion

Status: done.

The earlier subset emitted a `Vec<_>` for a method declared to return
`Set<String>`:

```rust
return self.context.keys().cloned().collect::<Vec<_>>();
```

Plan:

1. Done: add a minimized idiom test for returning `map.keySet()` from a
   `Set<String>` slot.
2. Done: keep existing `keySet().toArray(...)` behavior as `Vec<_>`.
3. Done: add a slot-aware conversion path so `keySet()` collected into a Java
   `Set`/`HashSet` slot renders as
   `collect::<std::collections::HashSet<_>>()`.
4. Done: rerun local gates and the focused FastQC exception subset.

Acceptance:

- `keySet().size()` remains lowered through native length.
- `keySet().toArray(...)` still lowers to a vector pipeline.
- `keySet()` in a `Set<T>` return/local/assignment slot lowers to a Rust
  `HashSet`.
- The focused subset loses the `get_keys` return mismatch without adding new
  invalid patterns. Evidence:
  `.tmp/corpus-fastqc-exception-subset-after-keyset-set-slot`, 23 generated
  Rust errors, 0 invalid-pattern files.

### 2. Source Trait Object ABI

Remaining rows include missing `as_any` on `&dyn Localizable`, an invalid
`Vec<Localizable>` trait storage type, a unit-to-`Localizable` coercion, and a
lifetime error. These should be fixed as source interface/object-model
semantics, not as FastQC-specific facts.

Plan:

1. Add fixtures for a source interface used as:
   - a dyn-trait parameter;
   - a vector element type;
   - an enum implementor;
   - an erased object recovery target.
2. Decide one trait ABI:
   - either generated source traits inherit a Java object recovery surface, or
     downcast/recovery lowering stops asking for `as_any` when the source type
     is already a dyn-trait value.
3. Ensure generic source types such as `List<Localizable>` render as
   `Vec<Box<dyn Localizable>>` or another explicit carrier, never
   `Vec<Localizable>`.
4. Keep true unsupported downcasts marked explicitly when no runtime identity
   model exists.

Acceptance:

- No generated `Vec<SomeTrait>` bare trait element types.
- No `pattern.as_any()` calls unless the generated trait ABI provides that
  method.
- No unit placeholders are coerced to source traits.
- The focused subset loses the `Localizable` rows.

### 3. Erased `Object` Carrier Policy

The subset still has clone/display problems around `Box<dyn Any>` and
`Vec<Box<dyn Any>>`, especially in exception message formatting and
serialization.

Plan:

1. Add fixtures for:
   - storing erased varargs;
   - forwarding erased varargs;
   - formatting erased varargs through `StringBuilder.append`;
   - serializing/deserializing erased values through JDK stream facades.
2. Introduce or reuse a generated runtime carrier helper for erased Java
   objects that supports the operations Java source actually uses:
   debug/display fallback, clone when source requires copying, and optional
   serialization handoff.
3. Avoid pretending arbitrary `dyn Any` is cloneable or displayable directly.
4. Use explicit TODO-bearing runtime helpers where Java semantics require
   identity/serialization that the translator cannot prove.

Acceptance:

- Generated code no longer calls `.clone()` or `.to_string()` directly on
  `Box<dyn Any>` or `Vec<Box<dyn Any>>`.
- Exception message formatting compiles with an explicit erased-object display
  policy.
- Serialization gaps are either type-directed or explicit handoff helpers.

### 4. Small JDK Facade Cleanup

Status: partially done.

The current subset no longer has the unresolved `arg_utils` helper row, and
`ObjectInputStream.readInt()` now has a typed JDK facade return. Reflection and
erased-object stream reads still need explicit carrier work.

Plan:

1. Done: fix `ArgUtils.flatten(...)` static owner rendering or replace the helper
   with generated support in the focused crate.
2. Add typed JDK facade facts for high-confidence methods:
   - done: `ObjectInputStream.readInt() -> i32`;
   - deferred: `ObjectInputStream.readObject() -> Java erased object carrier`;
   - `ObjectOutputStream.writeInt(int)`;
   - `ObjectOutputStream.writeObject(Object)`;
   - `Object.getClass().getName()` enough to avoid unit placeholders.
3. Keep these in core only where they are stable JDK surfaces.
4. Add fixtures for each facade method before changing lowering.

Acceptance:

- No unresolved `arg_utils`.
- No unit return from the listed JDK facade methods when source expects a
  value.
- No package-specific library tables are added to `src/stdlib.rs`.
- `readObject()` is intentionally deferred until erased-object carrier and
  `Object[]` assignment casts stop generating invalid downcasts. A typed
  `readObject()` attempt reduced one error but produced invalid generated Rust
  for `(Object) in.readObject()` into an `Object[]` element.

### 5. Rerun Full Corpus Batch

Once the focused FastQC exception subset drops materially, rerun broader
corpuses to gather the next idiom batch.

Plan:

1. Rerun the focused exception subset after each slice:

   ```sh
   scripts/translate-focused-package.sh --skip-resolver-bench --jobs 10 \
     --translate-timeout-sec 60 --offline-cargo \
     --primary /tmp/new_java2rust_fastqc_exception_subset \
     --out .tmp/corpus-fastqc-exception-subset-after-NAME
   ```

2. When the subset is below 15 generated Rust errors, rerun full FastQC.
3. Then rerun the currently tracked corpuses, including the ones expected to
   fail, and update `docs/current-corpus-issue-audit.md`.
4. Add new minimized idiom fixtures for repeated compile-error families before
   implementing fixes.

Acceptance:

- Full FastQC remains 156/156 translated and does not regress on invalid
  patterns.
- Other corpus gaps are represented by focused tests or fixture files.
- The next plan is based on fresh top-error tables, not old batch output.

## Later Corpus Order

After FastQC exception handling no longer dominates:

1. HTSJDK/VCF shared support assembly:
   fix enum/header support syntax and source trait interactions.
2. Spark/DISQ facade bridge:
   keep Spark as a noop facade/handoff boundary; translate only ordinary Java
   glue code and FFI/handoff state.
3. GenomicsDB handoff:
   keep GenomicsDB out of `javalibs`; translate Java setup/control code and
   model native behavior as future C/C++ transpiler handoff signatures.
4. Array-heavy corpuses:
   continue moving obvious `Vec<Vec<T>>` 2D numeric arrays toward `ndarray`
   when shape is rectangular and mechanically provable.

## Guardrails

- Add focused idiom tests before behavioral translator fixes.
- Run the smallest affected focused corpus before broad reruns.
- Do not add non-JDK package/type/method knowledge to `src/stdlib.rs`.
- Put package-specific facades, aliases, exceptions, fields, and ABI facts in
  `javalibs`.
- Do not make algorithmic changes that interfere with future global semantic
  analysis. Prefer indices, reduced cloning, profile builds, and skipped
  no-op scans.
- Use `/data/henriksson/github/claude/rust_refactor` only after generated Rust
  is in a working state; it is for cleanup and inlining shared across
  transpilers, not for making broken generated crates typecheck.

## Verification Loop

For each completed slice:

1. `cargo fmt --check`
2. `cargo check --quiet`
3. Focused tests for the new idiom.
4. `cargo test --test idiom_gaps --quiet`
5. Any directly affected test file, usually one of:
   - `cargo test --test parser_gaps --quiet`
   - `cargo test --test type_coercion --quiet`
   - `cargo test --test batch_translation --quiet`
6. `cargo build --release --quiet --bin new-java2rust`
7. Focused corpus rerun with `--jobs 10`.
8. Update `docs/current-corpus-issue-audit.md` with the new evidence.
