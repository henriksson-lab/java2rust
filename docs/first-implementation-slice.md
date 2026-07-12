# First Implementation Slice

## Goal

Prove the pipeline from Java source to Rust output for a tiny, coherent subset.

## Included

- One top-level class per file.
- Primitive fields.
- `String` fields.
- One or more constructors without delegation.
- Simple instance methods and static methods.
- Local variables and reassignment.
- Arithmetic, comparisons, `if`, `while`, and simple classic `for`.
- Direct method calls within the same class.

## Excluded

- Inheritance.
- Interfaces.
- Generics.
- Exceptions.
- Anonymous/local/inner classes.
- Streams.
- External dependencies.
- Complex stdlib rewrites beyond `String` basics.

## Candidate Examples

Start with:

- `examples/java/01-expressions/ExpressionPrecedence.java`
- `examples/java/02-control-flow/LoopsAndBranches.java`
- `examples/java/03-methods/MethodsAndOverloads.java` with overloads disabled or diagnosed in the first pass.
- `examples/java/04-objects/SimpleClass.java`

## Acceptance

The first implementation slice is acceptable when it emits formatted, compiling Rust for the included subset and emits structured diagnostics for every excluded construct encountered in the example suite.

