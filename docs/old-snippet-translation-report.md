# Old Snippet Translation Report

Source corpus: `/home/mahogny/github/claude/java2rust/tests/corpus`

Run date: 2026-07-13

Command shape:

```text
target/debug/new-java2rust <old-snippet.java>
rustc --crate-type lib <generated.rs>
```

Temporary generated output was written under
`/tmp/new_java2rust_old_snippets`.

## Summary

The old corpus has 42 Java snippets.

- 27 translated without translator diagnostics.
- 15 emitted `parse_no_class` because they are Java fragments rather than
  complete top-level compilation units.
- 19 generated Rust files compiled as standalone Rust libraries.
- 8 complete-class snippets translated without diagnostics but generated Rust
  that did not compile.
- Fragment snippets with `parse_no_class` often produced empty Rust that
  trivially compiled, so their rustc result is not meaningful.

## Diagnostic Snippets

These old inputs are fragments and currently fail the top-level-class parser
guard:

- `decl_self_param.java`
- `decl_static_method.java`
- `for_complete.java`
- `for_empty.java`
- `for_no_cond.java`
- `for_only_inc.java`
- `int_const.java`
- `new_class.java`
- `new_class_arg.java`
- `snake_method_call.java`
- `snake_method_decl.java`
- `snake_test_anno.java`
- `snake_test_anno_expected.java`
- `snake_test_anno_nl.java`
- `snake_var_decl.java`

Do not import these verbatim. If they are useful, wrap or rewrite them as small
complete Java classes with one rule pressure each.

## Clean Translation And Standalone Compile

These snippets translated without diagnostics and generated standalone-compiling
Rust:

- `comment_interface.java`
- `comment_package.java`
- `decl_enum_blocks.java`
- `decl_field.java`
- `decl_field_init.java`
- `decl_local.java`
- `decl_local_init.java`
- `decl_method_param.java`
- `gen_fields.java`
- `gen_labeled.java`
- `gen_strcat.java`
- `gen_switch.java`
- `gen_ternary.java`
- `gen_whilecast.java`
- `int_decl.java`
- `int_parameter.java`

This does not prove semantic quality. For example, `decl_enum_blocks.java`
compiled because enum contents were effectively dropped. Import decisions should
still be based on whether the snippet exercises a responsibility we want.

## Clean Translation But Bad Rust

These snippets translated without diagnostics but generated non-compiling Rust:

- `gen_calc.java`: method mutates `self.total` through `&self`; also emits
  `v / 2` for `f64`, which needs numeric promotion.
- `gen_nested_if.java`: unresolved local names/functions in the snippet.
- `gen_sync.java`: emits unresolved `Object`; synchronized body is dropped.
- `int_arrays_1.java`: Java array initializer lowers as a Rust block and
  `+2.` is emitted with unsupported Rust leading plus.
- `int_arrays_2.java`: same as above for nested arrays.
- `int_expr_1.java`: mixed `int * double` emits `10 * 1.5f64`.
- `int_expr_2.java`: same mixed numeric arithmetic issue.
- `int_for_expr.java`: loop condition compares `i32` to `f64`.
- `int_for_expr2.java`: combines array coercion, external owners, unresolved
  source/assert facts, and indexing.
- `stackoverflow_1.java`: broad real-world stress snippet with `String.format`,
  exceptions, constants, and unresolved receiver facts.
- `string_exp_decl.java`: unresolved local `i`.

## Import Candidates

Add minimized fixtures from the old snippets only when they cover a gap in the
current suite.

Recommended near-term imports:

- Done: wrapped `for_empty.java`, `for_no_cond.java`, and `for_only_inc.java`
  into `examples/java/02-control-flow/ForOmittedParts.java`.
- Done: adapted `int_arrays_1.java` and `int_arrays_2.java` into
  `examples/java/05-types/ArrayInitializers.java`. The original array
  initializer syntax, array creation expressions, and unary-plus literals still
  need parser IR support, so the fixture uses array parameters and assignments
  to keep the no-raw-fallback guard intact.
- Done: added expected parser-gap fixtures under `examples/parser-gaps` and
  tests in `tests/parser_gaps.rs` for array initializer literals, array
  creation expressions, and unary-plus literals.
- Done: minimized `gen_calc.java` into
  `examples/java/04-objects/FieldWriteReceiverMutation.java`.
- Done: wrapped `new_class.java` and `new_class_arg.java` into
  `examples/java/04-objects/ConstructorExpressions.java`.
- Done: wrapped `snake_method_decl.java`, `snake_method_call.java`, and
  `snake_var_decl.java` into `examples/java/03-methods/NamingPolicy.java`.
- Done: copied passing-but-weak cases into focused fixtures:
  `LabeledBreak.java`, `EnumConstantsWithFields.java`,
  `StaticFinalFields.java`, `MixedNumericArithmetic.java`,
  `SynchronizedBlock.java`, and `StringFormatAndThrow.java`.

Lower priority:

- `gen_sync.java` should become a guardrail fixture only when we define the
  responsibility for `synchronized`; until then it would just pin an
  unsupported diagnostic.
- `snake_test_anno*.java` should wait until annotation policy exists.
- `stackoverflow_1.java` is useful as a stress input, but it is too broad for
  the minimized fixture suite. Split it into smaller fixtures for
  `String.format`, exception construction, constants, and unresolved member
  facts instead.
- Comment-only fixtures should wait until we decide whether comment preservation
  is in scope.

## Result Table

| File | Translator result | Rustc result |
|---|---|---|
| `comment_interface.java` | clean | compile |
| `comment_package.java` | clean | compile |
| `decl_enum_blocks.java` | clean | compile |
| `decl_field.java` | clean | compile |
| `decl_field_init.java` | clean | compile |
| `decl_local.java` | clean | compile |
| `decl_local_init.java` | clean | compile |
| `decl_method_param.java` | clean | compile |
| `decl_self_param.java` | `parse_no_class` | not meaningful |
| `decl_static_method.java` | `parse_no_class` | not meaningful |
| `for_complete.java` | `parse_no_class` | not meaningful |
| `for_empty.java` | `parse_no_class` | not meaningful |
| `for_no_cond.java` | `parse_no_class` | not meaningful |
| `for_only_inc.java` | `parse_no_class` | not meaningful |
| `gen_calc.java` | clean | fail |
| `gen_fields.java` | clean | compile |
| `gen_labeled.java` | clean | compile |
| `gen_nested_if.java` | clean | fail |
| `gen_strcat.java` | clean | compile |
| `gen_switch.java` | clean | compile |
| `gen_sync.java` | clean | fail |
| `gen_ternary.java` | clean | compile |
| `gen_whilecast.java` | clean | compile |
| `int_arrays_1.java` | clean | fail |
| `int_arrays_2.java` | clean | fail |
| `int_const.java` | `parse_no_class` | not meaningful |
| `int_decl.java` | clean | compile |
| `int_expr_1.java` | clean | fail |
| `int_expr_2.java` | clean | fail |
| `int_for_expr.java` | clean | fail |
| `int_for_expr2.java` | clean | fail |
| `int_parameter.java` | clean | compile |
| `new_class.java` | `parse_no_class` | not meaningful |
| `new_class_arg.java` | `parse_no_class` | not meaningful |
| `snake_method_call.java` | `parse_no_class` | not meaningful |
| `snake_method_decl.java` | `parse_no_class` | not meaningful |
| `snake_test_anno.java` | `parse_no_class` | not meaningful |
| `snake_test_anno_expected.java` | `parse_no_class` | not meaningful |
| `snake_test_anno_nl.java` | `parse_no_class` | not meaningful |
| `snake_var_decl.java` | `parse_no_class` | not meaningful |
| `stackoverflow_1.java` | clean | fail |
| `string_exp_decl.java` | clean | fail |
