# Stream Mapping Boundary Refactor Plan

## Goal

Refactor stream lowering so Java stream API mapping, ownership/value-category decisions, and slot coercion stay in separate places.

The immediate reason is `findFirst()`: it naturally produces a borrowed optional element from a Rust iterator, but whether that becomes `.cloned()`, `.copied()`, a borrowed return, or no adapter must be decided by coercion/slot rules, not by the stream mapper.

## Problem To Avoid

Do not encode rules like this in stream lowering:

```rust
findFirst over Vec<String> -> .find(...).cloned()
```

That combines several responsibilities:

- Java stdlib API semantics: `findFirst` returns an optional stream element.
- Rust iterator carrier: `.iter()` yields borrowed elements.
- Return slot requirement: method returns `Option<String>`.
- Ownership adaptation: `Option<&String>` to `Option<String>` needs `.cloned()`.

If encoded as one mapping, every future source of `Option<&T>` will need another special case.

## Responsibility Boundaries

- `stdlib`
  - Owns Java API recognition and semantic operation planning.
  - May say: this is a stream chain ending in `FindFirst`.
  - Must not emit `.clone()`, `.cloned()`, `.copied()`, `.to_string()`, `Rc`, `RefCell`, or unwrap-like ownership adapters.

- `types`
  - Owns actual expression type and value category facts.
  - Example: `FindFirst` over `values.iter()` has actual type/category equivalent to `Option<&String>`.

- `coercion`
  - Owns adapting actual expression to a target slot.
  - Example: `Option<&String>` in `Return Option<String>` slot becomes `.cloned()`.

- `lower`
  - Converts a validated stream plan to Rust IR/expression surface.
  - Applies coercion only through `lower_expr_for_slot`.
  - Does not locally invent ownership repairs.

- `emit`
  - Prints Rust syntax only.

## Stream Plan Model

Introduce a small stream operation plan before adding more terminals.

Suggested data:

```rust
pub struct StreamPlan {
    pub source: StreamSource,
    pub ops: Vec<StreamOp>,
    pub terminal: StreamTerminal,
    pub element: StreamElement,
}

pub enum StreamSource {
    IterBorrowed { expr: Expr, element_ty: JavaType },
}

pub struct StreamElement {
    pub ty: JavaType,
    pub value_category: ValueCategory,
}

pub enum ValueCategory {
    Owned,
    Borrowed,
    Copy,
}

pub enum StreamOp {
    Filter(LambdaPlan),
    Map(LambdaPlan),
    MapToPrimitive {
        primitive: PrimitiveType,
        lambda: LambdaPlan,
    },
}

pub enum StreamTerminal {
    Sum { primitive: PrimitiveType },
    CollectToList,
    FindFirst,
    AnyMatch(LambdaPlan),
}

pub struct LambdaPlan {
    pub param_name: String,
    pub body: Expr,
    pub captures: Vec<Capture>,
}
```

The exact module can start in `lower` if kept private, but the target home is `stdlib` once the plan is shared by type inference and lowering.

## Actual Type And Slot Coercion

Add a value-category-aware actual type model sufficient for stream terminals.

First useful cases:

- `Sum(i32)` actual: `i32`.
- `CollectToList` actual: `Vec<T>`.
- `AnyMatch` actual: `bool`.
- `FindFirst` over borrowed iterator actual: `Option<&T>`.

Then coercion handles:

- `Option<&T>` to `Option<T>` by `.cloned()` when `T` is clone/value-like.
- `Option<&T>` to `Option<&T>` with no adapter.
- `&T` to `T` by copy only for `Copy` primitives.
- `&String` to `String` by clone only through a documented clone/value-like rule.

## Refactor Steps

### 1. Create Stream Plan Builder

Replace direct raw-string stream-chain recognition in `lower` with a builder:

```text
Expr::Call chain -> Option<StreamPlan>
```

The builder may still support only current compiling fixtures:

- `StreamsUse.java`
- `StreamsCollect.java`

But it should return semantic stream operations rather than final Rust text.

### 2. Gate Lambdas With Capture Facts

Use existing capture facts:

- Item-only lambdas with no captures are lowerable.
- Capturing lambdas are rejected until capture lowering is documented.

The stream plan stores lambda capture facts; it does not ignore them.

### 3. Lower Plan To Rust Expression And Actual Type

Create a small result type:

```rust
pub struct LoweredExpr {
    pub expr: RustExpr,
    pub actual: LoweredType,
}
```

For the first refactor, this can be private to `lower` and used by stream expressions only.

### 4. Route Slot Adaptation Through Coercion

When a stream expression is returned from a method, `lower_expr_for_slot` must:

1. Build/lower the stream plan.
2. See the actual lowered type/category.
3. Ask coercion to adapt to the return slot.

No stream terminal implementation may append `.cloned()` directly.

### 5. Add Guardrails

Add tests that enforce boundaries:

- Stream plan for `findFirst` reports borrowed optional element.
- Coercion, not stream mapping, inserts `.cloned()` for `Option<&String>` to `Option<String>`.
- A source scan test rejects these strings in stream mapping code:
  - `.clone(`
  - `.cloned(`
  - `.copied(`
  - `Rc<`
  - `RefCell`

The scan can be narrow at first, focused on the stream mapping helpers.

### 6. Re-implement Current Stream Fixtures Through The Plan

Before adding `findFirst`, make sure existing behavior still passes:

- `StreamsUse.java`
- `StreamsCollect.java`

Generated Rust can stay the same, but implementation must pass through `StreamPlan`.

### 7. Add `StreamsFindMatch`

Only after the plan boundary exists:

- `filter(...).findFirst()`
- `anyMatch(...)`
- `String.length()`
- `String.isEmpty()`

`findFirst` should lower to a borrowed optional actual value and rely on coercion for the final method return type.

## Tests

Add or update tests for:

- Current `StreamsUse` behavior.
- Current `StreamsCollect` behavior.
- Stream plan construction for `findFirst`.
- Coercion from optional borrowed element to owned optional return.
- Boundary scan for forbidden ownership adapters in stream mapping.

## Completion Criteria

This refactor is complete when:

- Current stream fixtures still compile and behavior tests pass.
- Stream lowering builds a semantic plan before Rust text/syntax emission.
- Ownership adapters are not emitted by stream mapping helpers.
- Slot coercion owns `Option<&T>` to `Option<T>` adaptation.
- `StreamsFindMatch.java` can be implemented without adding a `findFirst over Vec<String> -> cloned` special case.
- `cargo fmt` and `cargo test` pass.
