# Proceed From Current Corpus State

Date: 2026-07-23

This plan starts from the current checkout and the evidence in
`docs/current-corpus-issue-audit.md`. The active objective is to keep reducing
real corpus blockers, not to broaden the design surface prematurely.

## Current Baseline

Local gates were green before this plan was written:

- `cargo fmt --check`
- `cargo check --quiet`
- `cargo test --test idiom_gaps --quiet`
- `cargo test --test parser_gaps --quiet`
- `cargo test --test batch_translation --quiet`
- `cargo test --test type_coercion --quiet`
- `cargo build --release --quiet --bin new-java2rust`

Previous focused FastQC exception-subset evidence:

- Output:
  `.tmp/corpus-fastqc-exception-subset-after-recursive-implicit-super-fix`
- Translation: 9/9 files translated.
- Invalid scan: 0 invalid-pattern files.
- Cargo: generated Rust still fails with 30 reported errors.
- Top families:
  - 12 mismatched types;
  - 2 missing `ExceptionContext::clone`;
  - 2 missing `Localizable::as_any`;
  - 2 missing `get_context` on child exception structs;
  - erased `Object[]`/varargs clone/display gaps;
  - `Vec<String>::size`;
  - `Locale::US`;
  - `arg_utils`;
  - one `String: Borrow<&String>` mismatch;
  - one unit-to-`Localizable` coercion;
  - one lifetime row.

The latest implicit source-super work added a passing minimized fixture, but it
did not yet reduce the focused subset error count. Treat it as provisional until
the next inspection proves that it removes real generated code drift rather than
only moving errors sideways.

Updated evidence after the source varargs and inherited-context slice:

- Output:
  `.tmp/corpus-fastqc-exception-subset-after-trailing-field-future-read-clone`
- Translation: 9/9 files translated.
- Invalid scan: 0 invalid-pattern files.
- Cargo: generated Rust still fails, now with 26 reported errors.
- Improvement from this slice:
  - explicit `Object...` array forwarding no longer emits `args.as_slice()` for
    owned public varargs slots;
  - expanded varargs such as `max, args` append the existing varargs vector
    instead of nesting it as `vec![max, args]`;
  - inherited `getContext().addMessage(...)` trailing constructor calls lower
    through `__java_self.context.add_message(...)`;
  - flattened field initialization clones a future-read constructor parameter
    when a later trailing statement boxes the same value into erased varargs;
  - focused subset cargo errors dropped from 30 to 26.
- Remaining top families:
  - 10 mismatched types;
  - 2 missing `ExceptionContext::clone`;
  - 2 missing `Localizable::as_any`;
  - erased `Object[]` clone/display gaps;
  - `Vec<String>::size`;
  - `Locale::US`;
  - `arg_utils`;
  - one `String: Borrow<&String>` mismatch;
  - one unit-to-`Localizable` coercion;
  - one lifetime row.

Updated evidence after the JDK/native collection cleanup:

- Output:
  `.tmp/corpus-fastqc-exception-subset-after-keyset-size-locale-us`
- Translation: 9/9 files translated.
- Invalid scan: 0 invalid-pattern files.
- Cargo: generated Rust still fails, now with 24 reported errors.
- Improvement from this slice:
  - `keySet().size()` now lowers through the native collection length path;
  - `Locale.US` is typed as a stable JDK static constant and the generated
    support includes the rendered short owner `Locale::US`;
  - focused subset cargo errors dropped from 26 to 24.
- Remaining top families:
  - 10 mismatched types;
  - 2 missing `ExceptionContext::clone`;
  - 2 missing `Localizable::as_any`;
  - erased `Object[]` clone/display gaps;
  - `arg_utils`;
  - one `String: Borrow<&String>` mismatch;
  - one unit-to-`Localizable` coercion;
  - one lifetime row.

Updated evidence after Set-slot, utility-owner, and `readInt` cleanup:

- Output:
  `.tmp/corpus-fastqc-exception-subset-after-readint-facade`
- Translation: 9/9 files translated.
- Invalid scan: 0 invalid-pattern files.
- Cargo: generated Rust still fails, now with 19 reported errors.
- Improvement from this slice:
  - `keySet()` in a Java `Set<T>` slot now collects map keys into
    `std::collections::HashSet<_>`;
  - unshadowed same-package utility receivers such as `ArgUtils.flatten(...)`
    lower to static support owners;
  - `ObjectInputStream.readInt()` now uses a typed
    `Result<i32, JavaException>` facade.
- Remaining top families:
  - 6 mismatched types;
  - 2 missing `ExceptionContext::clone`;
  - 2 missing `Localizable::as_any`;
  - erased `Object[]` clone/display gaps;
  - one `Object.getClass().getName()` unit facade shape;
  - one `String: Borrow<&String>` mismatch;
  - one unit-to-`Localizable` coercion;
  - one lifetime row.
- Deferred:
  - `ObjectInputStream.readObject()` should wait for an explicit erased-object
    carrier/array-assignment cast policy. A typed `readObject()` attempt reduced
    one error but introduced invalid generated Rust for `(Object) in.readObject()`
    assigned into an `Object[]` element.

## Work Order

### 1. Decide The Implicit Source-Super Slice

Status: done for the current slice.

Decision: keep it. The latest evidence proves useful corpus movement:
inherited `getContext().addMessage(...)` calls now lower through
`__java_self.context`, the explicit varargs forwarding/nesting rows are gone,
and the focused subset dropped from 30 to 26 generated Rust errors.

Inspect the latest generated exception files and compare them with
`.tmp/corpus-fastqc-exception-subset-after-self-temp-seed-fix`.

Acceptance:

- Keep the slice if it removes inherited `context: Default::default()` rows or
  replaces broad constructor placeholders with more precise generated code.
- Continue immediately if the remaining failures are now varargs/interface
  shape issues.
- Rework the slice if it still leaves both `context: Default::default()` and
  missing child `get_context` rows with no useful corpus movement.

Commands:

- Inspect `top-compile-errors.tsv` and `compile.log` for both subset outputs.
- Inspect generated `MaxCountExceededException` and
  `TooManyEvaluationsException`.
- Keep the minimized test
  `implicit_source_super_constructor_initializes_inherited_fields` only if the
  behavior is a real desired invariant.

### 2. Fix Source Varargs Shape For ExceptionContext

Status: partially done.

The next likely blocker is disagreement between source varargs method
signatures and call packing. Generated calls currently include shapes like
passing `args.as_slice()` or mixed vectors into a method that expects owned
`Vec<Box<dyn Any>>`.

Plan:

1. Minimize `ExceptionContext.addMessage(Object pattern, Object... args)`.
2. Add tests for:
   - forwarding an existing varargs parameter;
   - packing fixed extra arguments;
   - mixing fixed arguments with forwarded varargs;
   - preserving erased `Object` carrier values without invalid downcasts.
3. Make source method signature planning and call lowering agree on whether a
   varargs slot is owned `Vec<T>`, borrowed slice, or erased carrier.
4. Prefer one shared varargs plan used by normal calls, constructor delegation,
   and source-super trailing calls.

Acceptance:

- Done: no generated owned `add_message` call passes `args.as_slice()`.
- Done: no generated `add_message` call nests forwarded varargs as
  `vec![max, args]`.
- Done: the focused FastQC exception subset dropped below 30 errors.
- Remaining: erased `Object[]` clone/display rows inside
  `ExceptionContext` serialization/message formatting still need a runtime
  carrier policy.

### 3. Fix Localizable Trait-Object Recovery

Current rows include missing `as_any` on `&dyn Localizable`, one unit-to-trait
coercion, and a lifetime row. These are object-model issues, not package facts.

Plan:

1. Minimize a source interface used as both a dyn-trait parameter and a concrete
   enum implementor.
2. Decide whether source interfaces that need downcast/recovery should inherit
   a generated `JavaObject`/`as_any` supertrait.
3. Ensure enum-to-interface coercion, source trait impl generation, and erased
   object recovery use the same trait ABI.
4. Keep unsupported true downcasts explicit when no runtime identity model is
   available.

Acceptance:

- `&dyn Localizable` has the recovery surface required by generated code, or the
  generated code stops asking for it.
- Unit placeholders are not used as trait implementors.
- The exception subset loses the `as_any`, unit-to-`Localizable`, and lifetime
  rows.

### 4. Fill Small JDK Facade Gaps

Address the small, high-confidence JDK surfaces exposed by the subset.

Status: partially done.

Plan:

1. Add `Vec::size` lowering through the shared collection method plan.
2. Add `Locale::US` as a typed static constant fact/support item.
3. Fix the `arg_utils` receiver/value rendering so generated code references a
   valid support item or avoids the helper.
4. Add focused fixtures for each gap.

Acceptance:

- Done: the exception subset no longer reports `Vec<String>::size` or
  `Locale::US`.
- Done: the exception subset no longer reports missing `arg_utils`.
- Done: `ObjectInputStream.readInt()` no longer returns unit.
- Remaining: `Object.getClass().getName()` still has a weak facade shape, and
  `ObjectInputStream.readObject()` is deferred until erased carriers are
  explicit.
- These fixes stay in core only if they are stable JDK facts; package-specific
  facts still belong in `javalibs`.

### 5. Rerun FastQC Exception Subset, Then Full FastQC

After the exception subset drops materially:

1. Run the focused exception subset with 10 jobs and offline cargo.
2. Update `docs/current-corpus-issue-audit.md` with the new count and top
   families.
3. If the subset is below 15 generated Rust errors or no longer dominated by
   exception object-model rows, rerun full FastQC.

Acceptance:

- Focused exception subset has 0 invalid-pattern files and a reduced generated
  Rust error count.
- Full FastQC still translates 156/156 files without translator panic.
- Full FastQC invalid-pattern count does not regress.

### 6. Move To The Next Corpus Families

After FastQC exception handling no longer dominates the current work, continue
in this order:

1. VCF shared-support enum assembly:
   - unblock the unclosed delimiter around
     `VCFCompoundHeaderLineSupportedHeaderLineType`.
2. Spark/DISQ interface bridge:
   - keep Spark as a noop facade boundary;
   - fix missing source trait impl methods and owned/borrowed delegate mismatch.
3. GenomicsDB overload and handoff:
   - keep GenomicsDB out of `javalibs`;
   - only lower Java-side FFI handover shapes produced by future C/C++ work.
4. Ndarray and array-heavy corpuses:
   - continue fixtures for array creation, switch scaffolding, trait/interface
     assembly, and stable JDK file/string/logging facades.

## Guardrails

- Add minimized idiom tests before each behavioral fix.
- Preserve local gates after every slice.
- Rerun the smallest affected focused corpus before updating broad docs.
- Do not add non-JDK package/type/method tables to `src/stdlib.rs`.
- Keep `javalibs` as the home for package-specific manifests, facades, aliases,
  exceptions, and ABI facts.
- Do not make algorithmic changes that block future global semantic analysis.
  Prefer indices, reduced cloning, and no-op scan skips that preserve the
  current saturation semantics.
- Delegate common Rust cleanup only after generated code is working; the
  external refactoring project at
  `/data/henriksson/github/claude/rust_refactor` is for cleanup and inlining,
  not for making broken generated Rust typecheck.

## Immediate Next Command Set

Use these first:

- `sed -n '1,90p' .tmp/corpus-fastqc-exception-subset-after-recursive-implicit-super-fix/top-compile-errors.tsv`
- `rg -n "ExceptionContext: Default|context: Default::default\\(\\)|get_context\\(|__java_self\\.context|add_message" .tmp/corpus-fastqc-exception-subset-after-recursive-implicit-super-fix/compile.log .tmp/corpus-fastqc-exception-subset-after-recursive-implicit-super-fix/generated-crate/src/generated/00004_MaxCountExceededException.rs .tmp/corpus-fastqc-exception-subset-after-recursive-implicit-super-fix/generated-crate/src/generated/00005_TooManyEvaluationsException.rs`
- `cargo test implicit_source_super_constructor_initializes_inherited_fields --test idiom_gaps --quiet`

Then implement the smallest confirmed next fix and rerun:

- `cargo fmt --check`
- `cargo check --quiet`
- `cargo test --test idiom_gaps --quiet`
- `cargo test --test parser_gaps --quiet`
- `cargo test --test batch_translation --quiet`
- `cargo test --test type_coercion --quiet`
- `cargo build --release --quiet --bin new-java2rust`
