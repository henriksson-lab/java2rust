# New Feature Responsibility Checklist

Use this checklist before adding support for a new Java feature or fixing a
translation corner case. The goal is to keep mappings in one owner instead of
spreading Java-specific special cases through lowering.

## 1. Java Semantics

- Add parser or resolver facts for Java syntax, names, overload targets,
  hierarchy edges, captures, throws, nullability, or generic/type facts.
- Keep Java type inference in `types` and resolver-owned facts in `resolve` or
  `facts`.
- Do not choose Rust ownership, view, dispatch, or object representation here.

## 2. Capabilities And External Facts

- Express reusable type properties as `FactSet` capability queries or external
  fact syntax.
- Prefer facts such as copy, clone, string-like, sequence-like, object-safe,
  extends, implements, final, and closed-world hierarchy over type-name branches.
- Add conflict diagnostics when user-provided facts disagree.

## 3. Object Representation

- Put class, interface, subclass, enum, dyn trait, boxed dyn trait, and erased
  object choices in `representation`.
- Use `ObjectRepresentationPolicy`, `HierarchyOverrideValidationPolicy`, or
  `HierarchyEdgeDiagnosticPolicy` for fallback and diagnostic decisions.
- Lowering may consume `RepresentationPlan`; it must not recompute hierarchy
  strategy.

## 4. Signature And Slot Shape

- Add body-use facts in `usage`; add shape constraints in `signature`.
- Route public, unknown, and storage escapes through `SignatureBoundaryPolicy`.
- Route type-capability baseline constraints through `SignatureCapabilityPolicy`.
- If callers and callees affect each other, add a monotone fixpoint edge rather
  than recursively inspecting function bodies during lowering.
- For recursive code, add only finite graph edges and bounded-domain facts:
  direct recursion, mutual recursion, recursive constructors, and recursive
  hierarchy/capability facts must saturate or report a finite-budget diagnostic.
  Do not create unbounded path-specific facts, fresh recursive slots, or
  recursively specialized Rust shapes.

## 5. Shape Projection

- Put Java-type-to-`RustSlotShape` projection in `shape_projection`.
- Direct ownership, view, dispatch, or mutability axis writes belong there or in
  `slot_shape` constructors, not in lowering.

## 6. Coercion

- Put actual-to-target adapters in `coercion`.
- Use syntax, dispatch, view, ownership, and borrowed-element recovery policies
  before adding a new adapter.
- Fallbacks must expose diagnostics or explicit runtime behavior.

## 7. Rendering And Lowering

- Rendering modules turn planned Rust IR into text.
- Lowering modules assemble plans, call coercion, and emit Rust IR.
- Do not add direct special cases in lowering for ownership adaptation, view
  conversion, hierarchy representation, cast strategy, or public API fallback.

## 8. Required Tests

- Add a narrow fixture or unit test for the Java feature.
- Add or update responsibility guards when introducing a new policy boundary.
- Run `cargo fmt`, `cargo test`, `cargo clippy`, and the responsibility scans
  tracked in `docs/rust-slot-shape-refactor-audit.md`.
