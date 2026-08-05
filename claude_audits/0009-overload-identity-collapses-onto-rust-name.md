# 0009 — Overload identity collapses onto the Rust name; inherited bodies land on the wrong method

Status: **validated** (corpus: FastQC, 14 files affected)
Severity: **highest correctness severity found so far** — a child's own method body is
silently replaced by an inherited one
Kind: identity modelling — methods keyed by emitted name rather than Java signature

## Symptom

When a child class overrides one overload of a method and inherits a different
overload of the same name from its superclass, the **inherited body is emitted
under the child's signature**. The child's own implementation disappears.

## Validated instance

Java (`FastQC/org/apache/commons/math3/distribution/`):

```java
// AbstractIntegerDistribution.java:55  — inherited default
public double cumulativeProbability(int x0, int x1) throws NumberIsTooLargeException {
    if (x1 < x0) { throw new NumberIsTooLargeException(..., x0, x1, true); }
    return cumulativeProbability(x1) - cumulativeProbability(x0);
}

// BinomialDistribution.java:94  — the child's own override
public double cumulativeProbability(int x) {
    double ret;
    if (x < 0) { ret = 0.0; }
    else if (x >= numberOfTrials) { ret = 1.0; }
    else { ... }
    return ret;
}
```

Generated (`00014_BinomialDistribution.rs`):

```rust
pub fn cumulative_probability(&self, x: i32) -> Result<f64, JavaException> {
    if x1 < x0 {                                    // <-- parent's 2-arg body
        return Err(JavaException::new("NumberIsTooLargeException", ...));
    }
    return Ok(self.cumulative_probability_int(x1)? - self.cumulative_probability_int(x0)?);
}

fn cumulative_probability_int(&self, x: i32) -> Result<f64, JavaException> {
    BinomialDistribution::cumulative_probability(self, x)
}

fn cumulative_probability_int_int(&self, x0: i32, x1: i32) -> Result<f64, JavaException> {
    panic!("incompatible Java interface method implementation: \
            BinomialDistribution.cumulativeProbability parameter count does not match interface");
}
```

Three things went wrong at once:

1. The signature is the child's 1-arg `(x: i32)`; the body is the parent's
   2-arg body referring to `x0`/`x1`. Hence 164 `E0425 cannot find value` errors
   in this corpus.
2. **BinomialDistribution's own `cumulativeProbability(int x)` body is gone.**
   Nothing in the output computes it. Even if the `E0425`s were repaired by
   renaming, the program would be wrong.
3. The genuine 2-arg slot became a `panic!` stub.

## Spread

17 stubs across 14 files in FastQC alone — every class in the
`commons-math3` distribution hierarchy, plus `PoissonDistribution.sample` and
`NormalDistribution.sample`:

```
$ grep -rho 'incompatible Java interface method implementation: [A-Za-z0-9_.]*' src/generated/
ZipfDistribution.cumulativeProbability        WeibullDistribution.cumulativeProbability
TDistribution.cumulativeProbability           PoissonDistribution.sample
PoissonDistribution.cumulativeProbability     PascalDistribution.cumulativeProbability
NormalDistribution.sample                     HypergeometricDistribution.cumulativeProbability
GammaDistribution.cumulativeProbability       FDistribution.cumulativeProbability
ChiSquaredDistribution.cumulativeProbability  CauchyDistribution.cumulativeProbability
BinomialDistribution.cumulativeProbability    BetaDistribution.cumulativeProbability
AbstractRealDistribution.cumulativeProbability
```

Any Java class hierarchy that overloads *and* overrides the same method name —
extremely common — is exposed.

## Root cause

Java method identity is `(name, parameter types)`. The translator reduces it to
the emitted Rust name, then disambiguates overloads with a suffix scheme
(`cumulative_probability`, `_int`, `_int_int`). The inheritance merge runs on
that reduced identity, so the parent's 2-arg method and the child's 1-arg method
compete for the unsuffixed name and one overwrites the other.

`src/inherited_members.rs:290` keys direct methods as
`HashSet<(String, usize)>` — name plus **arity**. Arity is closer than a bare
name but still not the Java signature: it cannot separate
`f(int)` from `f(String)`, and — more importantly — the *rest* of the pipeline
matches on `method.name == …` alone. `src/lower.rs` has at least six such
comparisons (`:1487`, `:20939`, `:21179`, `:21311`, `:21457`, `:21736`).

This is the same failure mode as
[0006](done/0006-move-analysis-is-five-approximations-of-one-relation.md), one level
up: **identity is modelled as a name string**, so distinct entities alias.

## Fix direction

1. Introduce a `JavaMethodSig { name, param_types }` (erased param types are
   enough for overload identity, per JLS 8.4.2) and use it as the key for
   inheritance, override detection, and interface-implementation matching.
   Do **not** key on the Rust name or on arity.
2. Assign Rust names as a *late, total* function of the signature set — after
   the override/inherit graph is resolved, not during it. Overload suffixes are
   a rendering concern and must not participate in identity.
3. Treat "child overrides one overload, inherits another" as the primary test
   case; it is the shape that currently breaks.
4. Replace the `panic!` stub with a translator diagnostic. A `panic!` that says
   "parameter count does not match interface" is reporting a *translator* bug as
   a *runtime* condition in the generated program.

## Regression tests worth adding

```java
class Base   { double f(int a, int b) { return f(a) - f(b); }  double f(int a) { return a; } }
class Child extends Base { @Override double f(int a) { return a * 2; } }
// Child.f(1)    must be 2      (child's own body survives)
// Child.f(3,1)  must be 4      (inherited body, correct args)
```

That single pair pins both directions and would have caught all 14 files.


## Update 2026-08-02 — naming made consistent across override groups

The remaining item is fixed. Overload suffixes were decided from a type's
*declared* methods only, so a subclass overriding one of several overloads got a
different Rust name from the superclass method it overrides — `Base::f_int`
against `Child::f`. Fine while every call is statically bound, wrong the moment
dispatch is virtual.

`assign_type_rust_names` now counts a type's methods **including its
supertypes'** (`method_counts_including_supertypes`), with counts indexed under
both the qualified and simple type name since nested types are stored qualified
while `extends` names them simply. `Child::f_int` now matches `Base::f_int`.

Locked in by `tests/differential/Overloads.java`, which checks the dispatched
*values* through both classes, not just that it compiles.

## Update 2026-08-03 — the empty suffix, and two tests that disagreed

Making names agree across override groups exposed a latent defect in how the
suffix is *spelled*. `resolve_method` built it as `format!("{base}_{suffix}")`
from the joined parameter types, so an overload with **no** parameters got an
empty suffix and came out as `to_string_encoding_` — a trailing underscore.

Before the supertype-aware counting this only fired when a single type declared
a no-argument overload alongside others; afterwards it fired wherever an
ancestor overloaded the name, which is much more often.

Fixed by giving the zero-argument overload the bare name. Only one overload of a
name can take no arguments, so this cannot collide with anything, and it removes
the underscore entirely rather than making it prettier.

**The suffix had never actually been decided.** Two tests in `tests/idiom_gaps.rs`
asserted *contradictory* conventions on the same construct:

```rust
// source_generic_class_bound_overload_call_uses_bound_rust_name
translation.rust.contains("fn get_count_(&self) -> i32;")

// inherited_receiverless_instance_call_uses_current_override_signature_name
!child_impl.contains("self.to_string_encoding_()")
```

One required the trailing underscore, the other forbade it. Neither could pass
while the other did, which is how an accidental spelling survived: any change in
either direction looked like a regression against one of them. Both now assert
the single convention, and the second needed no edit once the underscore was
gone.

The naming assertion in `overloaded_override_preserves_child_body_and_inherited_overload`
was updated from `f` to `f_int` — that rename is the fix this issue is about,
not a regression. Its value assertions were left untouched and independently
checked against the Java oracle (`2,4` from both toolchains) *before* the test
was edited.

### Caution for whoever touches this next

Suffix spelling is asserted textually in several places. Before changing it,
grep for the literal names rather than relying on the suite: `idiom_gaps` does
not terminate ([0035](done/0035-idiom-gaps-suite-never-terminates.md)), so a naming
change can pass review while breaking assertions nobody sees.

## Update 2026-08-03 — the key exists but has two consumers; one live instance found and closed

### Measured state

`JavaCallableKey` / `JavaCallableSlotKey` (`src/java_signature.rs`) key on
`(java_name, erased_param_types, kind, staticness)` — exactly what this issue
asked for. They are consumed by **two files**: the module that defines them and
`src/inherited_members.rs`. Everything else still keys on the emitted Rust name:
`usage::CallableKey::Method` carries a `method_name: String` that is the Rust
name, and `lower.rs` retains 20 `.name ==` comparisons.

So the key was built and then not adopted. That is the remaining work.

### A live instance of the defect, found via 0010

`override_groups_for_program` (`src/exception_plan.rs`) built its group keys from
`method.name` — the **Java** name — while every other `CallableKey::Method` in
the program is built from `source_method_rust_name`. The two spellings coincide
only for a method that is neither camelCased nor overloaded. For
`cumulativeProbability` (→ `cumulative_probability`) and for every overload they
do not, so the fallibility fixpoint's override equalization looked up keys that
were not in the plan and **silently did nothing**, and a second `|=` pass in
`representation_lowering.rs` had to compensate.

Fixed, and made unrepresentable: `CallableKey::source_method(type_name, method)`
now does the naming, and the construction sites that have a `&MethodDecl` go
through it. A future caller cannot pick the wrong spelling without deliberately
bypassing the constructor.

This is worth recording because of *how* it surfaced. It was not found by
reading this issue; it was found by asking why 0010's second store was
load-bearing. **Identity defects present as something else** — here as a
duplicated fallibility join — which is the argument for adopting the key
everywhere rather than fixing spellings as they are noticed.

### Not attempted, and why

Making `usage::CallableKey::Method` carry a `JavaCallableSlotKey` is the real
adoption step. It was deliberately not attempted in this pass: it touches the
call graph, the fallibility fixpoint, signature planning and the 20 name
comparisons in `lower.rs` at once, and this file's own guidance — echoed by
[0008](0008-inference-machinery-does-not-reach-lowering.md)'s caution — is that a
broad simultaneous change produces an error movement that cannot be attributed.
The constructor above is the seam to migrate behind, one caller at a time.

## Update 2026-08-03 (second pass) — interface slots pick the right overload

`implementing_method_candidates` (`representation_lowering.rs`) and its twin
`lower_implementing_method_candidates` (`lower.rs`) collected candidates for a
trait slot by **name**, so `score(int)` and `score(int, int)` were both
candidates for either slot and the first match won.

Both now order candidates by `JavaCallableSlotKey` — the exact Java signature is
tried first. Deliberately an *ordering* change, not a filter: an interface and
its implementation can spell a parameter type differently, and narrowing the set
would lose a real implementation. Ordering makes the correct overload win
without that risk, which is the monotone version of the fix.

### Two things found while testing it

1. **A nested type name reached `as &dyn` unmangled.** `&dyn Outer.Inner` is not
   valid Rust. [0013](done/0013-nested-type-names-unmangled-in-downcast.md)
   fixed this for `downcast::<T>` targets only, so the same name failed in a
   different position — a fix applied at one symptom rather than at the name.
   The normalisation is now `normalize_emitted_java_type_name` and covers every
   `dyn` position. Fixed.
2. **An interface method satisfied by an *inherited* method emitted no `impl` at
   all.** Trait-impl generation looked for every slot among the type's own
   members. Fixed in the follow-up: implemented interface names are canonicalized
   before representation lookup, and interface-slot candidates now include
   inherited methods whose Java slot is not overridden by the child. The former
   known-gap fixture is now `tests/differential/InterfaceOverloads.java`.

## Update 2026-08-05 — a fresh instance, minimized: the two call paths disagree

Probing the remaining `.name ==` comparisons in `lower.rs` turned up a live
instance on the **context-summary** route, and it is a clean demonstration of
what this file means by "identity is modelled as a name string": the *same*
overloaded method gets a different Rust name depending on which lowering path
reaches it.

```java
// summary source, package dep
public class Util {
    public static int pick(int x)      { return x + 1; }
    public static int pick(String s)   { return s.length() + 100; }
    public int inst(int x)             { return x + 2; }
    public int inst(String s)          { return s.length() + 200; }
}
// primary
Util u = new Util();
return Util.pick(1) + "," + Util.pick("ab") + "," + u.inst(1) + "," + u.inst("ab");
```

Java prints `2,102,3,202`. Emitted:

```rust
let u: Util = Util::new();
return format!("{},{},{},{}",
    Util::pick_int(1),        // static path:   overload-suffixed, correct
    Util::pick_string("ab"),
    u.inst(1),                // instance path: both overloads collapse
    u.inst("ab"));            //                onto one name
```

The static path resolves the overload and suffixes the name; the instance path
does not, so `inst(int)` and `inst(String)` are the same Rust method. Two
independent answers to "what is this method's identity", which is the thing this
file asks to be made single.

### And the facade does not provide either name

The generated facade for the summary type is:

```rust
impl DepUtil {
    pub fn pick<R, T0>(_arg0: T0) -> R { unimplemented!("imported static method pick is not modelled") }
}
```

No `new`, no `inst`, and `pick` under the *unsuffixed* name that no call site
uses. So the program does not compile:

```
error[E0599]: no function or associated item named `new` found for struct `DepUtil`
error[E0599]: no function or associated item named `pick_int` found for struct `DepUtil`
error[E0599]: no function or associated item named `pick_string` found for struct `DepUtil`
```

That second half is a facade-completeness gap of the
[0016](done/0016-core-jdk-facades-are-no-op-stubs.md) family rather than an
identity defect, but it sits on top of this one: the facade emitter and the call
sites are a third and fourth place that name methods, and they disagree with
each other *and* with the two above.

**Not fixed in this pass, deliberately.** The repair is the adoption step this
file already identifies — give the facade emitter and the instance-call path the
same `JavaCallableSlotKey` the static path effectively uses — and that is the
broad change the previous update argued should be done one caller at a time
behind `CallableKey::source_method`. Landing half of it would move errors
without attributing them. The reproducer above is written down so the next pass
starts from a failing case rather than from the corpus symptom.

## Update 2026-08-05 (second pass) — the two paths now agree; the facade half does not

The instance/static disagreement reported above is **fixed**, and the cause was
narrower and duller than the audit's framing suggested — which is worth saying,
because the framing pointed at a large refactor and the defect did not need one.

### Cause

`assign_type_rust_names` gives every method its overload-disambiguated Rust
name, and it was already correct, including the supertype-aware counting added
in the first pass. It simply **was not run on context and summary programs**.

The primary program gets it during resolution. A summary source went
`parse_java_source` → `declaration_only_program` → straight into
`ctx.source_summary_type_program`, so every `MethodDecl` in it carried
`rust_name: None`, and `source_method_rust_name` falls back to the bare
snake-cased Java name:

```rust
pub fn source_method_rust_name(method: &MethodDecl) -> String {
    method.rust_name.clone()
        .unwrap_or_else(|| crate::parser::to_snake_case(&method.name))
}
```

The static path escaped because it does not read `rust_name` at all — it
re-solves signatures on an isolated one-type program
(`solve_signatures_with_facts_and_support_context`), and *that* assigns names.
So the same question had two implementations, one of which was fed and one of
which was not. That is this file's thesis, and the instance of it here is a
missing call rather than a missing key.

### Fix

`resolve::assign_rust_names_to_context_program`, applied to summary declarations
in both the single-file and batch paths. Deliberately **not**
`qualify_nested_type_names`: those programs are looked up by the names they
already carry, so requalifying would change lookups rather than names.

```
before:  Util::pick_int(1), Util::pick_string("ab"),  u.inst(1),      u.inst("ab")
after:   Util::pick_int(1), Util::pick_string("ab"),  u.inst_int(1),  u.inst_string("ab")
```

Pinned by `summary_source_overloads_get_one_rust_name_from_every_path`, which
asserts the four resolved spellings **and** that neither collapsed spelling
survives anywhere in the output.

`scripts/gate.sh --all`: **zero movement** on every target. A naming change with
no corpus effect is the expected shape here — the collapsed name was only ever
reachable through the summary route, which the gated fixtures exercise for
signatures rather than for overloaded instance calls.

### What is still open, and it is not an identity defect

The facade emitted for a summary type is still incomplete:

```rust
impl DepUtil {
    pub fn pick<R, T0>(_arg0: T0) -> R { unimplemented!("imported static method pick is not modelled") }
}
```

No `new`, no `inst`, and `pick` under the unsuffixed name that no call site now
uses — so the program still does not compile (`E0599` ×3). The *call sites* are
now right and the *facade* is wrong, where before both were wrong in the same
direction and accidentally agreed on one name.

That is a facade-completeness gap of the
[0016](done/0016-core-jdk-facades-are-no-op-stubs.md) family: the emitter
synthesises a generic arity-based stub instead of the declared methods it has in
hand. It is a **fourth** place that names methods, and it should consume the
same naming pass rather than inventing its own — but the work is in facade
emission, not in identity, and it is not started.

### Status

The adoption step this file has been asking for — making
`usage::CallableKey::Method` carry a `JavaCallableSlotKey` — is **still not
done**, and remains the right description of the residual work. What this pass
shows is that some of what looked like it needed that adoption did not: the
name was already computed correctly and was not being asked for. Worth checking
the remaining `.name ==` sites for the same shape before assuming they need the
key.
