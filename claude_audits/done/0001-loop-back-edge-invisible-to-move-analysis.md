# 0001 — Loop back edges are invisible to the move/clone analysis

Status: **FIXED** — was: validated (rustc-confirmed miscompilation)
Severity: high — emits non-compiling Rust for a common Java shape
Kind: missing inference machinery (dataflow), not a local rule bug


## Resolution

**FIXED (verified 2026-08-01).** Method bodies now get a `LivenessAnalysis`
result and the block walkers prefer `live_out_for(stmt)` over the old textual
suffix scan. The focused regression
`loop_liveness_keeps_owned_value_live_across_back_edge` proves the original
LoopMove-style repeated consume now receives a clone across a loop back edge.
Lambda bodies lowered into loops also get a loop-body liveness result.

NOTE: the broader fragmentation in
[0006](0006-move-analysis-is-five-approximations-of-one-relation.md) is **not**
fixed. The current result is still name-keyed, optional in `LowerCtx`, and keeps
legacy suffix scans as compatibility fallbacks for synthesized statements.

## Symptom

Any owned (non-`Copy`) value that is defined outside a loop and consumed by
value inside the loop body is moved without a clone, producing
`error[E0382]: use of moved value ... value moved here, in previous iteration of loop`.

## Minimal repro

`claude_audits/repro/LoopMove.java`:

```java
import java.util.ArrayList;
import java.util.List;

public class LoopMove {
    private List<String> items = new ArrayList<>();

    void add(String s) {
        items.add(s);
    }

    public void run(String value, int n) {
        for (int i = 0; i < n; i++) {
            add(value);
        }
    }
}
```

Reproduce:

```
cargo build --release
./target/release/new-java2rust claude_audits/repro/LoopMove.java > /tmp/loopmove.rs
rustc --edition 2021 --crate-type lib --crate-name probe --emit=metadata --out-dir /tmp /tmp/loopmove.rs
```

## Emitted Rust (verified)

```rust
pub fn add(&mut self, s: String) {
    self.items.push(s);
}

pub fn run(&mut self, value: String, n: i32) {
    let mut i: i32 = 0;
    while i < n {
        self.add(value);                 // <-- moved, no clone
        { let __java_old_value = i; i += 1; __java_old_value };
    }
}
```

rustc output:

```
error[E0382]: use of moved value: `value`
    --> loopmove.rs:1686:22
1685 |         while i < n {
     |         ----------- inside of this loop
1686 |             self.add(value);
     |                      ^^^^^ value moved here, in previous iteration of loop
```

## Root cause

The move analysis is not a dataflow analysis. It is a syntactic scan of the
*textual suffix* of the enclosing statement list.

- `src/lower.rs:4359` — per statement in a block:
  ```rust
  let mut later_read_names = statement_suffix_read_names(&block.statements[index + 1..]);
  if let Some(inherited_later_read_names) = block_ctx.later_read_names {
      later_read_names.extend(inherited_later_read_names.iter().cloned());
  }
  ```
- `src/lower.rs:4439` `statement_suffix_read_names` → `src/lower.rs:8000`
  `collect_stmt_read_names`, a plain syntax-directed walk.
- Consumers: `MoveUseContext` at `src/lower.rs:15464` (`has_later_read`,
  `has_later_or_repeated_current_read`, …), feeding `CloneRecoveryDecision`
  at `src/lower.rs:15445`.

"Reads that occur later in the source text, plus reads later in enclosing
blocks" is not the same set as "reads that can execute after this program
point". The two differ exactly on **back edges**. When lowering the body of a
`while`/`for`/`do-while`, the loop's own body and condition are not part of any
suffix that is consulted, so a use at the top of a loop body is treated as the
last use of that binding.

The same defect applies to `later_mutated_names`
(`statement_suffix_mutated_names`) and, by construction, to
`exclusive_terminal_branch_read_names`.

## Why this is a machinery gap rather than a patch site

Special-casing loops in the suffix scan (e.g. "also union the whole enclosing
loop body") would fix this example but is still not the right relation — the
same suffix model is wrong for `continue`, labelled `break`, `switch`
fallthrough, exception edges into `catch`/`finally`, and early `return` inside
a branch. Each of those would become another special case in the same scan.

The property actually needed is standard **backward liveness (last-use)
analysis over a control-flow graph**: `live_out(n) = ⋃ live_in(s) for s ∈ succ(n)`,
`live_in(n) = use(n) ∪ (live_out(n) − def(n))`, solved to a fixpoint so back
edges are included by construction. A value needs a clone at a consuming use
iff it is live-out at that point.

## Proposed machinery

1. Build a per-callable CFG from the Java IR, including loop back edges,
   `break`/`continue`/labelled targets, `switch` case edges, and exception
   edges into `catch`/`finally`.
2. Key the analysis on **binding identity**, not on `String` names. The current
   sets are `HashSet<String>`, so a name cannot distinguish a field from a
   shadowing local, a loop variable from a same-named variable in a sibling
   scope, or two distinct bindings across scopes. See
   [0006](0006-move-analysis-is-five-approximations-of-one-relation.md).
3. Run backward liveness to a fixpoint; expose `is_live_after(binding, program_point)`.
4. Replace `later_read_names`, `later_mutated_names`,
   `exclusive_terminal_branch_read_names`, `catch_future_use_names`, and
   `current_stmt_read_counts` with queries against that one result — they are
   five approximations of one relation (see 0006).

`src/graph.rs` and `src/lattice.rs` already exist and provide the shape for a
fixpoint solver; neither is currently reachable from `src/lower.rs`. See
[0008](../0008-inference-machinery-does-not-reach-lowering.md).

## Notes for whoever picks this up

- `later_read_names` is `Option<&HashSet<String>>` on `LowerCtx`
  (`src/lower.rs:218`). When it is `None` the analysis silently degrades to
  "no later reads", which is the unsound direction. A CFG-based result should
  be non-optional.
- Line numbers are from the working tree at the time of writing; `src/lower.rs`
  is under concurrent edit, so prefer locating by symbol name.
