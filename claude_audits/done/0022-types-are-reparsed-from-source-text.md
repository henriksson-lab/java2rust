# 0022 — Types are re-parsed from source text, discarding the parse tree

Status: **validated by inspection** (structural; direct cause of parts of
[0017](0017-generic-bounds-dropped-and-method-type-params-undeclared.md) and
[0021](0021-instanceof-pattern-variable-not-bound.md))
Severity: medium-high — makes a whole dimension of the grammar unreachable
Kind: parser architecture

## Observation

Every Java type is converted by throwing away the tree-sitter parse tree and
re-parsing the node's raw text:

```rust
fn parse_type_node(node: tree_sitter::Node, source: &str) -> JavaType {
    parse_type_text(text(node, source))
}
```

`parse_type_text` is ~57 lines of string surgery:

```rust
while let Some(rest) = src.strip_prefix("final ") { src = rest.trim_start(); }
if let Some(inner) = src.strip_suffix("[]") { return JavaType::Array(…); }
…
if let Some(rest) = src.strip_prefix("? extends ") { … }
if let Some(open) = src.find('<') {
    let close = matching_angle_close(src, open)?;
    let inner = src[open + 1 .. src.rfind('>')…].trim();
    … split_top_level_commas(inner) …
}
```

The grammar provides structured nodes for exactly this — `array_type`,
`annotated_type`, `type_arguments`, `type_parameter`, `type_bound`,
`wildcard`, `scoped_identifier`, `integral_type`, `floating_point_type`,
`void_type`, `type_list` — and **none of them is referenced anywhere in
`src/parser.rs`**.

## Why it matters

1. **It makes constructs invisible.** Anything the text parser does not
   anticipate is silently reshaped rather than rejected. `String s` (an
   `instanceof` type pattern) is handed to `parse_type_text` as text, the
   identifier is discarded, and the binding is lost — that is
   [0021](0021-instanceof-pattern-variable-not-bound.md) exactly.
2. **It cannot see bounds.** `type_parameter` and `type_bound` are the nodes
   carrying `T extends Comparable<T>`. Because nothing reads them, class and
   method type-parameter bounds have no path into the IR at all — the root
   cause behind [0017](0017-generic-bounds-dropped-and-method-type-params-undeclared.md),
   which is ~36% of the biojava corpus errors.
3. **It is fragile in the usual ways.** Annotations inside types
   (`List<@NonNull String>`), nested generics with `>>`, arrays of generics,
   varargs `T...`, and qualified nested types all have to be re-derived by
   string matching that the parser already did correctly once.

## The pattern

This is [0007](../0007-rust-ir-is-mostly-opaque-strings.md) at the *input* end of
the pipeline. The output end makes decisions by inspecting rendered Rust text;
the input end makes decisions by inspecting raw Java text. In both cases a
structured representation exists and is bypassed, and in both cases the cost is
that some information becomes unrecoverable rather than merely inconvenient.

Worth stating explicitly because the convergence plan
(`docs/rule-generalization-convergence-plan.md`) addresses the output end only.
The same principle applies here, and this end is *cheaper* to fix: the
structured data is already sitting in the parse tree.

## Fix direction

1. Dispatch `parse_type_node` on `node.kind()` — `array_type`, `annotated_type`,
   `generic_type`/`type_arguments`, `scoped_identifier`, the primitive type
   nodes, `wildcard` — reading child nodes and fields instead of text.
2. Add `type_parameter` / `type_bound` handling so declared bounds reach the IR.
   This is the prerequisite for 0017 and cannot be worked around downstream.
3. Keep `parse_type_text` only for genuinely text-sourced types (external
   manifests, `javalibs` signatures), not for source parsing.

Step 2 is the one with measurable corpus payoff; step 1 is what makes step 2
possible without adding more string cases.

## Suggested check after fixing

Re-run the grammar-coverage diff (see `AUDIT-BACKLOG.md`) — the type-level node
kinds should move from "never referenced" to "handled", which is a concrete
completion signal for this issue.

---

## Update 2026-08-03 — measured, and the last source-route text paths are gone

Status: **closed for the source route.** `parse_type_text` survives, correctly,
for the input it was always right for: `javalibs` signatures and external pack
manifests.

### The measurement first, as this file's own advice requires

`parse_type_text` was instrumented and run over the corpus (80 files) plus a
dense type-shape probe (nested generics, `Map.Entry<K,V>`, C-style declarator
arrays, varargs, multi-dimensional arrays, four cast shapes):

| call site | hits |
|---|---|
| `parse_type_node`'s `_ =>` fallthrough | **0** |
| `parse_named_type_node`'s `raw.contains('<')` | **0** |
| `parse_cast_type_node`'s text preference | 4 of 4 cast shapes |
| `javalibs` / external manifests | **4,913,073** |

So the tree dispatch added in the previous round already covers every type-level
node the corpus produces. Nearly five million text parses remain, and every one
of them is the legitimate use.

### The cast text path was covering for a real bug

`parse_cast_type_node` preferred a text re-parse whenever that produced an array
or a generic type. Printing both answers side by side showed why:

```
CASTTEXT|int[][]|tree=Some(Array(Primitive(Int)))     <-- one dimension lost
CASTTEXT|String[]|tree=Some(Array(String))            <-- agrees
CASTTEXT|List<String>|tree=Some(Named { .. })         <-- agrees
CASTTEXT|Map.Entry<String,Integer>|tree=Some(..)      <-- agrees
```

`int[][]` is **one** `array_type` node whose `dimensions` child carries both
pairs of brackets, not two nested `array_type` nodes. `parse_array_type_node`
wrapped exactly once, so it dropped a dimension — and the text preference was a
workaround for that, not a reason to parse types from text.

Fixed by `array_type_node_dimension_depth`, which counts `[` tokens among the
`dimensions` children. With that, the tree and the text agree on all four
shapes, and the text preference is deleted.

This is the general shape worth carrying: **a text fallback that never fires is
dead weight, and one that always fires is hiding a bug in the structured path.**
Instrumenting both answers distinguishes the two in minutes; neither is visible
from the call site.

### What was deliberately not done

The `_ =>` fallthrough is kept rather than converted to a diagnostic. It measured
0 hits, but the sample is 248 files and the real corpora (FastQC, htsjdk,
biojava) are far larger, so turning a never-taken path into a hard failure trades
a measured non-problem for an unmeasured risk. It is documented with its
measurement instead.

### Effect

`focused_package_offline_cargo_uses_local_ndarray_shim` (`tests/batch_translation.rs`)
passes for the first time — a 2-D array reaching `ndarray::Array2` needs the
dimension the tree path was dropping. No other suite moved.

## Follow-up 2026-09-12 — parameter `java.io` preservation no longer reparses

`parse_params_node` had one remaining live source-route reparse: every formal
parameter text was fed through `parse_param_text_fallback` before the tree type
node, solely so fully-qualified `java.io.*Stream` parameter names could survive
suffix normalization. That policy now lives in
`java_type_name_from_parse_tree_node`, next to the rest of tree-owned type-name
normalization, and the parameter walker uses its `type` child directly.

The documented defensive fallbacks remain: text parsing still handles genuinely
text-sourced signatures and the parser's measured zero-hit safety nets. Focused
checks:
`parser_gaps::preserves_explicit_java_io_stream_parameter_type_names` and
`parser_tree_sitter` pass.
