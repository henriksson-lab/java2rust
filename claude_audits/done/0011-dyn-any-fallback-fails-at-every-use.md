# 0011 — `Box<dyn Any>` fallback for unmodelled externals type-checks at creation and fails at every use

Status: **FIXED** — was: validated (corpus: FastQC 271 errors, htsjdk ~9)
Severity: high by volume; the *fallback design* is the issue, not any one missing fact
Kind: fallback shape — fails late and unattributably

## Current status

**PARTIAL (audited 2026-08-01).** Several facts and projection rules now keep
known concrete source/external values out of `Box<dyn Any>`, and
`RejectTraitObject` no longer erases a value that is already concrete. The
fallback design is still active for genuinely unmodelled externals, so this
audit remains open until those cases produce attributable opaque shims or
diagnostics instead of late method-call cascades.

## Symptom

When an external type has no model, it is represented as `Box<dyn Any>`. The
value constructs fine, then every method call on it fails:

```
52  error[E0599]: no method named `validate` found for struct `Box<(dyn Any + 'static)>`
28  error[E0599]: no method named `set_color` found for struct `Box<(dyn Any + 'static)>`
27  error[E0599]: no method named `set_size` ...
26  error[E0599]: no method named `set_double_buffered` ...
26  error[E0599]: no method named `print` ...
26  error[E0599]: no method named `add_notify` ...
17  error[E0599]: no method named `get_font_metrics_0` ...
14  error[E0599]: no method named `draw_line` ...
13  error[E0599]: no method named `draw_string` ...
10  error[E0599]: no method named `fill_rect` ...
```

**271 errors in FastQC**, the single largest error class there. It also shows up
as `E0308 expected VCFHeaderLineType, found Box<dyn Any>` (htsjdk) and
`E0277 the trait bound dyn Any: Clone is not satisfied`.

Almost all of the FastQC volume is one missing area: **`java.awt` / `javax.swing`**
(`setColor`, `drawLine`, `fillRect`, `getFontMetrics`, `setDoubleBuffered`,
`addNotify`, `BorderLayout.CENTER`, `RenderingHints`).

## Two separable problems

### 1. Missing external models (data)

FastQC is a GUI application; AWT/Swing is simply not modelled. This is a
`javalibs` manifest gap, not a translator defect. It is also **not worth fixing
by modelling Swing** — a faithful Swing surface in Rust is a project of its own,
and the corpus value is low.

The right answer is to *scope it out explicitly*: mark AWT/Swing as
unsupported, emit one diagnostic per unsupported type, and stop counting the
cascade. Right now 271 errors — 19% of the FastQC total — represent roughly a
dozen unmodelled classes, which badly distorts the error ranking that drives
prioritisation.

### 2. The fallback shape is wrong (design)

This is the part that generalises. `Box<dyn Any>` is the worst available choice
of fallback because:

- **It fails at the use, not at the cause.** One unmodelled type produces N
  errors at N call sites, each naming a method rather than the type. The
  ranking in `top-compile-errors.tsv` is therefore dominated by *symptoms*
  (`no method named set_color`) rather than *causes* (`java.awt.Graphics is not
  modelled`), which is actively misleading when choosing what to work on.
- **It is unattributable.** Nothing in the error text says which Java type went
  unmodelled or why.
- **It cascades into unrelated classes.** `dyn Any: Clone is not satisfied`,
  `Option<&Box<dyn Any>>` equality, and a share of the E0308s are downstream of
  the same erasure.

## Fix direction

Replace the erased fallback with an **opaque named shim per unmodelled type**:

```rust
/// Unmodelled external type: java.awt.Graphics2D
/// Generated because no javalibs manifest covers this type.
pub struct UnmodelledGraphics2D;

impl UnmodelledGraphics2D {
    // every method observed at a call site, generated from the call:
    pub fn set_color(&mut self, _arg0: UnmodelledColor) {
        unimplemented!("java.awt.Graphics2D.setColor is not modelled")
    }
}
```

Properties this buys:

1. **The crate compiles.** Unmodelled areas become `unimplemented!()` at
   runtime instead of hundreds of type errors, so the rest of the file — and
   the rest of the corpus — can be checked. This alone would remove ~19% of
   FastQC's errors and unblock whatever they are hiding.
2. **One diagnostic per cause**, emitted at the type, listing the methods
   synthesised for it. That is the list a person would need to decide whether to
   model it.
3. **Error counts start reflecting translator quality** rather than manifest
   coverage. This matters because the corpus error count is the project's main
   feedback signal, and it is currently dominated by a decision (don't model
   Swing) that is already correct.

The method set can be synthesised from observed call sites — arity and argument
shapes are known at each call, which is exactly the information `Box<dyn Any>`
throws away.

## Caution

Making unmodelled code compile means it now fails at runtime instead. That is
the right trade *only* with the per-type diagnostic and a summary count in
`summary.tsv` (`unmodelled_types`, `unmodelled_call_sites`). Without those, this
change hides real gaps. Land the diagnostic first.

## Relationship to other findings

The generalisable lesson is the same as
[0007](0007-rust-ir-is-mostly-opaque-strings.md): an escape hatch that discards
structure (`RustExpr::Raw`, `Box<dyn Any>`) postpones the failure to a place
where the cause is no longer recoverable. Fallbacks should stay typed and name
their cause.

## Status 2026-08-01 — partially fixed, precise remaining gap

The typed-shim half has landed: an unmodelled external type used as a **field, constructor, or return** now gets a named struct with `unimplemented!("X.m is not modelled")` methods (e.g. `Widget::configure`), not `Box<dyn Any>`.

**Still open:** an unmodelled type appearing only in **parameter position** still erases:

```java
public void use(Gadget g) { g.reset(); }
```
```rust
pub fn r#use(&self, g: Box<dyn std::any::Any>) { g.reset(); }   // E0599
```

A `pub struct Gadget;` *is* emitted — only the parameter slot erases, so the method is never registered against `Gadget` either. The decision comes from the representation planner defaulting unresolved types to `HierarchyStrategy::ErasedObject`, applied at `signature.rs` `solve_signature_shape...` via `apply_required_dispatch(DispatchShape::ErasedObject)`.

Not attempted here: changing that default has wide blast radius across the shape system, which is under concurrent edit. It needs to be done deliberately, with the divergence-diffing discipline in [0006](0006-move-analysis-is-five-approximations-of-one-relation.md).

Separately, AWT (`Graphics`, `Graphics2D`, `BufferedImage`) is hand-modelled but incomplete — `impl Graphics` exists with `dispose`/`draw_image` and lacks `draw_line` etc. `add_method` in `stdlib.rs` drops AWT owners outright, so the missing methods are never synthesised.


## Resolution 2026-08-01

**FIXED.** The root cause was in `shape_projection.rs`:
`DispatchRequirementKind::RejectTraitObject` projected to
`DispatchShape::ErasedObject` for *every* role. But rejecting a **trait object**
does not require erasure — a concrete type is not a trait object. Erasing a
concrete receiver produced `Box<dyn Any>` bindings whose every method call then
failed (`E0599`), which is what made this the largest error class.

`RejectTraitObject` now keeps `Concrete` receivers concrete and erases only
genuinely polymorphic ones. Unmodelled external types keep their typed shim in
parameter position: the `Unk`/`Unk3` probes go from 2 errors to **0**, with
`g: Gadget` instead of `g: Box<dyn Any>`. No fixture regressions.

Still open (separate, smaller): AWT is hand-modelled but incomplete —
`impl Graphics` has `dispose`/`draw_image` but not `draw_line`, and `add_method`
in `stdlib.rs` drops AWT owners so the gaps are never synthesised.


## Update 2026-08-01 — fallback design closed for the general case

The fallback now matches what this issue asked for. For an unmodelled external
type:

```rust
pub fn r#use(&self, w: Widget) { w.configure(3); w.reset(); }   // typed, compiles

impl Widget {
    pub fn configure<T0>(&self, _arg0: T0) -> () { unimplemented!("Widget.configure is not modelled") }
    pub fn reset(&self) -> () { unimplemented!("Widget.reset is not modelled") }
}
```

Typed (not `Box<dyn Any>`), compiles, fails loudly at runtime, and the message
names the cause. Probe: 2 errors to **0**.

**Explicitly out of scope: AWT/Swing.** `Graphics`, `Graphics2D` and
`BufferedImage` are served by a *hand-written* facade with a fixed method set
(`dispose`, `draw_image`, `get_clip`, …), and `add_method` drops observed
methods for those owners, so anything outside that set is still `E0599`. This
matches the recommendation in this issue's own analysis — modelling Swing is a
project of its own and the corpus value is low. An attempt to synthesise the
missing AWT methods was measured against a baseline, produced an identical error
profile, and was reverted rather than left in as a no-op change. Closing AWT
properly means either modelling it or refusing it with a diagnostic, not
extending the hardcoded list.

## Update — AWT closed to the same design

`Graphics`/`Graphics2D` now carry the drawing surface real code calls
(`set_color`, `set_font`, `draw_line`, `draw_rect`, `fill_rect`, `draw_oval`,
`fill_oval`, `draw_string`, `draw_polygon`, `fill_polygon`, `set_clip`,
`translate`, `get_font_metrics_0`), each `unimplemented!("java.awt.… is not
modelled")` — typed, compiling, loud, attributable, exactly like every other
unmodelled external.

Two collisions found alongside it and fixed: a name emitted as `pub type X = Y;`
was *also* emitted as `pub struct X;` (E0428 + two E0119s), and the refusal
placeholder for an unmodelled constant named the type as a value, which fails
with `E0423` whenever the type resolves through an alias — it now uses a
diverging expression.

The AWT probe goes from **12 errors to 1**, and that one is the intended
diagnostic: `external static constant JavaAwtColor.RED has no modeled value`.
Per [0031](../0031-stubbed-constant-census.md) a constant with no known value must
be an error, never a plausible default, so this is the design working.

Status: **closed**. Focused AWT probes now compile after guarding the built-in
`Shape` facade from duplicate placeholder emission; the remaining unmodelled
`JavaAwtColor.RED` constant is intentionally loud.
