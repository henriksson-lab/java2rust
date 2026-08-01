# 0006 — The move/clone analysis is five uncoordinated approximations of one relation

Status: **PARTIAL** — was: validated by inspection (structural; direct cause of
[0001](0001-loop-back-edge-invisible-to-move-analysis.md))
Severity: high — this is the mechanism that keeps generating clone/borrow bugs
Kind: missing inference machinery


## Resolution

**PARTIAL (audited 2026-08-01).** `src/liveness.rs` computes the relation once as
a **backward liveness fixpoint** instead of a textual-suffix scan. Because Java
statements are structured, the dataflow is expressed compositionally
(`live_in(stmt, live_out)`), with loops iterated to a fixpoint — which is what
makes the back edge visible. It covers loop back edges, `break`/`continue`
(via the enclosing loop's carried set), `switch` fallthrough, and exception
edges into `catch`/`finally`, and it deliberately over-approximates so an
uncertain construct costs a clone rather than a miscompile.

It is authoritative in **both** block walkers (`lower_block_with_refinements`
and `lower_block_for_body`), and lambda bodies that a lowering turns into loops
go through `LivenessAnalysis::for_loop_body`.

The per-shape rules this issue was about are **deleted**: the enhanced-`for` and
`forEach`-lambda loop-carried rules no longer exist, and those shapes now get
correct clones from liveness alone. Verified across `while`, `for`, `do-while`,
nested, labelled, in-`try`, in-`switch`, enhanced-`for`, and `forEach` — all
`E0382`-free, while `afterLoop` still takes no needless clone (precision kept).

**Still open:** this audit is not closed. `LivenessAnalysis` now stores
`Binding { name, decl }` internally, `LowerCtx::liveness` is non-optional, and
read, write, and within-statement read-count facts are owned by the analysis.
Focused regressions cover terminal statements, switch fallthrough, and lowered
loop-body write analysis. The lowerer still projects live-out/write bindings
back to names, several optional local override sets still exist, and synthesized
fallback scans are still name-based. Closing this requires retiring the
fragmented query surface and making clone/borrow decisions consume binding
identities end to end.

## Observation

`LowerCtx` (`src/lower.rs:218`) threads **six** optional, independently
computed, name-keyed sets through the whole of lowering:

```rust
struct LowerCtx<'a> {
    // ...
    catch_future_use_names:               Option<&'a HashSet<String>>,
    later_read_names:                     Option<&'a HashSet<String>>,
    later_mutated_names:                  Option<&'a HashSet<String>>,
    current_stmt_read_counts:             Option<&'a HashMap<String, usize>>,
    exclusive_terminal_branch_read_names: Option<&'a HashSet<String>>,
    preinitialized_try_capture_names:     Option<&'a HashSet<String>>,
}
```

Five of these are approximations of **one** relation: *can a use of this binding
execute after this program point?* They differ only in which control-flow edges
they happen to cover:

| field | edges it approximates |
|---|---|
| `later_read_names` | textual suffix of enclosing blocks |
| `catch_future_use_names` | exception edges into `catch` |
| `exclusive_terminal_branch_read_names` | one branch-terminates case |
| `current_stmt_read_counts` | repeated uses within one statement |
| `later_mutated_names` | same question, for writes |

No back edges are covered by any of them, which is exactly
[0001](0001-loop-back-edge-invisible-to-move-analysis.md).

## Three structural defects

### 1. Optional means "silently unsound"

Every field is `Option<&…>`, and every consumer uses `is_some_and`:

```rust
fn has_later_read(&self, name: &str) -> bool {
    self.ctx.later_read_names.is_some_and(|later| later.contains(name))
}
```

When the set is absent the answer is `false` — "no later read" — which is the
direction that *omits* a clone. `LowerCtx` is constructed with all of these set
to `None` (`src/lower.rs:353`), so every lowering path that does not go through
the block walker gets the unsound default. A real analysis result should be
non-optional.

### 2. Keyed by `String`, so bindings are not distinguishable

`HashSet<String>` cannot distinguish:

- a field from a local that shadows it,
- a loop variable from a same-named variable in a sibling scope,
- two distinct bindings of the same name in nested blocks,
- `this.x` from a local `x`.

`collect_stmt_read_names` (`src/lower.rs:8000`) partly acknowledges this with
`names.remove(name)` on `VarDecl` and `names.remove(var_name)` on
`EnhancedFor`, but a `remove` on a *name* during a forward walk cannot express
scoping — it removes the name for the whole traversal, including uses recorded
from an outer scope earlier in the same walk.

The analysis needs binding identity (a `BindingId` assigned at declaration),
not names.

### 3. The consumer surface has grown to match

Because the relation is fragmented, the predicates over it are fragmented too.
`src/lower.rs` currently defines **206** `-> bool` functions. A sample of the
ones that are all asking the same underlying question:

```
has_later_read                              has_later_read_facts
has_repeated_current_stmt_read              has_later_or_repeated_current_read
has_catch_future_use                        has_exclusive_terminal_branch_read
current_callable_binding_has_later_read     current_callable_binding_needs_clone_for_move
reused_owned_argument_needs_clone           reused_owned_value_needs_clone
shared_borrow_argument_needs_owned_clone    later_arg_reads_prior_mutable_alias
known_named_clone_recovery                  known_owned_argument_clone_recovery
known_owned_argument_clone_recovery_with_facts
optional_owned_argument_clone_recovery      clone_recovery_idempotence_matches
external_facade_preserving_arg_needs_clone
```

That is ~17 predicates over one relation. Each one encodes a slightly different
guess about which edges matter, which is why fixing one clone bug so often
exposes another.

## Proposed machinery

Replace all five read/mutate sets with a single backward dataflow result:

1. **Binding identity.** Assign a `BindingId` at each declaration site
   (params, locals, catch params, loop variables, pattern bindings). Resolve
   every `Expr::Var` to a `BindingId`. This alone fixes the shadowing class and
   is a prerequisite for everything else.
2. **CFG.** Build a per-callable control-flow graph from the Java IR with loop
   back edges, `break`/`continue`/labelled targets, `switch` case edges,
   `return`, and exception edges into `catch`/`finally`.
3. **Liveness fixpoint.** `live_out(n) = ⋃ live_in(s) for s ∈ succ(n)`;
   `live_in(n) = use(n) ∪ (live_out(n) − def(n))`. Iterate to a fixpoint.
   `src/lattice.rs` and `src/graph.rs` already provide the shape for this; see
   [0008](0008-inference-machinery-does-not-reach-lowering.md).
4. **One query surface.** `is_live_after(binding, point) -> bool`, plus
   `use_count_at(point, binding)` for the within-statement case. Collapse the
   ~17 predicates onto it.
5. **Non-optional.** Compute it once per callable before lowering that callable
   and pass it by value, not `Option`.

## Sequencing advice

Do **not** start by deleting the 17 predicates. Land the analysis first, add an
assertion mode that computes both the old sets and the new result and reports
divergences over the corpus, then retire the old predicates in batches once the
divergences are understood. The divergences are the interesting output — each
one is either a bug being fixed or a deliberate approximation that needs to be
written down.

## Relationship to the clone-insertion problem

Even a correct liveness result does not by itself decide clone-vs-borrow. It
decides when a move is *illegal*. What to do instead — borrow, clone, or change
the representation — is the question in
[0002](0002-object-aliasing-has-no-representation.md). The two are
complementary: 0006 tells you a repair is needed, 0002 provides the vocabulary
to make the right one.
