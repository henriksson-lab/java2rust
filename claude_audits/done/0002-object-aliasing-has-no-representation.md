# 0002 — Java object aliasing has no representation; aliases become clones

Status: **fixed/fenced for the audited aliasing and anonymous-object surface**.
The closure verification at the end of this file supersedes historical
"remaining work" notes retained below as implementation chronology.
Severity: high — the original mutable-alias oracle is now promoted and passing,
including direct locals, field/array storage, named callback parameters, stored
actions, parameterized anonymous functional-interface adapters, and narrow
stateful anonymous-object adapters.
Explicit fences remain for anonymous objects beyond the bounded interface/class
adapter grammars and borrowed-return trait surfaces through cell/lock handles
that cannot safely outlive a shared-handle guard. Read-only
`if`/`return` field access is now within the object-backed slice.
The interface adapter covers initialized primitive or `String` locals, their
reassignment, and bounded `if`/`else` local updates around receiver-field
writes. Broader statement/control-flow composition still needs the general
statement-rebinding and interior-cell assignment path.
The object-backed interface path also handles ordered top-level receiver-field
writes separated by ordinary local statements, with value or void returns. A local variable
initialized directly from that anonymous object receives a boxed trait slot;
[`repro/AnonymousInterfacePostWriteLocal.java`](../repro/AnonymousInterfacePostWriteLocal.java)
and `fixtures::anonymous_interface_method_reads_local_after_field_write`
both produce `66`. Two-field Java/Rust oracles produce `1818` for a value
method and `10` for a void method. A recursive interface-adapter statement
path now supports `if`/`else` branches with receiver writes and branch-local
or outer-local reassignment; Java/Rust oracles produce `688` and `571`.
Field-read projection also preserves longer field names and string literals
(`60`). The recursive interface path also preserves loop-carried receiver writes
in an unlabeled `while`, including field reads in the condition, mutable outer
loop locals, and `break`/`continue`; Java/Rust oracles produce `810` and `566`.
The same path now preserves unlabeled `do`/`for` receiver writes, including
`continue`-time condition checks or updates (`566` and `1010`). Nested owned
`String` field replacement also matches Java (`455`). Ordinary call statements
in mutable and read-only interface methods preserve a captured `Rc<RefCell<_>>`
alias while reading receiver fields; Java/Rust oracles produce `405` and `632`.
Prefix/postfix increment and decrement statements on explicit `this` integer
fields also update the cell, with Java/Rust oracles at `355` and `311`. Labeled
`for`, `while`, and `do/while` loops in the mutable interface receiver path now
carry active break/continue targets through nested loops. The Java/Rust pairs
[`AnonymousInterfaceLabeledFor.java`](../repro/AnonymousInterfaceLabeledFor.java),
[`AnonymousInterfaceLabeledWhile.java`](../repro/AnonymousInterfaceLabeledWhile.java),
and [`AnonymousInterfaceLabeledDoWhile.java`](../repro/AnonymousInterfaceLabeledDoWhile.java)
agree on `1310`, `19419`, and `9409`, including outer `for` updates and outer
`do/while` condition checks on labeled `continue`. Scoped ordinary and labeled
blocks also preserve receiver state and a nested `break` target in the bounded
interface and class carriers: Java/Rust oracles
[`AnonymousInterfaceLabeledBlock.java`](../repro/AnonymousInterfaceLabeledBlock.java)
and [`AnonymousClassLabeledBlock.java`](../repro/AnonymousClassLabeledBlock.java)
agree on `9209` and `93`; the class case evaluates a `super` call in a composed
return after the block. Rust labeled blocks require explicit loop targets for a jump crossing
them, so both anonymous renderers now give surrounding loops Rust labels:
[`AnonymousLabeledBlockUnlabeledJumps.java`](../repro/AnonymousLabeledBlockUnlabeledJumps.java)
and its generated-Rust fixture agree on `1313` for unlabeled `break` and
`continue` inside a labeled block. Class-anonymous arithmetic returns now
recurse through nested operands and unary negation before lowering a selected
`super` call. The Java/Rust
[`AnonymousClassNestedSuperExpression.java`](../repro/AnonymousClassNestedSuperExpression.java)
pair agrees on `16`, `-8`, and two caught array-index failures, including
fallible `super` calls beneath both operator shapes. A mutable-interface method
can now initialize a named local from its shared anonymous field by cloning
the `Rc<RefCell<T>>` handle, then mutate through that local. The Java/Rust
[`AnonymousInterfaceReferenceLocal.java`](../repro/AnonymousInterfaceReferenceLocal.java)
pair now agrees on `6968`: adapter and caller observe the same node after
caller-side mutation, a local-alias accessor call, and a branch-local direct
field assignment. The eligibility walk tracks field-derived aliases by lexical
scope and drops the alias fact on reassignment; unit guard
`representation::tests::receiver_statement_reference_alias_loses_field_provenance_on_reassignment`
checks that boundary. The trivial accessor-setter path uses the ordinary
shared-field assignment lowering to obtain a mutable borrow and end the RHS
read borrow before the write. The mutable interface receiver path also admits
an enhanced `for` over a matching primitive-array method parameter, including
labeled `continue`/`break` and receiver-cell writes. Java/Rust
[`AnonymousInterfaceEnhancedFor.java`](../repro/AnonymousInterfaceEnhancedFor.java)
agree on `499` across two calls to the same adapter. A guarded direct `throw`
in a stateful interface-anonymous method now keeps the virtual `Result` ABI,
wraps normal returns, and propagates the exception without making the factory
fallible. [`AnonymousInterfaceDirectThrowContract.java`](../repro/AnonymousInterfaceDirectThrowContract.java)
prints `2` and `caught`, and the generated-Rust fixture
`anonymous_interface_direct_throw_promotes_virtual_contract` checks both
paths. The same object-backed interface path now wraps a read-only method's
fallible array return in `Ok` and appends `Ok(())` after a successful fallible
`void` receiver write. Java/Rust pairs
[`AnonymousInterfaceReadOnlyArrayResult.java`](../repro/AnonymousInterfaceReadOnlyArrayResult.java)
and [`AnonymousInterfaceFallibleVoidMutator.java`](../repro/AnonymousInterfaceFallibleVoidMutator.java)
agree on `17`/`caught` and `5`/`caught`, respectively. A read-only method with
a guarded explicit throw also follows the virtual result contract; Java/Rust
[`AnonymousInterfaceReadOnlyDirectThrow.java`](../repro/AnonymousInterfaceReadOnlyDirectThrow.java)
agree on `8`/`caught`. For a duplicate inherited method, the anonymous
override now equalizes fallibility with both parent contracts, including
calls through each parent upcast. Java/Rust
[`AnonymousDuplicateParentThrowContract.java`](../repro/AnonymousDuplicateParentThrowContract.java)
agree on `9`/`caught`. The focused fallibility-plan unit guard verifies both
parent edges and the infallible factory. A colliding generic-return contract
now keeps fallibility through the joined and both parent interface upcasts:
Java/Rust [`AnonymousGenericCollisionThrow.java`](../repro/AnonymousGenericCollisionThrow.java)
agree on `15`/`caught`. The generic-parameter collision now deduplicates its
target Rust method by effective specialized parameter types, while override
edges connect the anonymous method to both parent declarations. Java/Rust
[`AnonymousGenericParameterCollisionThrow.java`](../repro/AnonymousGenericParameterCollisionThrow.java)
agree on `2`/`caught` through the two parents; a direct joined-interface call
is ambiguous in Java and is deliberately not asserted. A class-anonymous
method can now clone a retained named field into a method local, write through
that shared local inside a branch, and still compose its return with a `super`
call. Java/Rust
[`AnonymousClassReferenceLocalAlias.java`](../repro/AnonymousClassReferenceLocalAlias.java)
agree on `75`, including caller observation of the mutation. The class
capability check tracks field-derived aliases by scope and drops that fact on
reassignment; unit guard
`representation::tests::class_statement_reference_alias_loses_field_provenance_on_reassignment`
checks that boundary. Its write operand grammar currently covers literals,
method parameters, and reads through those aliases. All 106 anonymous fixtures
and 1,096 library tests pass. Other throwing expression and special-case method-body shapes,
reference-local origins, enhanced-for
iterables, and expression-statement forms,
unmodeled label placements, and broader class-adapter statement rebinding
remain open.
For a named class reference retained by a class-anonymous field, a factory
method now accepts and forwards the planned `Rc<T>` handle instead of a
borrowed `&T`; the Java/Rust parameter-field fixture returns `733` and checks
reference identity before and after one field replaces its handle. Generic
interface specialization and broader anonymous-class reference expressions
remain open, so this does not close the audit.
Kind: missing representation in the object model

Codex follow-up 2026-09-17 synchronized-erasure refusal: Java
`synchronized (monitor) { ... }` statements and `synchronized` methods are no
longer lowered as ordinary blocks/methods. Both forms now emit an explicit
unsupported diagnostic and generated `compile_error!` until the reentrant
SharedSync monitor representation is implemented. Guards:
`idiom_gaps::{synchronized_statement_is_loud_until_monitor_semantics_exist,
synchronized_method_is_loud_until_monitor_semantics_exist}`. The same pass
also refuses `volatile` fields until Java atomic memory-order semantics are
implemented; guard:
`idiom_gaps::volatile_field_is_loud_until_atomic_memory_order_semantics_exist`.

Codex follow-up 2026-09-15 direct SharedLocal receiver companion pass:
implemented mutable receiver companions now get emitted even when the ordinary
lowered body is textually identical, because call sites with only an
`Rc<RefCell<T>>` handle still need a legal entry point. The identical-body
case delegates through one `borrow_mut().method(...)` call, avoiding the
previous `RefCell already borrowed` runtime panic from repeated
`__java_self.borrow_mut()` projections inside the generated companion body.
Guards:
`differential::aliasing_shared_local_fixtures_match_java_output`,
`differential::qualified_inner_capture_fixtures_match_java_output`, and the
adjacent `differential::super_call_fixtures_match_java_output` regression.

## Symptom

Java reference assignment that requires mutable sharing originally had no
representation as one object with two live mutable handles. The historical repro
below was first translated as a deep copy (`a.clone()`), so subsequent mutation
through one reference was not observed through the other. The direct mutable
`Node`/`Counter` shapes are now represented with `SharedLocal`
(`Rc<RefCell<T>>`) in the promoted `tests/differential/Aliasing.java` oracle.
The remaining defect is no longer this direct aliasing shape; it is the wider
surface where general anonymous object lowering or an unforwardable shared
trait surface is needed.

Codex follow-up 2026-09-18: a static-reachable outer can now construct a
non-static inner through the same `_with_outer` companion mechanism as a local
shared outer. The inner `__outer` field, constructor companion parameter, and
field projections all use the enclosing type's selected storage placement, so
the `Arc<Mutex<Outer>>` handle is preserved rather than being forced into the
old `Rc<RefCell<Outer>>` shape. Guard:
`idiom_gaps::static_reachable_outer_capture_uses_arc_mutex_and_preserves_outer_identity`.

Codex follow-up 2026-09-18 anonymous functional-interface pass: the closure
adapter is no longer limited to zero-argument methods. A source functional
interface with ordinary parameters and a return value now carries its declared
parameter/return slot shapes into the closure and generated trait adapter.
The promoted `AnonymousInterface.IntPredicate` fixture invokes its anonymous
`test(int)` implementation at runtime; bodies that mention anonymous `this` or
`super` remain explicitly unsupported because a closure cannot represent that
receiver identity.

Codex follow-up 2026-09-18 shared-capture callback intersection: a supported
parameterized anonymous functional interface assigned to a local interface now
plans that local as `Box<dyn Interface>` before lowering its initializer. Its
closure can mutate a captured `Rc<RefCell<T>>` alias without overlapping the
read and write borrows: field-assignment RHS evaluation is hoisted before the
mutable field place. Runtime guard:
`idiom_gaps::parameterized_anonymous_callback_preserves_sharedlocal_capture_identity`.

Codex follow-up 2026-09-18 stateful anonymous-interface slice: a one-method
functional-interface anonymous object with initialized instance fields and a
single read-only returned expression can now be a generated local adapter
struct. This represents `this.field` as real anonymous-instance state instead
of falsely treating it as a closure capture. At that initial slice, mutable
fields, control-flow bodies, `super`, and multi-method anonymous objects were
still unsupported; later entries below record implemented subsets. Guard:
`fixtures::stateful_anonymous_functional_interface_preserves_this_field`.

Codex verification 2026-09-19: object-backed lowering now precedes the
functional shortcut for eligible interface objects with state or multiple
methods. The fieldless two-method `CapturedPairOps` retains one factory capture
and returns `14` across its calls. The stateful one-method `CapturedCounter`
retains that capture alongside mutable `Cell<i32>` state, returning `3` and
then `7`. The [Java oracle](../repro/AnonymousObjectCapturePaths.java) produces
the same `14`, `3`, `7`, and aliased `7`; all 53 anonymous `fixtures` tests pass.
A fieldless one-method local interface still takes the closure route.

Codex verification 2026-09-19 class-alias follow-up: the anonymous variant of
the source hierarchy carrier now stores an `Rc<dyn BaseAnonymousDispatch>`.
Representation-aware body shapes keep carrier locals as carrier values rather
than misclassifying them as erased `Object`, and fresh anonymous returns use the
slot's shared placement. The executable
[Java oracle](../repro/AnonymousClassAlias.java) and
`fixtures::anonymous_source_class_assignment_shares_mutable_instance_state`
both return `12` after mutating through two aliases.

Codex verification 2026-09-19 static class follow-up: a base class reachable
from static storage selects the sync anonymous-dispatch payload
`Arc<Mutex<dyn Dispatch + Send>>`, while a local base keeps the `Rc` payload.
The static field initializer lowers the anonymous constructor into that
carrier rather than asking for an impossible closed-enum default. The
[Java oracle](../repro/AnonymousClassStatic.java) and
`fixtures::static_reachable_anonymous_class_keeps_dispatch_and_state` both
return `4` after mutation through the static base-typed field.

Codex verification 2026-09-19 static interface follow-up: a directly
process-wide-static interface type gains a `Send` trait bound and forwards
through `Arc<Mutex<Adapter>>` instead of an `Rc` carrier. The one-method
fieldless form also uses the object adapter, since its local closure carrier
is not thread-sendable. The
[Java oracle](../repro/AnonymousInterfaceStatic.java) and
`fixtures::{static_reachable_anonymous_interface_keeps_dispatch_and_state,
static_reachable_single_method_anonymous_interface_uses_send_object}` agree
on `4`, default-method `8`, and single-method `5`.

Codex verification 2026-09-19 transitive static interface follow-up: static
reachability now includes source interfaces held in a statically reachable
class's instance field. Such a class uses `Arc<Mutex<T>>` rather than `Arc<T>`
because its interface field can conceal mutable anonymous state; the interface
trait and adapter use `Send` and `Arc<Mutex<Adapter>>`. The
[Java oracle](../repro/AnonymousInterfaceTransitiveStatic.java),
`fixtures::statically_reachable_field_carries_anonymous_interface_safely`,
and `storage_strategy::constructor_mutation_tests::statically_reachable_interface_container_needs_sync_cell`
guard the `static -> class -> interface -> mutable anonymous object` path and
return `4`. That fixture alone does not prove collection, inheritance, or
external object-graph paths safe.

Codex verification 2026-09-19 inherited static-field follow-up: static
reachability now follows a class's superclass edge, and the sync-cell
requirement follows an interface field inherited from that superclass into
the flattened subclass representation. The
[Java oracle](../repro/AnonymousInterfaceInheritedStatic.java),
`fixtures::static_subclass_reaches_inherited_anonymous_interface_field`, and
`storage_strategy::constructor_mutation_tests::statically_reachable_subclass_inherits_interface_cell_requirement`
all cover `static -> subclass -> inherited interface field`; the Java and
Rust executions both return `4`. Collection and external object-graph paths
remain outside this evidence.

Codex verification 2026-09-19 collection-element follow-up: a directly
static-stored `Interface[]` element and a method-local
`ArrayList<Interface>` element now each retain a mutable anonymous object's
state. The [Java oracle](../repro/AnonymousInterfaceCollection.java) prints `4`
for both paths, matching
`fixtures::{static_array_element_carries_anonymous_interface_state,
local_generic_collection_element_carries_anonymous_interface_state}`. The
local-list `add` path now preserves the element's owned `Box<dyn Interface>`
slot instead of weakening it to `&dyn Interface` before `Vec::push`.
The process-wide static `ArrayList<Interface>` follow-up now lowers a
zero-argument collection initializer through the normal collection
constructor path and performs `List.add`, indexed insertion, `set`, and
`remove(index)` on the guarded persistent vector,
with the argument evaluated before locking. The
[Java oracle](../repro/AnonymousInterfaceStaticList.java) and
`fixtures::static_generic_collection_element_carries_anonymous_interface_state`
return `4` for aliasing and `307` after structural edits. That includes
`counters.add(counters.get(0))`, indexed insertion of another alias, and
mutations through the old interface values returned by `set` and
`remove(index)`. The owned boxed-interface argument clones its shared handle
rather than moving a borrowed `&Box` into the list; the returned values avoid
double-boxing. Other mutating static collection methods still need the same
persistent-storage treatment; the ordinary getter continues to clone the
vector for reads.

The first mutable stateful subcase is now also represented: an `int` field with
the exact `this.counter = expression; return this.counter;` functional-method
shape, including Java compound-assignment normalization such as `+=`, is stored
as `Cell<i32>`. This permits state to persist across calls while
the generated interface receiver remains `&self`; it is intentionally not a
claim of general mutable-field support. Guard:
`fixtures::stateful_anonymous_counter_preserves_mutable_this_field`.
Within that exact no-local-declaration body, ordinary bare reads (`counter`)
are structurally rebound to the anonymous field; a parameter of the same name
is rejected rather than guessed.

The same counter path now canonicalizes an explicit Java camel-case receiver
field before emitting Rust, so `this.totalValue += value; return totalValue;`
uses one `Cell<i32>` field named `total_value` rather than falling back or
emitting an invalid mixed-spelling projection. Guard:
`fixtures::stateful_anonymous_counter_rebinds_camel_case_source_field`.

Codex follow-up 2026-09-18 shared-trait forwarding: mutable interface methods
now forward through `Rc<RefCell<T>>` and `Arc<Mutex<T>>` rather than rejecting
the entire shared trait surface because direct `Rc<T>` has no `DerefMut`.
Generic source bounds receive a mutable borrow when a bound method is known to
mutate, so an aliased implementation can satisfy `T extends Counter` and retain
its Java identity. Guard:
`fixtures::shared_mutating_interface_implementation_satisfies_generic_bound`.
The same forwarding applies to a thread-reachable `SharedSync` implementation:
an `Arc<Mutex<T>>` alias satisfies the mutating generic interface bound and
observes the shared update. Guard:
`fixtures::shared_sync_mutating_interface_implementation_satisfies_generic_bound`.
Methods returning borrowed references now forward through direct `Rc<T>` and
`Arc<T>` handles, where the borrow is tied to the handle's `&self` lifetime.
They remain intentionally excluded from `Rc<RefCell<T>>` and `Arc<Mutex<T>>`:
their temporary cell/lock guards cannot escape the forwarding call. Guard:
`representation_lowering::shared_handle_forwarding_tests::borrowed_returns_forward_through_direct_handles_only`.

The interface-getter residual is now covered: a mutable source `Item` returned
by a `Runnable` holder through `<T extends ItemSource>` keeps its concrete
`Rc<RefCell<Item>>` carrier. Parsed source declarations suppress a stale
unknown-runtime requirement from the generic call; shared mutable returns use
an owned cloned handle rather than an invalid borrow of the element. Guard:
`fixtures::shared_interface_getter_preserves_mutable_item_handle` verifies the
alias-observable update through both the generic caller and a second holder
alias.
The same owned-handle return is forwardable through a mutable sync holder:
`Arc<Mutex<SyncItemSource>>` locks only long enough to call the getter, which
returns its cloned item handle after the guard is dropped. Guard:
`fixtures::shared_sync_interface_getter_preserves_mutable_item_handle`.
The observable-alias diagnostic now accepts an implemented `SharedSync`
carrier as well as `SharedLocal`, so a direct mutation through one `Arc` alias
does not falsely report that a later read through its clone diverges. The same
sync getter guard exercises that route.

Codex follow-up 2026-09-18 fieldless anonymous objects: a non-functional source
interface with multiple abstract methods now lowers to a local generated object
adapter rather than the `<dyn Trait>::anonymous()` panic fallback. Every
declared method is lowered through the ordinary body pipeline. The deliberately
next boundary is anonymous instance state and `this`/`super`; guard:
`fixtures::fieldless_multi_method_anonymous_interface_is_object_backed`.

That fieldless adapter now also has a narrow real-`this` tier: anonymous-only
primitive helpers (including primitive parameters) with a single non-`super`
return are emitted in the adapter's inherent `impl`. Interface methods can call
those helpers through `this.helper()` without closure capture or fabricated
receiver identity. Guards:
`fixtures::fieldless_anonymous_interface_supports_this_helper_method` and
`fixtures::fieldless_anonymous_interface_supports_this_helper_parameters`.
The helper tier composes with initialized primitive anonymous fields: helper
reads use the same `Cell::get()` projection as interface methods, retaining
per-instance state rather than returning the storage cell. It also supports the
exact update-and-return form `this.field = expression; return field;`, emitting
one `Cell::set(...)` followed by `Cell::get()`; this is deliberately narrower
than arbitrary helper control flow. Guards:
`fixtures::stateful_anonymous_interface_helper_reads_instance_field` and
`fixtures::stateful_anonymous_interface_helper_mutates_instance_field`.

The object-backed path also follows every source superinterface chain with
distinct inherited contracts, including Java's comma-separated direct
interfaces.
An anonymous implementation of a derived interface now emits the required
parent-trait impl before its derived-trait impl, so inherited abstract methods
are supplied by the adapter and inherited Java default methods remain callable.
If the child redundantly redeclares a parent abstract method, its Rust subtrait
omits that duplicate contract rather than creating ambiguous method lookup.
The same adapter preserves a qualified interface-default call such as
`Scaled.super.tripled()` across parent-state mutation; this is distinct from
anonymous classes extending a source class and using class `super`.
Two unrelated parent interfaces that declare the same abstract signature now
collapse onto one canonical Java contract on the combined Rust trait. The
anonymous adapter implements each parent trait and the combined contract, while
a named implementation uses the same canonical surface. Matching is by name
plus erased parameter types rather than name plus arity, preserving overloads.
Guards: `fixtures::anonymous_interface_collapses_matching_parent_contracts` and
`fixtures::anonymous_interface_matches_overloads_by_erased_parameter_contract`.
The same canonical contract is now materialized when the derived interface has
its own default method: abstract-only parent bounds fully covered by a
collision are omitted from the Rust supertrait list, preventing ambiguous
method lookup while parent defaults or unique contracts remain bounds. Guard:
`fixtures::anonymous_interface_default_method_uses_canonical_colliding_parent_contract`.
The remaining hierarchy work is the class-anonymous representation, not
duplicate abstract contracts or owned interface upcast/coercion.

Codex verification 2026-09-18: an `InterfaceUpcastPlan` carrier is emitted for
each reachable `(derived, parent)` pair and argument lowering invokes it with
explicit `Box<dyn Derived>` and `Box<dyn Parent>` types before borrowing the
parent trait view. That resolves the former ambiguous `.into()` at `take(make())`.
The verified class-anonymous slice now uses its own declared carrier rather
than pretending a local Rust struct is a concrete Java superclass. An
anonymously subclassed base is promoted to `SourceHierarchyCarrier`; its
anonymous variant holds `Box<dyn BaseAnonymousDispatch>`, while the local
adapter composes initialized `Base` state. Carrier forwarding invokes that
global object-safe surface, and `super.value` calls the composed base directly.
The executable oracle [`repro/AnonymousClassSuper.java`](../repro/AnonymousClassSuper.java)
is guarded by `fixtures::anonymous_source_class_super_uses_virtual_carrier_payload`
and returns `14`; an inherited carrier field read returns the initialized
offset `3` through the same virtual payload. This does not yet claim broad
class-anonymous statements or mutable receiver behavior.

The object-safe class carrier now also includes inherited source-class instance
methods. For `Base extends Parent`, an anonymous `Base` override may call
`super.inherited(input)` through its composed base, while adapters that do not
override the member forward it normally. The same executable class-super guard
returns `7`; `representation_lowering::type_emission_tests::anonymous_carrier_dispatch_includes_same_source_superclass_method`
asserts the inherited dispatch signature is emitted. A bounded override may
call a sibling override
through `this`, retaining virtual adapter dispatch; the same oracle's
`forwarded()` path returns `14`. One local initialized from a direct
`super` call before a primitive return expression is also supported and
execution-guarded; broader local control flow remains open. Initialized
immutable primitive or `String` fields declared by the anonymous class are
adapter-owned
and bare or `this.field` reads rebind to `self`; the class-super oracle's
`adjusted()` returns `9`. A primitive factory capture may initialize that
adapter field and is retained through the returned carrier. An owned `String`
field is likewise initialized from a borrowed factory parameter and returns
`"left"` through the carrier. The exact primitive mutation form
`int next = this.total + input; this.total = next; return total;` is
adapter-owned `Cell<i32>` state: the local reads the cell before the write, and
two calls through one returned carrier retain state (`2` then `5`, with the
oracle returning `7`). The equivalent parser-normalized `+=` form is also
covered. Read-only receiver-aware locals also survive the carrier boundary as
an arbitrary sequential run: each initializer may read adapter state, a
capture, an earlier local, or a sibling helper before becoming an ordinary Rust
local. `localPlus()`, `chainedLocalPlus()`, and the three-local
`manyLocalPlus()` each return `9` in the class oracle. General class-anonymous
local reassignment and nested `if`/`else` are also adapter-rebound: the
`localControl` oracle obtains `10` through the nested true branch and `4`
through the false branch without reading factory scope. Primitive adapter-field
writes can now interleave with that local control flow: `interleavedUpdate`
initializes a local from `this.total`, writes the same escaped carrier's
`Cell`, and returns `5`/`6` across its two branches. General class-anonymous
captured-object calls are also receiver-rebound inside branches: the
`capturedConditional` oracle updates one `CapturedNode` alias only in its true
branch and reads `24` across the escaped adapter and original shared handle.
The same branch renderer preserves composed class-super behavior: `branchSuper`
calls `super.branchSuper(input)` on its true path, adds adapter state, and
returns `9` (the false path returns `5`).
An unlabeled `while` loop is now receiver-rebound too: `loopUpdate` advances a
local counter and writes `this.total` on each pass, returning `3` through the
escaped carrier. The same adapter path renders a basic C-style `for` in Java
evaluation order; `forUpdate` uses `local++` and returns `4` after updating the
escaped `Cell`, while `forAssignmentUpdate` covers `local = local + 1` and
returns `3`.
An unlabeled `break` inside the receiver-aware loop body is now preserved:
`breakUpdate` exits after two `Cell` writes and returns `2`. Unlabeled
`continue` is now context-aware: it directly continues a `while`, performs the
post-body update for a C-style `for`, and rechecks the condition for a
`do`/`while`; its three oracle paths return `2`, `2`, and `1`. Labeled loop
control beyond the bounded class and mutable-interface `while`, C-style `for`,
and `do` loops remains separate work. The
[`AnonymousClassLabeledWhile.java`](../repro/AnonymousClassLabeledWhile.java)
oracle and its generated-Rust fixture agree on `9`, `15`, `13`, and `11` through
escaped carriers after labeled `continue`/`break`, nested field writes, and
an inner-to-outer labeled continue that runs the outer `for` update.
The adapter also preserves `do`/`while` body-before-condition order:
`doWhileUpdate(0)` writes its `Cell` once and returns `1`. Receiver-aware local
bindings retain their Java primitive/String type in Rust, preventing literal
initializers from becoming ambiguous when later used in wrapping arithmetic.
General class-anonymous reference mutation and control flow,
and arbitrary reference/shared captures remain open.
Read-only lexical captures now become adapter fields rendered with their
already-planned slot shapes, instead of referring to the factory's Rust scope.
The class oracle returns a carrier from `captureOnly(11)` and later reads the
captured value as `11`. The same adapter path now rebinds a captured object
field through its planned shared handle: after the caller changes
`CapturedNode.value` from `3` to `12`, the returned carrier observes `Ok(12)`.
The alias oracle reads through `node.read()` as well, proving captured instance
receiver calls use that same handle projection rather than an outer scope.
That receiver projection composes through an anonymous-only helper reached by
`this.helperObserved()`, so nested adapter dispatch also observes the mutation.
The same path supports a projected mutating call/read pair: an override uses
`node.set(17)` then `node.read()`, and the caller observes `Ok(17)` through its
original handle. Broader captured-object statement/control flow remains open.
An adapter-owned named reference field now uses the same rule instead of a
nominal Rust field: its declaration and initializer share one owned storage
slot shape, so `CapturedNode localNode = new CapturedNode(7)` emits
`Rc<RefCell<CapturedNode>>` consistently. Calls through `this.localNode` are
rebound through the ordinary handle receiver path, as are direct member reads
such as `this.localNode.value`; the escaped class-carrier oracle reads `7` and
then mutates/reads `17`. When initialized from a caller
`CapturedNode`, later caller mutation reads `12` through the carrier and an
adapter mutation reads `17` through the original caller handle. A mutable named
field can also take a plain replacement construction: `this.localNode = new
CapturedNode(value)` lowers into its preselected shared slot and a later carrier
read observes `21`. Arrays, nullable/generic fields, assignment from arbitrary
reference expressions, and wider reference control flow remain outside this
verified slice.
The class-carrier counterpart is now covered by
[`repro/AnonymousClassDirectMemberWrite.java`](../repro/AnonymousClassDirectMemberWrite.java):
a direct `this.node.value = value` override must preserve the caller's node
identity. Its paired direct write/read body is recognized by the class
capability record, marks the anonymous field's declared type mutable for the
shared storage plan, and is lowered through `Rc<RefCell<Node>>`; the focused
fixture also carries a primitive method local into the direct write and returns
`24`. Broader target expressions remain outside this bounded class-carrier
path.
Nested anonymous blocks now contribute the same mutation fact before lowering:
an `if` body that writes `this.node.value` plans the retained `Node` as
`SharedLocal`. The matching class-carrier capability and local-statement
renderer now admit that exact nested target: a local `next` writes `24` on the
true branch and `26` on the false branch through the same caller-visible node.
Guards:
`storage_strategy::tests::nested_anonymous_field_member_write_marks_retained_type_mutable`
and `fixtures::anonymous_class_carrier_writes_through_named_reference_field`.
When that anonymous field is initialized from the factory's existing named
object local, construction clones the planned shared handle instead of moving
it. The factory can therefore retain and observe that alias after a loop writes
`29` through the carrier's field. Guard:
`fixtures::anonymous_class_carrier_writes_through_named_reference_field`.
The same planned handle is retained when one same-typed adapter field is
assigned to another: `this.targetNode = this.sourceNode` clones the handle into
the target cell, and the carrier reads `11` rather than a copied payload.
The adapter statement renderer now also tracks a declared named local's slot
shape within its block. `CapturedNode local = new CapturedNode(31); return
local.value;` returns `31`, and assigning that local to `this.localNode`
returns `29` through the carrier. The same local may be reassigned from a plain
construction and then reads `41` through its retained slot. The method-local
slot environment now follows that handle into nested `if` and `while` bodies:
`localNodeNestedBranch()` mutates the escaped field to `53`, and
`localNodeNestedLoop()` uses `local.set(local.read() + 1)` twice and returns
`9`. That call stages its argument before taking the mutable receiver borrow,
avoiding a `RefCell` panic. `do/while` and C-style `for` carry the same local
shape environment, with the latter also tracking its initializer declarations;
the same executable class fixture now checks two writes through a named
reference local in each form and observes `9` through the carrier field.
Class-anonymous capture discovery also respects method-local shadowing:
`localCaptureShadow(13)` returns its local `13` rather than factory `bonus =
5`, and a sibling capture/parameter/local collision returns `27` as in the
Java [shadowing reproducer](../repro/AnonymousCaptureShadow.java). Object-backed
interface adapters now apply the same per-method capture selection and
block-scoped bare-name rebinding. The
[`AnonymousInterfaceCaptureShadow`](../repro/AnonymousInterfaceCaptureShadow.java)
reproducer and matching generated-Rust fixture each return `25` when a
factory capture, sibling local shadow, and explicit `this.field` coexist.
An object-backed interface method may also reassign a declared primitive
local in an `if/else` while reading an adapter field and a factory capture.
The Java [branch reproducer](../repro/AnonymousInterfaceLocalBranch.java) and
generated-Rust fixture agree on `6` for the captured false branch and `7` for
the branch-local shadow. This does not yet cover general reference-valued
interface statements. Unlabeled `while`, `do/while`, and C-style `for` now
retain the same receiver-aware local and capture binding across their bodies.
The Java [loop reproducer](../repro/AnonymousInterfaceLocalLoops.java) and
generated-Rust fixture agree on `5`, `3`, `5`, `3`, `5`, `2`, including unlabeled
break/continue and a `for` initializer that shadows the factory capture.
An anonymous reference field may also initialize that typed local with its
shared handle: the local reads `7`, mutates the node to `43`, and the adapter
field observes `43`, proving it did not copy the payload.
Two same-typed adapter locals preserve that identity as well: the second
receives a cloned handle, and its mutation to `48` is visible through the
first local.
Reassignment preserves the same rule: after `second = first`, a mutation via
the second local is observed as `52` through the first.
An exact owned-`String` local-and-assignment has the analogous
`RefCell<String>` path: `String next = value; this.mutableLabel = next; return
mutableLabel;` clones reads across the object-safe `&self` boundary and owns
the replacement. The same carrier oracle sets it to `"right"` and reads it
back through a sibling override. This does not extend to arbitrary reference
fields or compound String expressions.
The primitive carrier path also admits one guarded receiver-field update:
`if (enabled) { this.total += input; } return total;`. Its condition is a
simple local/parameter expression and its branch has one assignment; the
class oracle writes `4` on a `true` call and observes `4` after a later
`false` call. General branches and control-flow nesting remain open.
Anonymous-only helper methods now have an adapter inherent impl rather than
being mistaken for failed base overrides. A base override can call
`this.helperTotal()` and the helper reads the same `Cell` state; the oracle
updates it to `6` then returns `6` through that helper path.
Carrier forwarding uses a collision-free payload binding. The oracle declares
and returns a Java parameter named `__java_carrier_payload`; forwarding keeps
that parameter intact and returns `19` rather than shadowing it with an enum
match payload.
The generated oracle further shows that source method signatures still render
the concrete `Base` name despite carrier slot dispatch. Carrier-aware signature
rendering is now covered even for conservative concrete signature shapes.
Named-base construction and the virtual payload remain the preceding sound
sub-milestones. Carrier forwarding now also accepts parameterized instance
methods, covering the ABI needed by `Base.value(int)`; promoting only the
representation would still leave anonymous construction unsupported. The
anonymous-base plan now emits its object-safe `BaseAnonymousDispatch` trait
surface, including clone support for its boxed payload; the local anonymous
adapter implements that global ABI.
An empty anonymous subclass is no longer only a planning signal:
`new Base(8) {}` creates the same anonymous carrier payload, which forwards
every non-overridden object-safe base method to its composed base. The class
oracle calls inherited `value(1)` and receives `9`.
The executable source oracle is
[`repro/InterfaceUpcast.java`](../repro/InterfaceUpcast.java): `take(make())`
must preserve the anonymous object's identity while presenting its `Left`
contract.
[`repro/AnonymousClassSuper.java`](../repro/AnonymousClassSuper.java) is the
parallel class-object oracle: `make()` must return a `Base` carrier whose
override can call `super.value` against the same initialized base state.

The same object-backed tier now retains initialized read-only instance fields
across multiple interface methods. Bare field reads are structurally rebound to
the adapter's `self.field`, while parameter shadowing stays refused. Guard:
`fixtures::stateful_multi_method_anonymous_interface_preserves_instance_fields`.
That read-only adapter now stores a named source reference field through its
declared slot and seeds that shape into each generated method. An anonymous
field initializer `Node node = incoming` is now a storage-plan escape, so a
caller mutation from `61` to `67` remains visible through both adapter methods
(sum `134`) via their shared `Rc<RefCell<Node>>` handle. Guard:
`fixtures::stateful_multi_method_anonymous_interface_preserves_named_field_handle`.

The corresponding narrow mutable tier is now shared across multiple methods:
one primitive field is emitted as `Cell<T>`, a void `this.field += value`
mutator (or direct primitive assignment) updates it, and a separate reader
observes the result. Guards:
`fixtures::mutable_stateful_multi_method_anonymous_interface_preserves_counter`.
`fixtures::mutable_stateful_multi_method_anonymous_interface_preserves_boolean_field`
pins the `boolean`/`Cell<bool>` form.

An initialized named reference field now also supports the precise ordinary
receiver write `this.node.value = value`. The representation gate and renderer
share this capability, and normal shared-handle receiver lowering lets the
adapter and external caller observe the same updated `Node` value. This is not
yet a general reference-field or mutable-control-flow path. Guard:
`fixtures::stateful_anonymous_interface_writes_through_named_reference_field`.

The first reference-valued mutable field is also covered without leaking a
borrow from the adapter: an initialized `String` field uses `RefCell<String>`;
the exact setter replaces its value and a `String` reader or setter-return path
returns an owned clone. Guards:
`fixtures::mutable_stateful_multi_method_anonymous_interface_preserves_string_field`
and
`fixtures::mutable_stateful_multi_method_anonymous_interface_returns_updated_string`.
Arbitrary reference fields remain outside this narrow ownership-safe slice.
The same route safely handles a `String` read-modify-write RHS: it clones the
borrowed field value before replacing the `RefCell`, so successive
`this.label = this.label + suffix` calls retain their accumulated state. Guard:
`fixtures::mutable_stateful_anonymous_interface_appends_string_field`.

Supported mutable fields are no longer limited to one slot per adapter: each
recognized primitive or `String` field gets its own `Cell<T>` or
`RefCell<String>` storage, so independent setter/reader pairs share their own
anonymous-instance state. Guard:
`fixtures::mutable_stateful_multi_method_anonymous_interface_keeps_multiple_fields`.
Two factory calls create separate adapters and therefore separate counter cells;
guard: `fixtures::mutable_multi_method_anonymous_objects_keep_distinct_instance_state`.

The primitive `Cell<T>` route now also admits guarded void updates:
`if (enabled) { this.total += value; }`, and `if/else` when both branches
update the same field. Conditions may read another cell-backed anonymous field;
the lowering projects that field through `.get()` before the branch. The
adapter retains the same state on both sides of the branch; this is still not a
general mutable control-flow or definite-assignment model. Guards:
`fixtures::mutable_stateful_anonymous_interface_allows_guarded_primitive_update`
and
`fixtures::mutable_stateful_anonymous_interface_allows_two_branch_primitive_update`,
plus
`fixtures::mutable_stateful_anonymous_interface_reads_cell_backed_condition_field`.
The guarded form can likewise return that same primitive field after the branch,
so `if (enabled) { this.total += value; } return total;` returns the selected
post-update adapter state. Guard:
`fixtures::mutable_stateful_anonymous_interface_returns_after_guarded_update`.
The same guarded assignment/owned-return route applies to the already-supported
`RefCell<String>` field, without letting a `Ref` guard escape the adapter.
Arbitrary reference fields remain outside this ownership-safe slice. Guard:
`fixtures::mutable_stateful_anonymous_interface_allows_guarded_string_update`.

That same multi-method tier now accepts the equally narrow non-void update
form, `int add(int value) { this.total += value; return total; }`, returning
the post-update `Cell` value while a sibling reader sees the same object state.
Guard:
`fixtures::mutable_stateful_multi_method_anonymous_interface_returns_updated_counter`.

Codex follow-up 2026-09-18: the mutable adapter no longer requires the update
right-hand side to be a single expression. It accepts a sequence of initialized
primitive or `String` local declarations and local reassignments followed by
`this.field = local; return field;`.
Field reads in those declarations are rebound to the adapter and rendered as
`Cell::get()` (or an owned `RefCell<String>` clone) before the final write, so
locals are evaluated in Java statement order and each call observes the same
anonymous-instance state.
This is still a bounded statement tier: it now permits `if`/`else` blocks whose
branches only update those locals before the final receiver-field write, but it
does not model loops, try scopes, receiver mutation before that write, or
arbitrary assignments. Guards:
`fixtures::mutable_stateful_anonymous_interface_updates_field_through_local_variable`
and
`fixtures::mutable_stateful_anonymous_interface_updates_string_field_through_local_variable`,
and
`fixtures::mutable_stateful_anonymous_interface_conditionally_updates_string_local_before_field_write`,
and
`fixtures::mutable_stateful_anonymous_interface_conditionally_updates_local_before_field_write`.
The same receiver-aware rebinding applies to initialized locals in read-only
methods, so a local can consume a bare or explicit anonymous field before a
later return. Guard:
`fixtures::stateful_anonymous_interface_reads_field_through_local_variable`.

The object-backed adapter now also materializes captured factory locals as
cloned adapter fields. Ordinary read-only method bodies bind those captures to
`self` alongside anonymous instance fields, so a returned adapter no longer
tries to capture the surrounding Rust function environment. This is the
ownership-safe cloned-capture slice; mutable captured locals and capture
assignment remain outside it. Guard:
`fixtures::stateful_anonymous_interface_retains_captured_factory_values`.
The same adapter-field capture route covers owned `String` values; guard:
`fixtures::stateful_anonymous_interface_retains_captured_string_value`.
The mutable-alias boundary is executable in
[`repro/AnonymousCapturedMutableAlias.java`](../repro/AnonymousCapturedMutableAlias.java):
after `make(node)`, mutating `node.value` must be observed by `reader.read()`.
Storage analysis now walks anonymous-method lexical free variables, marks a
captured declared-class local as an alias, and therefore selects `SharedLocal`
when its object is later mutated. The factory argument, adapter capture field,
and caller-side mutation retain the same `Rc<RefCell<Node>>`; guard:
`fixtures::stateful_anonymous_interface_preserves_captured_mutable_alias`.
Assignment *to* a captured Java local remains outside this adapter tier.

Read-only anonymous state is not confined to one expression: the adapter now
accepts an `if`/`return` body with explicit or bare instance-field reads and no
assignment or `super`. Field rebinding recurses through the approved control
flow grammar, so each branch uses the adapter's `self.field`. Guard:
`fixtures::stateful_anonymous_interface_allows_read_only_branching`.
The same rebinding traversal covers nested safe expressions (including
ternaries, calls, arrays, casts, `instanceof`, and switch-expression arms),
keeping eligibility and lowering aligned. Guard:
`fixtures::stateful_multi_method_anonymous_interface_rebinds_nested_field_reads`.
The one-method stateful functional adapter uses the same traversal before
lowering, rather than relying on a separate `this.field` spelling path. Guard:
`fixtures::stateful_anonymous_functional_interface_rebinds_nested_bare_field_reads`.

Follow-up cleanup 2026-09-12: the `aliasing` module is crate-internal now. The
translator still reports observable mutable-alias diagnostics through the batch
analysis path, but the detection structs/helpers and diagnostic entry point are
`pub(crate)` implementation details rather than public crate API. The no-facts
convenience collector is test-only/private; production reporting uses the
fact-aware collector.

Codex follow-up cleanup 2026-09-12: `ObservableAlias` is no longer a
crate-local field bag. The detector constructs it through `new(...)`, the
reporter reads the diagnostic fields through accessors, and same-module tests
assert observed alias facts through the same API. This is a layout cleanup only;
mutable aliasing still requires the `SharedLocal` representation work above.

## Minimal repro

`claude_audits/repro/Shared.java`:

```java
public class Shared {
    static class Node { int v; Node next; }
    public int run() {
        Node a = new Node();
        Node b = a;
        a.v = 5;
        return b.v;
    }
}
```

Java result: `5`. (`a` and `b` are the same object.)

## Historical emitted Rust (verified when filed)

```rust
pub struct SharedNode {          // note: no #[derive(Clone)]
    v: i32,
    next: Option<Box<SharedNode>>,
}

impl Shared {
    pub fn run(&self) -> i32 {
        let mut a: SharedNode = SharedNode::new();
        let b: SharedNode = a.clone()/* TODO(translation): validate added clone */;
        a.v = 5;
        return b.v;                  // would be 0, not 5
    }
}
```

rustc output:

```
error[E0599]: no method named `clone` found for struct `SharedNode` in the current scope
```

Two independent defects in one line:

1. **Semantic**: alias → copy. Even with `#[derive(Clone)]` added, `run()`
   returns `0` where Java returns `5`.
2. **Compile**: the clone-insertion path does not require or record that the
   cloned type is `Clone`, so no derive is added.

Clone recovery remains load-bearing, but the marker rendering is no longer a
21-site `lower.rs` mechanism; the audited clone-recovery spelling is centralized
in `coercion.rs`, with lowering calling through structured recovery helpers.

## Root cause

Historically, `ObjectRepresentation` (`src/object_model.rs:151`) could only
express value-like or trait-object representations:

```rust
pub enum ObjectRepresentation {
    ConcreteStruct,
    DynTrait { trait_name: String },
    JavaEnum { enum_name: String, variants: Vec<String> },
    ClosedEnum { enum_name: String, variants: Vec<String> },
    ErasedObject,
}
```

The current tree has a storage-strategy lattice and implements the immutable
handle tier (`SharedImmutable`, rendered as `Rc<T>` or `Arc<T>` depending on
reachability). It still does **not** implement the mutable shared tier for this
repro: no general `Rc<RefCell<T>>`/`Arc<Mutex<T>>` receiver and field lowering,
so Java's mutable reference semantics remain missing for aliased objects that
are written after sharing.

General `Rc<RefCell<T>>` and `Arc<Mutex<T>>` storage/render helpers now exist,
but mutable shared receiver and field lowering are still not implemented.
`__outer` remains the concrete inner-capture use rather than a placement the
planner can safely enable for mutable aliases.

For the mutable aliasing tier, every place that needs a second handle to a
mutable object must still either move it, copy it, or emit a loud unsupported
path. Clone insertion remains a pervasive compatibility mechanism for value
and recovery paths, which is why clone/borrow recovery is still one of the
largest rule clusters in `src/lower.rs` (see
[0006](done/0006-move-analysis-is-five-approximations-of-one-relation.md)).

## Why this is a machinery gap

This cannot be fixed at a lowering site. Deciding between value, borrow, and
shared ownership is a whole-program property of each *allocation site and
type*: does any object of this type become reachable from two live places at
once, and is it mutated after that point?

That is an **escape / sharing analysis** producing a representation decision,
not a local rule. The old unused `EscapeLatticeValue` descriptor in
`src/lattice.rs` has since been deleted as part of 0062; the live domain is
`storage_strategy::Escape`, which is computed by the storage-strategy planner.

## Historical proposed machinery

The concrete names below predate the current `StorageStrategy` /
`StoragePlacement` split. The live version of this work is tracked in 0002b:
`SharedImmutable` has landed, while mutable shared placements remain disabled
until the receiver/field borrow policy for `SharedLocal`/`SharedSync` exists.

1. Extend `ObjectRepresentation` with at least a shared case, e.g.
   `SharedMutable { cell: Rc<RefCell<_>> }` (and later `SharedImmutable(Rc<_>)`
   when the type is never mutated after sharing).
2. Add a sharing/aliasing analysis over the call graph (`src/usage.rs` already
   builds one) that classifies each type — or better, each allocation site —
   as:
   - *linear*: at most one live handle ⇒ `ConcreteStruct` by value (today's behaviour, then correct);
   - *shared read-only*: multiple handles, never mutated after sharing ⇒ `Rc<T>` or by-reference;
   - *shared mutable*: multiple handles with mutation ⇒ `Rc<RefCell<T>>`.
3. Make clone insertion require a *justification*: a clone is legal only when
   the analysis proves no aliasing is observable (value is dead afterwards, or
   the type is immutable). Otherwise the representation must change, or a
   diagnostic must be raised. Today an unjustified clone is emitted with a
   comment.
4. Independently and cheaply: whenever a `clone()` is emitted for a
   source-declared struct, record a `Clone` requirement so `#[derive(Clone)]`
   is added. This fixes the E0599 class on its own and is worth doing first as
   a separate, small change.

## Suggested split for implementers

- **0002a (small, mechanical)**: derive-`Clone` obligation tracking. Fixes the
  compile error, leaves the semantics wrong.
- **0002b (large, design)**: shared representation + aliasing analysis. Fixes
  the semantics.

Do not let 0002a close this issue — it makes the wrong output compile, which is
strictly worse for detection.

---

## Update 2026-08-03 — 0002a done, detection added, 0002b still open

Status at this checkpoint: **still open**. The mutable-alias semantics were
unchanged; what changed is that the divergence was now reported instead of
shipped silently. Later 0002b work landed `SharedImmutable` handles and
reference identity for the immutable tier, but this mutable-alias repro still
requires `SharedLocal`.

### 0002a is done (and confirmed the warning above)

`SharedNode { v: i32, next: Option<Box<SharedNode>> }` was missing
`#[derive(Clone)]` because the derive-reachability walk handled
`Box<dyn Trait>` but not `Box<ConcreteType>` — and a recursive Java object graph
(`Node next`) always lowers to `Option<Box<Node>>`. Fixed by
`raw_rust_box_sized_inner` (`src/types.rs`), used from both raw-type arms of
`rust_type_supports_struct_derive_clone` (`src/item_assembly.rs`). The recursion
guard was already coinductive, so self-referential types work.

The repro now compiles cleanly **and prints `0` where Java prints `5`** — the
exact outcome this file predicted. That is why the next part was not optional.

### New: the divergence is detected

`src/aliasing.rs` decides, per alias point, whether the substituted copy is
observationally equal to Java's alias. It joins two facts that already existed
apart:

- *a second handle is created here* — `T b = a;` where `a` is still live, which
  is precisely the shape that makes lowering clone instead of move;
- *the other handle is still read after the write* — the backward liveness
  fixpoint from [0006](done/0006-move-analysis-is-five-approximations-of-one-relation.md),
  queried at the mutation site.

A write is only reported when both hold, so ordering falls out of the liveness
query rather than needing a separate CFG walk: a write that precedes the
declaration of `b` cannot have `b` live after it.

Mutation sites are field writes (`x.f = …`), element writes (`x[i] = …`), and
calls solved as writing through their receiver — `MethodUsageFacts.mutates_self`
in `src/usage.rs`, which is already a fixpoint over self-calls, so a method that
mutates only by delegating counts. This is a second consumer for the liveness
relation and the first consumer for `mutates_self` outside signature planning,
which is the wiring [0008](done/0008-inference-machinery-does-not-reach-lowering.md)
asks for.

Reported as an `observable_alias` diagnostic from
`translate_parsed_source_with_context`.

**Precision measured on the corpus**: exactly **1** diagnostic across all 196 files
under `examples/`, on `examples/java/04-objects/AliasingSharedMutation.java` —
the one file written to exercise this. No other file produced the diagnostic, so the
diagnostic is not noise.

Conservative in three places. The first is deliberate — a diagnostic that
over-fires gets ignored — the other two were scope limits worth lifting:

- an **unresolved call** answers "does not mutate" rather than "unknown", so an
  external mutator is a miss rather than a false alarm;
- originally, only **local declarations** counted as alias points, so
  `cell[0] = a` and `this.f = a` (an alias stored into an element or a field)
  were not detected;
- **mutations inside a block-bodied lambda** are not walked, since the lambda's
  statements are nested in an expression rather than reached by the statement
  walk. `LivenessAnalysis::for_loop_body` already exists for exactly this shape
  and is the way to lift it.

All three fail in the same direction: a missed report, never a false one.

### What is still missing

`ObjectRepresentation` still has no shared case, so nothing can be *repaired* —
the analysis can only name the defect. The two-part split above stands: 0002b is
the representation work, unchanged.

Historical: `tests/differential/known-gaps/Aliasing.java` reproduced eight
mutable-alias cases against the Java oracle (`5|7|3|9|11|13|17|19`). The
`known_gap_fixtures_are_valid_java_and_match_expected_failure_mode` test verified
that this fixture was still live: Java had to print
`5|7|3|9|11|13|17|19`, and the translator had to report `observable_alias`
instead of silently translating mutable sharing without a `SharedLocal`
representation. That fixture has since moved to `tests/differential/Aliasing.java`
after the bounded SharedLocal slices below landed.


---

## Update 2026-08-03 — the size of the repair is now a number

Status at this checkpoint: still open — the semantics were unchanged and
`ObjectRepresentation` still had no shared case. What was new is that the
*scope* was measured rather than guessed. Later 0002b work consumes the storage
placement for implemented `SharedImmutable`; mutable `SharedLocal`/`SharedSync`
placements remain open.

`src/storage_strategy.rs` implements steps 1–2 of
[0002b](0002b-shared-representation-design.md): the decision rule runs, lowering
is untouched, and every type the plan says needs sharing while lowering holds it
by value is reported as `storage_strategy_disagreement`. In the original
2026-08-03 measurement over 248 files:

| tier | distinct types |
|---|---|
| `SharedLocal` (`Rc<RefCell<T>>`) | 34 |
| `SharedImmutable` (`Rc<T>`) | 14 |
| `SharedSync` (`Arc<Mutex<T>>`) | 0 |

Against **5** `observable_alias` hits on the same corpus. The two numbers measure
different things and the gap is informative: `observable_alias` reports an alias
whose divergence it can prove at a specific write — a lower bound on the damage —
while the 47 count types whose representation is wrong, which is the size of the
change. Neither is the other.

The escape input behind that 47 is deliberately shallow and does not yet produce
`UnknownBoundary`, so 47 is a floor.

## Update 2026-08-11 (Codex follow-up) — block-lambda mutation detection lifted

One of the detection limits above is now narrower. `observable_alias` still
reports diagnostics rather than repairing representation, but block-bodied
lambdas are no longer invisible to the mutation half of the join. When a lambda
body is nested inside an expression statement such as
`values.forEach(x -> { a.v = 5; })`, aliasing now walks the lambda block and
attributes its writes to the enclosing statement for the liveness query.

This originally covered only the immediate-execution lambda shape where the
statement's live-out set is the right observation point. A later follow-up below
lifts the `Runnable r = () -> { a.v = 5; }; r.run();` case too.

## Update 2026-08-11 (Codex follow-up) — storage alias points are detected

Another detection limit above is lifted. `cell[0] = a` and `holder.f = a` now
record alias points when the assigned source local is an object type and is still
live after the assignment. The alias handle recorded for the diagnostic is the
storage receiver (`cell` or `holder`), which is the name liveness can observe
later at `cell[0].v` or `holder.f.v`.

Primitive storage assignments stay excluded: `int` copied into `int[]` or an
`int` field is copy semantics in Java too.

## Update 2026-08-11 (Codex follow-up) — this-field storage aliases are detected

The remaining storage alias shape above is lifted too. `this.f = a` now records
`this.f` as the alias handle, and later reads are detected structurally in
subsequent statements instead of asking local liveness for a `this` binding that
does not exist. Assignment targets such as a later `this.f = new Node()` are not
counted as reads of the previous value.

## Update 2026-08-11 (Codex follow-up) — external receiver mutators feed the diagnostic

The unresolved-call detection limit above is narrower too. `observable_alias`
previously recognised receiver-mutating calls only when the call resolved to a
source method and `UsageFacts` proved `mutates_self`. Fact-backed external APIs
already know the same thing through `ExternalApiEffectFact.receiver_mutability`,
but the alias detector did not receive `FactSet`, so `a.setFlag(...)` on an
external `Record` was treated as non-mutating even when the external pack marked
the method `mutates-receiver`.

Post-lowering alias diagnostics now pass `FactSet` into aliasing. The call
mutation scan still uses source `UsageFacts` for resolved source methods, and
falls back to external API effects using the receiver binding's Java type and
argument count. A read-only external fact remains non-mutating; the new
regression pins both sides.

This still does not repair representation.

## Update 2026-08-11 (Codex follow-up) — stored `Runnable.run()` callbacks are detected

The stored-callback caveat above is narrower. `observable_alias` now records the
captured mutation in a local lambda initializer without treating the declaration
as an execution site. When the stored lambda is later invoked through
`r.run()`, those captured writes are attributed to that call statement, so
`Runnable r = () -> { a.v = 5; }; r.run(); return b.v;` reports the same
observable alias as a direct field write.

The regression also pins the inverse: declaring the callback without invoking it
does not report a mutation.

This still does not repair representation.

## Update 2026-08-28 — current state after the SharedImmutable rollout

This audit no longer means "there is no shared representation at all."
`StorageStrategy::SharedImmutable` is implemented and the identity fixtures now
exercise `Rc::ptr_eq` for shared immutable source values. The still-open part is
the mutable Java aliasing case from this file: aliased values that are mutated
after sharing need `SharedLocal` (`Rc<RefCell<T>>`) or `SharedSync`
(`Arc<Mutex<T>>`) throughout slots, receivers, field access, assignment, and
method calls.

That distinction matters for handoff. A value-object alias may now be correct
through `Rc<T>`; the `Node b = a; a.v = 5; return b.v;` repro is not, because
the observable write requires a mutable shared carrier. The implementation
direction is therefore the remaining mutable-handle tiers described in
[0002b](0002b-shared-representation-design.md), not another clone-recovery
patch.

## Update 2026-09-07 — observable mutable aliases are translation errors

Until `SharedLocal`/`SharedSync` exist, a proven mutable alias still means the
generated Rust computes a known-wrong answer. `report_observable_aliases` now
emits `observable_alias` as `Severity::Error` instead of a warning, so the CLI
fails the translation rather than shipping the copied-value program. The
known-gap harness classifies `Aliasing.java` as `LoudDiagnostic` and checks that
the diagnostic code is an error.

## Update 2026-09-10 — known-gap oracle covered field storage too

Historical: `tests/differential/known-gaps/Aliasing.java` was expanded to
include the field-storage alias shapes already covered by `src/aliasing.rs`
unit tests: storing an alias through another object's field and through a
`this` field, alongside the block-action and stored-runnable callback cases.
The known-gap harness expected Java output `5|7|3|9|11|13|17|19` and the same
`observable_alias` error. This did not implement `SharedLocal`; it made the
then-remaining mutable-alias representation gap executable at the Java-oracle
level.

## Update 2026-09-10 — aliasing known gap was tied to the `SharedLocal` queue

Historical: the `Aliasing.java` known-gap row required more than the
error-level `observable_alias` diagnostic. It also required
`storage_strategy_disagreement` warnings naming `Aliasing.Node needs SharedLocal`
and `Aliasing.Counter needs SharedLocal`, so the core mutable-alias fixture
proved that the same source program landed in the 0002b mutable
shared-representation work queue.

## Update 2026-09-14 — first `SharedLocal` slice: direct local/storage aliases

`tests/differential/AliasingLocalField.java` is now a passing Java-vs-Rust
oracle for the smallest mutable-alias shapes:

```java
Node a = new Node();
Node b = a;
a.v = 5;
return b.v;

Counter a = new Counter();
Counter b = a;
a.bump();
return b.get();

Node[] cell = new Node[1];
cell[0] = a;
a.v = 9;
return cell[0].v;

holder.stored = a;
a.v = 11;
return holder.stored.v;

this.stored = node;
node.v = 13;
return this.stored.v;

StoredRunnable action = new StoredRunnable(a);
action.run();
return b.v;
```

That slice renders the aliased source type as `Rc<RefCell<T>>`, clones the
handle for the second local, writes fields through `borrow_mut()`, reads fields
through `borrow()`, and dispatches direct source receiver calls through
`borrow()`/`borrow_mut()` according to the solved method signature. The
optional field-read path also borrows through optional `Rc<RefCell<T>>`
receivers before projecting fields, so array-element and holder-field aliases
read the shared object rather than the handle. Optional SharedLocal assignment
targets also unwrap with `as_ref()` before `borrow_mut()`, so a named stored
action can mutate through an optional shared field without requiring `&mut self`.
The `observable_alias` diagnostic is suppressed only for these direct
local/direct field-or-receiver-call shapes when the aliased type has
implemented single-threaded `SharedLocal` placement.

At this checkpoint the broader `known-gaps/Aliasing.java` fixture still stopped
at `unsupported_anonymous_class`: the direct field and direct receiver-call
alias cases were represented, so the fixture reached the captured action
boundary. It identified the next unsupported shapes: captured anonymous actions
and callback methods whose parameter/trait-object carrier had to preserve the
shared source-object handle across the call. Inner-class outer construction was
also still blocked: `Rc<RefCell>` field storage does not imply lowering can
synthesize an `Rc<RefCell<Self>>` receiver handle from `&mut self`.

Follow-up 2026-09-14: named callback method parameters now preserve the
SharedLocal handle across the call boundary. `AliasingCallbackParam.java`
exercises `owner.apply(new NamedAction(a), b)`, where `NamedAction.run()`
mutates `a` and `Owner.apply(Action action, Node node)` reads `node.v` after
the callback. The emitted callee parameter and caller argument slot both use
the storage shape (`Rc<RefCell<Node>>`) rather than rejoining to `&Node`, so the
Java/Rust oracle now matches `17`. At that checkpoint the broad known-gap
fixture still stopped at the anonymous action object itself; implementing
anonymous class object lowering was separate from preserving named callback
parameter handles.

Follow-up 2026-09-14: the broad `Aliasing.java` fixture is promoted. The
remaining anonymous action shape is intentionally narrow: a zero-argument,
void-returning functional-interface implementation now lowers to a boxed
closure that clones captured locals before `move`, so the outer
`Rc<RefCell<Node>>` handle is preserved across `Action.run()`. The generated
trait surface has a matching closure adapter only for that single-method shape;
general anonymous class object lowering, anonymous bodies that use `this`, and
interfaces with parameters/returns remain separate work. The Java/Rust oracle
now covers the full original output:
`5|7|3|9|11|13|17|19`.

Codex follow-up 2026-09-14: the diagnostic-side support predicate now matches
the lowering-side boundary for that narrow anonymous action slice. A
zero-argument void anonymous functional-interface body that mentions `this` or
`super` is still outside the closure shortcut, so
`anonymous_class_diagnostics_with_plan` no longer treats it as supported and
continues to report `unsupported_anonymous_class`. Regression:
`representation::tests::anonymous_zero_arg_interface_body_using_this_still_reports_unsupported`.
The shared `this`/`super` body scanner now lives in `java_ir`, so representation
diagnostics and lowering use the same predicate instead of two copied walkers.

Codex follow-up 2026-09-14 later: the same diagnostic predicate now receives the
support-local anonymous-interface context that lowering uses. A source file can
implement a zero-argument void `Action` supplied by co-translated context while
capturing a SharedLocal handle; that narrow closure path lowers correctly and no
longer receives a false `unsupported_anonymous_class` diagnostic. Regression:
`fixtures::context_anonymous_action_with_sharedlocal_capture_is_not_reported_unsupported`.

Codex follow-up 2026-09-17 anonymous mutable-capture storage pass: the storage
lattice now descends into anonymous bodies and records writes through captured
outer handles. The context `Action` fixture therefore plans `Node` as
`Rc<RefCell<Node>>`, rather than `Rc<Node>`, and its closure mutation is both
compilable and observable through the alias (`"23"`). The same fixture is now
an execution guard, not only a generated-crate compile check.

Codex follow-up 2026-09-18 interface-upcast verification: lowering invokes the
identity-preserving `From<Box<dyn Derived>> for Box<dyn Parent>` carrier with
explicit box types before projecting `&dyn Parent`; therefore source-level
`take(make())` no longer leaves `.into()` unconstrained. The executable oracle
[`repro/InterfaceUpcast.java`](../repro/InterfaceUpcast.java) is guarded by
`fixtures::anonymous_interface_upcasts_to_parent_at_source_call_argument`.

Codex follow-up 2026-09-19: a class-anonymous override can now iterate a
primitive array parameter with enhanced `for`, update its own state, and
honor labeled loop control through the base-class carrier. The executable
Java/Rust guard is
`fixtures::anonymous_source_class_enhanced_for_rebinds_array_parameter_and_state`
with the `13` oracle in
[`repro/AnonymousClassEnhancedFor.java`](../repro/AnonymousClassEnhancedFor.java).
This does not close general anonymous-object representation or collection
iteration; those remain in scope for the audit.

Codex follow-up 2026-09-19: the class-anonymous carrier now preserves the
solved mutable-slice ABI and fallible return type for an inherited base method
that writes a caller-owned array. The caller keeps a mutable local and observes
the same array element after dispatch. The Java/Rust oracle is
[`repro/AnonymousClassArrayMutation.java`](../repro/AnonymousClassArrayMutation.java)
(`10`), guarded by
`fixtures::anonymous_source_class_inherited_method_mutates_array_argument`.
This is a verified array-alias forwarding case, not a claim that all anonymous
overrides or mutable-array loop forms are supported.

Codex follow-up 2026-09-19: a class-anonymous override now keeps a fallible
base method's `Result` ABI while rebinding method parameters for its own body.
The Java/Rust guards are
`fixtures::anonymous_source_class_override_preserves_fallible_base_contract`
(`0` for an empty-array `length` override) and
`fixtures::anonymous_source_class_override_propagates_its_own_array_failure`
(`7` for a valid checked read, error for an empty array). Repro sources are
[`AnonymousClassFallibleOverride.java`](../repro/AnonymousClassFallibleOverride.java)
and [`AnonymousClassFallibleBody.java`](../repro/AnonymousClassFallibleBody.java).
These cases do not prove general exception-aware anonymous-body lowering.

Codex follow-up 2026-09-19: a class-anonymous override now forwards a
fallible same-method `super` call both as a direct return and through an
ordinary local before returning an adjusted value. The fixture
`fixtures::anonymous_source_class_fallible_super_return_preserves_result`
matches the Java oracle
[`AnonymousClassFallibleSuper.java`](../repro/AnonymousClassFallibleSuper.java):
`7`, `8`, then two caught array-index failures. Other `super` targets remain
unverified.

Codex follow-up 2026-09-19: a subsequent fixture covers a different
class-`super` target. `super.base(value)` in two overrides must call the
selected `base(int)` method, not each override's own base implementation;
`super.checked(values)` also propagates its array error through an anonymous
method local. The Java oracle
[`AnonymousClassOtherSuperTarget.java`](../repro/AnonymousClassOtherSuperTarget.java)
printed `21`, `8`, `caught` at this checkpoint, matched by
`fixtures::anonymous_source_class_super_call_uses_the_resolved_other_method`.
The `base(String)` overload and `checkedViaLocal` name prefix are regression
guards against ambiguous Rust forwarding. Other class-super expression forms
remain in scope.

Codex follow-up 2026-09-19: class-`super` calls nested in arithmetic returns
now use their selected target on either operand side; fallible calls propagate
array errors before the arithmetic when the override contract is already
fallible. The expanded
[`AnonymousClassOtherSuperTarget.java`](../repro/AnonymousClassOtherSuperTarget.java)
oracle printed `21`, `9`, `9`, `8`, `8`, `8`, then three caught errors at this
checkpoint, matched by
`fixtures::anonymous_source_class_super_call_uses_the_resolved_other_method`.
The 1,092 library tests and 87 anonymous fixtures passed. At this checkpoint,
a fallible anonymous override did not promote an otherwise-infallible base
contract merely by calling another fallible `super` method.

Codex follow-up 2026-09-19: the anonymous override and exact base virtual
contract now share a fallibility result even when the base declaration itself
cannot throw. The factory remains infallible because constructing the
anonymous object does not invoke its methods. The expanded
[`AnonymousClassOtherSuperTarget.java`](../repro/AnonymousClassOtherSuperTarget.java)
oracle prints `21`, `9`, `9`, four `8` lines, then four caught errors, matched
by the translated fixture. At this checkpoint, the 1,093 library tests and 87
anonymous fixtures passed. An anonymous override that threw directly, without
reaching a known fallible callee, still needed independent fallibility seeding.

Codex follow-up 2026-09-19: anonymous method declarations now remain
associated with call-graph keys, allowing direct body effects to promote the
base virtual contract. Java/Rust guards
[`AnonymousClassDirectArrayContract.java`](../repro/AnonymousClassDirectArrayContract.java)
and [`AnonymousClassDirectThrowContract.java`](../repro/AnonymousClassDirectThrowContract.java)
verify that a checked array read and an explicit exception construction are
catchable through a base-typed reference. The 1,093 library tests and 89
anonymous fixtures pass. This is not a claim that every throwing expression
or other open audit requirement is closed.

## Closure verification 2026-09-21

The representation defect reported by this audit is fixed for the repository's
concrete repro corpus. A fresh translator build and the complete anonymous
fixture selection ran 112 tests with zero failures. The focused shared-handle
selection ran seven tests with zero failures, including local and sync mutable
aliasing, interface getters returning mutable objects, and the parameterized
mutable-interface bound. The five shared slot-shape tests, four placement-law
tests, and seven shared-forwarding unit tests also pass.

All 63 `claude_audits/repro/Anonymous*.java` sources translate without an error
diagnostic. Remaining output is warning debt (principally conservative
ownership/clone joins and false factory-to-anonymous-override fallibility
edges), not a reproduced compile failure or wrong answer; the corresponding
generated-runtime fixtures pass. Anonymous-adapter forms not represented by a
current valid-Java repro remain explicitly fenced rather than inferred as
supported. In particular, the bounded catch-only anonymous try-body path does
not claim anonymous try-with-resources or `finally` lowering. On this evidence,
0002 can be marked fixed/fenced; no concrete in-scope failure remains.
