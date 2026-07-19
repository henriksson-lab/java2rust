# Pipeline

## Stages

1. Parse Java into a source AST.
2. Normalize Java syntax into a Java IR.
3. Resolve packages, imports, scopes, fields, methods, overload candidates, and type names.
4. Attach semantic facts: types, nullability, mutability, constructor initialization, hierarchy, and carrier facts.
5. Lower Java IR into Rust IR using transformation rules.
6. Apply explicit structural rewrites where local expression emission cannot preserve semantics.
7. Emit Rust code from Rust IR.
8. Format and run validation gates.

## Java IR

Java IR should remove parser incidental detail while preserving Java semantics. Examples:

- Desugar `for` into an explicit loop form with initializer, condition, update, and body.
- Represent enhanced `for` as a distinct iteration form until carrier facts choose Rust iteration.
- Represent constructor delegation, `super(...)`, field initializers, and instance initializer blocks explicitly.
- Preserve source spans and Java FQNs.

## Rust IR

Rust IR should not be raw strings. It should represent:

- Modules, structs, enums, traits, impl blocks, functions, statements, expressions, and attributes.
- Types as Rust semantic nodes, including references, ownership wrappers, `Option`, `Result`, trait objects, and generics.
- Required imports and runtime dependencies.
- Diagnostics/provenance comments.

## Fact Discipline

Rendering and typing must consume the same facts. If a method return type drives emission, the same resolved method fact must drive later chained-call typing. Fallback facts must carry lower confidence so operation surfaces can reject them when necessary.

## Transformation Rule Discipline

Every transformation rule must follow
`docs/transformation-rule-contract.md`. In particular, rule documentation must
start with an **Ideal Rust Target** section before the **Implementation Rule**.
The target section describes the Rust shape we would want by hand; the
implementation section then explains which facts and plan nodes select that
shape. Lowering rules, standard-library mappings, coercions, object
representation decisions, dependency boundaries, exception rules, and fallback
rules all count as transformation rules.

This is checked per rule, not per epic. If a feature slice changes several
mappings, each mapping needs its own **Ideal Rust Target** before its own
**Implementation Rule**, unless the plan explicitly states that those mappings
share the same target Rust shape and fact requirements.

This requirement applies to the broader transpiler plan, not only the
GATK-focused roadmap. A design note, benchmark report, stdlib mapping table,
dependency-boundary plan, or feature checklist may mention corpus evidence
first, but the transformation rule itself is not implementation-ready until the
tracked rule entry states the ideal Rust target before any implementation
mechanics. When in doubt, add or normalize the rule in
`docs/transformation-rule-contract.md` format before coding.

For the broader transpiler plan, this is a hard readiness gate. A rule may be
motivated by a corpus failure or a known Java idiom, but the tracked rule entry
must first state the ideal Rust API, expression, type, ownership/nullability
shape, dispatch form, or fallback diagnostic we would want to maintain by hand.
Only after that target is explicit should the rule describe the semantic facts,
solver constraints, plan nodes, or renderer behavior that select it.

## Method Resolution Facts

The resolver records overload sets before lowering. A resolved call fact contains:

- owner FQN,
- Java method name,
- erased parameter types,
- chosen declaration,
- static/instance flag,
- varargs flag,
- return type after owner generic substitution,
- ambiguity reason when no single declaration wins.

Lowering consumes this fact to choose the Rust function name and argument coercions. Emission must not recompute overload resolution from rendered Rust types.

## Validation Gates

Early gates:

- Fixture inventory checks: every Java example has a documented expected shape.
- Rust IR pretty-print snapshots once implementation begins.
- `cargo fmt`/`cargo check` for generated Rust once code exists.
- Semantic report checks for unsupported constructs and fallback use.
