# 0073 — Remaining Java grammar coverage was partly stale and partly unsafe

Status: **fixed/audited** — every substantive parser item in
`AUDIT-BACKLOG.md` A1–A5 now has an executable disposition. Record and unnamed
patterns fail loudly instead of being interpreted as fabricated type names.

## Findings

- `yield_statement` is structurally handled inside colon-form switch
  expressions. A standalone `yield` remains an explicit unsupported statement;
  it is not legal Java outside that context.
- `switch_rule` and ordinary `type_pattern` have structured parser paths.
  Record/unnamed destructuring patterns do not fit the current `Expr::InstanceOf`
  IR. They previously reached type parsing through the pattern text. The parser
  now emits one precise `Expr::Unsupported` for that whole expression. Preview
  string-template expressions also take the central explicit unsupported path.
- Array, annotated, generic, wildcard, and scoped types are structurally parsed.
  Existing parser-tree tests cover annotated generic arguments and nested type
  shapes.
- A Java module descriptor contains no translatable type and produces the
  existing `parse_no_class` diagnostic instead of disappearing silently.
- A receiver parameter is declaration metadata, not a runtime parameter. The
  parser correctly excludes it while retaining ordinary parameters.

## Annotation policy

Declaration and type-use annotations are intentionally erased after their
underlying type is parsed. The Java language assigns no general nullability
meaning to names such as `Nullable` or `NonNull`; consuming them requires a
configured annotation vocabulary and precedence policy. The repository's Java
inputs currently contain `@Override`, one `@Deprecated`, and an annotation
declaration, but no nullability annotations. Override behavior is already
derived from resolved signatures, so erasing `@Override` loses no translator
semantics on the audited inputs.

If annotation-driven nullability becomes a product requirement, it should enter
as a configured fact provider rather than hard-coding spelling guesses in the
parser.

## Verification

- `parser_gaps::parses_yield_switch_expression_without_fallbacks`
- `parser_tree_sitter::parses_annotated_generic_type_arguments_as_structured_types`
- `parser_gaps::record_pattern_instanceof_is_an_explicit_unsupported_expression`
- `parser_gaps::string_template_expression_is_an_explicit_unsupported_expression`
- `parser_gaps::module_descriptor_is_reported_instead_of_silently_ignored`
- `parser_gaps::receiver_parameter_is_metadata_not_a_runtime_parameter`
- full `parser_gaps`: 46 passed
