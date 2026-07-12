# Ownership And Mutability

## Default Strategy

Start with value ownership for simple, acyclic data and explicit references for method receivers:

- Instance methods use `&self` unless they mutate fields or expose mutable aliases.
- Mutating methods use `&mut self`.
- Locals are `let` unless reassigned, then `let mut`.
- Primitive and immutable scalar copies use Rust `Copy`.

## Java Aliasing

Java object variables are references. Rust owned moves can silently diverge from Java aliasing. The translator must classify object values by use:

- Single-owner, no observed alias mutation: owned value is acceptable.
- Shared immutable aliasing: `Rc<T>` or references may be needed depending on lifetime.
- Shared mutable aliasing: `Rc<RefCell<T>>` or a structural rewrite is required.
- Cross-thread shared mutation: `Arc<Mutex<T>>` or explicit unsupported diagnostic.

## Clone Policy

Do not insert unmarked clones as a convenience. A clone is allowed only when:

- The source semantics are value-like, such as strings or immutable objects, or
- A rule proves the clone is unobservable, or
- The output includes a diagnostic/provenance marker requiring audit.

## Borrow Overlay

Borrowing is slot-sensitive:

- Passing to `&T` borrows the value.
- Passing to `&mut T` requires an exclusive mutable slot.
- Reading from `Option<T>` requires pattern matching, `as_ref`, `as_mut`, or a proven unwrap.
- Collection lookups usually return borrowed values; Java value semantics may require cloning only under explicit rule.

## Structural Rewrite Trigger

When local expression emission would create borrow conflicts or move-after-use errors, record a rewrite-needed fact instead of patching strings. Examples include nested mutating calls, temporaries whose lifetime must be extended, and aliasing created by chained lookups.

## Lambda And Local Class Captures

The first stream lambda slice supports only expression lambdas whose parameter is the stream item and whose body does not capture mutable Java locals. For `List<Integer>.stream().filter(...).mapToInt(...).sum()`, lowering uses borrowed iteration:

- `values.iter()` produces borrowed elements.
- `filter` receives a borrowed item reference from Rust's iterator adapter.
- `mapToInt` dereferences the borrowed item and returns a copied primitive.

No captured Java local is cloned, moved, or wrapped in this slice.

Before lowering local classes, anonymous classes, or escaping lambdas:

- `resolve` must record captured locals and captured `this`.
- `types` must record whether captured locals are effectively final.
- `lower` must choose explicit fields or closure captures from those facts.
- Mutable captured locals require `Rc<RefCell<T>>` or a structural rewrite unless a lexical borrow is proven not to escape.
- Captured object references must not be silently cloned; any clone must come from the clone policy above.

Anonymous and local classes remain deferred until these capture facts exist.
