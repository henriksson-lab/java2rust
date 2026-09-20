# 0002b — Shared object representation design and rollout

Status: **partially implemented** (`SharedImmutable`, `SharedLocal`, and the
base `SharedSync` tier landed; bounded class-anonymous carriers and source-level
interface upcasts landed; broad anonymous-class objects and cell/lock
borrowed-return trait surfaces remain open)

Codex verification 2026-09-19: a local variable initialized with a stateful
anonymous interface object now has the same boxed trait slot as a returned
object, avoiding an unsized `dyn Trait` local and default-initializer fallback.
The bounded sequential mutator path can lower locals before, between, and after
ordered top-level `this.field` writes while preserving the same receiver state. The
Java oracle `repro/AnonymousInterfacePostWriteLocal.java` and the generated
Rust fixture `anonymous_interface_method_reads_local_after_field_write` both
produce `66`. `repro/AnonymousInterfaceTwoWrites.java` and its generated Rust
fixture produce `1818`; `repro/AnonymousInterfaceVoidTwoWrites.java` and its
generated Rust fixture produce `10`. A recursive receiver-aware `if`/`else`
statement path preserves branch-local declarations, outer-local reassignment,
and writes to distinct anonymous fields. The Java/Rust pairs
`repro/AnonymousInterfaceNestedBranchWrites.java` and
`repro/AnonymousInterfaceNestedOuterLocal.java` produce `688` and `571`.
`repro/AnonymousInterfaceFieldNames.java` produces `60`, guarding against
rewriting a longer field name or a string literal during cell-read projection.
The recursive path also admits unlabeled `while` loops with receiver-cell writes,
including `break`/`continue` and a condition that rereads a mutated cell. Java
oracles `repro/AnonymousInterfaceWhileWrites.java` and
`repro/AnonymousInterfaceWhileControl.java` match generated Rust at `810` and
`566`. Unlabeled `do`/`for` loops execute the condition or update on a
`continue` path: `repro/AnonymousInterfaceDoWhileWrites.java` and
`repro/AnonymousInterfaceForWrites.java` match generated Rust at `566` and
`1010`. A branch-local `String` literal assignment reaches the owned
`RefCell<String>` slot, guarded by
`repro/AnonymousInterfaceNestedStringWrite.java` at `455`. Ordinary call
statements now pass through both mutable and read-only interface method
bodies; `repro/AnonymousInterfaceCapturedCall.java` and
`repro/AnonymousInterfaceReadOnlyCall.java` prove that calls through a captured
shared handle mutate the same source object (`405` and `632`). This does not
establish all expression-statement forms: explicit `this` integer-field
prefix/postfix `++` and `--` also pass through the cell-write path, guarded by
`repro/AnonymousInterfaceIncrementStatements.java` (`355`) and
`repro/AnonymousInterfaceDecrementStatements.java` (`311`). The mutable
interface receiver renderer now supports active labels on `for`, `while`, and
`do/while` loops, including labeled jumps from nested loops. Java oracles
`repro/AnonymousInterfaceLabeledFor.java`,
`repro/AnonymousInterfaceLabeledWhile.java`, and
`repro/AnonymousInterfaceLabeledDoWhile.java` match generated Rust at `1310`,
`19419`, and `9409`. Their focused fixtures and all 92 anonymous fixtures pass;
the 1,093 library tests also pass. Scoped ordinary and labeled blocks now run
through the same receiver-aware paths in both mutable interface and class
carriers; Java/Rust pairs `repro/AnonymousInterfaceLabeledBlock.java` and
`repro/AnonymousClassLabeledBlock.java` agree on `9209` and `93`, including a
class `super` call in a composed return after the block. All 94 anonymous fixtures and
1,093 library tests passed at that checkpoint. A further Java/Rust pair,
`repro/AnonymousLabeledBlockUnlabeledJumps.java`, agrees on `1313` after both
renderers give enclosing loops explicit Rust labels. This preserves Java's
unlabeled `break`/`continue` targets when they cross a labeled block; all 95
anonymous fixtures and 1,093 library tests passed at that checkpoint. Numeric
returns with `super` nested beneath multiple arithmetic operators or unary
negation now lower recursively. Java/Rust
`repro/AnonymousClassNestedSuperExpression.java` agrees on `16`, `-8`, and two
caught array-index failures; all 96 anonymous fixtures and 1,093 library tests
passed at that checkpoint. The mutable-interface receiver path now admits a
named local initialized from a matching shared anonymous field. Its handle is
cloned, not moved from `&self`; a trivial accessor setter on that local uses
the ordinary shared-field assignment path with a mutable borrow and ordered
RHS evaluation. The same scoped receiver-aware path accepts direct field
assignment through a field-derived reference local, including a local declared
inside an `if` branch; its eligibility walk discards alias provenance on
reassignment. Unit guard
`representation::tests::receiver_statement_reference_alias_loses_field_provenance_on_reassignment`
checks that boundary. Java/Rust
`repro/AnonymousInterfaceReferenceLocal.java` agree on `6968`, including caller
and adapter observation of the same mutated node.
The mutable-interface receiver path now also admits an enhanced `for` over a
matching primitive-array method parameter. Its loop binding is scoped, and
active labels preserve `continue`/`break` while receiver-cell writes persist
across calls. Java/Rust `repro/AnonymousInterfaceEnhancedFor.java` agree on
`499`. A stateful interface-anonymous method with a guarded direct exception
construction now follows its solved virtual `Result` contract. The field-read
binder preserves non-anonymous `new` expressions in the throw path; normal
returns are wrapped in `Ok`, while the thrown path returns an error. Java
`repro/AnonymousInterfaceDirectThrowContract.java` prints `2` and `caught`;
`anonymous_interface_direct_throw_promotes_virtual_contract` verifies the
generated Rust. A read-only fallible array-return method now wraps its normal
return in `Ok`, and a fallible `void` receiver-write method returns `Ok(())`
after success. Java/Rust oracles
`repro/AnonymousInterfaceReadOnlyArrayResult.java` and
`repro/AnonymousInterfaceFallibleVoidMutator.java` agree on `17`/`caught` and
`5`/`caught`. A read-only guarded explicit throw also passes its virtual
`Result` contract (`repro/AnonymousInterfaceReadOnlyDirectThrow.java`,
`8`/`caught`). Duplicate inherited method contracts are no longer represented
by a single anonymous override edge: both parent declarations get the
fallibility result, and the synthesized target adapter method uses the
declaration owner's exception plan. Java/Rust
`repro/AnonymousDuplicateParentThrowContract.java` agree on `9`/`caught`
through the target and both parent interfaces; the fallibility unit guard
checks both edges without promoting the factory. All 103 anonymous fixtures
and 1,095 library tests pass at that checkpoint. A colliding generic return
contract keeps the result ABI through the joined and both parent interfaces
(`repro/AnonymousGenericCollisionThrow.java`, `15`/`caught`). For a collision
whose generic parameter specializes from `T` to `String`, the target adapter
now keys synthesized contracts by effective parameter types rather than raw
declaration erasure, avoiding duplicate Rust method definitions. The anonymous
override graph also matches specialized parameters, so both parent slots get
the fallibility result. Java/Rust
`repro/AnonymousGenericParameterCollisionThrow.java` agree on `2`/`caught`
through the two parent interfaces; direct joined calls are ambiguous in Java.
A class-anonymous method now keeps a field-derived shared local across a
branch member write and a composed `super` return. Its capability check
tracks the local alias by scope, and the class renderer sends the write through
ordinary typed statement lowering. Java/Rust
`repro/AnonymousClassReferenceLocalAlias.java` agree on `75`, including the
caller's view of the mutated node. A capability unit guard rejects an alias
after reassignment. All 106 anonymous fixtures and 1,096
library tests pass. Other throwing expression and
special-case method-body shapes, reference-local origins, enhanced-for iterable forms, label placements, and
broader class-adapter statement rebinding remain open.
Blocks: [0002](0002-object-aliasing-has-no-representation.md),
[0023](done/0023-inner-class-construction-always-panics.md),
and all of [CONCURRENCY-SCOPE](../CONCURRENCY-SCOPE.md)

This exists because three separate documents all stop at the same sentence —
*"the object model has no shared representation"* — and
[CONCURRENCY-SCOPE](../CONCURRENCY-SCOPE.md) step 2 says explicitly: **fix the
representation choice on paper before implementing it. Cheap now, expensive
later.** This is that paper.

The early sections below are the original design. Later updates record the
current implementation state: `SharedImmutable` handles, 0028 identity
lowering, `SharedLocal`/`SharedSync` receiver slices, and source-level
interface upcasts are landed; broad anonymous-class lowering and cell/lock
borrowed-return forwarding remain.

Codex verification 2026-09-18: the current `StoragePlacement` gate implements
both `Rc<RefCell<T>>` (`SharedLocal`) and `Arc<Mutex<T>>` (`SharedSync`). The
Java/Rust differential oracles for mutable aliasing and qualified/ordinary
inner capture pass, and a generated-crate guard covers a static-reachable
`Arc<Mutex<Outer>>` captured by a non-static inner:
`differential::aliasing_shared_local_fixtures_match_java_output` and
`differential::qualified_inner_capture_fixtures_match_java_output`; the output
marker guard `inner_capture_routes_shared_outer_handle_companion` also passes.

Codex verification 2026-09-18 callback intersection: the functional-interface
adapter path now supports ordinary parameters while preserving a captured
`SharedLocal` handle. The local interface slot is a `Box<dyn Interface>` and a
read-modify-write through its closure evaluates the RHS before taking the
`RefCell` write borrow. Guard:
`idiom_gaps::parameterized_anonymous_callback_preserves_sharedlocal_capture_identity`.

Codex verification 2026-09-18: the anonymous-interface boundary now has a
small object-backed tier alongside the closure tier. An initialized anonymous
field read through `this` is emitted in a local adapter struct, retaining its
per-instance state; this is intentionally not yet the mutable/general
anonymous-object representation.

The multi-method object adapter now also admits an initialized read-only named
source field. Its declaration and initializer use the same storage slot shape,
and that shape is seeded into every generated interface method before ordinary
field access lowering. Anonymous field initializers now participate in storage
planning: `Node node = incoming` marks `Node` as stored, so a caller mutation
and the adapter field use the same `Rc<RefCell<Node>>`. `NamedFieldObject`
mutates its caller-owned node from `61` to `67` after construction and two
adapter reads return `134`; the storage-plan guard is
`storage_strategy::tests::anonymous_object_field_initializer_marks_factory_argument_stored`.
Interface object adapters now materialize only factory locals read by an
anonymous method body. Field initializer-only parameters are not captured as
borrowed struct fields, avoiding an invalid escaping lifetime while the
declared anonymous field retains ownership of its lowered initializer. Existing
fieldless and mutable-capture adapter guards remain green.

Codex verification 2026-09-19: eligible anonymous interfaces now try the
object-backed adapter before the functional shortcut. A one-method object with
instance state and a fieldless object with multiple methods therefore share
the same per-method capture rebinding, while a fieldless one-method interface
still uses the closure adapter. The new fieldless `CapturedPairOps` fixture
returns `14` from two methods reading the escaped factory value. The mutable
`CapturedCounter` fixture retains both factory `bonus = 2` and its own
`Cell<i32>` total across two calls, returning `3` then `7`. Java
[`repro/AnonymousObjectCapturePaths.java`](repro/AnonymousObjectCapturePaths.java)
prints the same `14`, `3`, `7`, `7`, including a second alias to the same
counter. The full `fixtures` anonymous subset passes 58/58 after routing
object-backed forms through the common path and sharing class dispatch storage.

Codex verification 2026-09-19: the static-reachability closure now includes
declared interfaces as well as classes. A statically reachable class that owns
an interface field gets a sync cell (`Arc<Mutex<T>>`), so the field's
`Box<dyn Interface + Send>` does not also have to be `Sync`. The anonymous
adapter for that interface uses `Arc<Mutex<Adapter>>`. The Java oracle
[`repro/AnonymousInterfaceTransitiveStatic.java`](repro/AnonymousInterfaceTransitiveStatic.java)
and `fixtures::statically_reachable_field_carries_anonymous_interface_safely`
both return `4`; `storage_strategy::constructor_mutation_tests::statically_reachable_interface_container_needs_sync_cell`
pins the placement. This is a
specific source-field reachability path, not a claim of complete object-graph
or collection-element coverage. A follow-up also propagates static
reachability across class inheritance, including an inherited interface field
in a flattened subclass. The Java oracle
[`repro/AnonymousInterfaceInheritedStatic.java`](repro/AnonymousInterfaceInheritedStatic.java)
and `fixtures::static_subclass_reaches_inherited_anonymous_interface_field`
both return `4`; the placement guard is
`storage_strategy::constructor_mutation_tests::statically_reachable_subclass_inherits_interface_cell_requirement`.
The full library suite passes 1087/1087 after this follow-up.

Codex verification 2026-09-19: static array elements and local generic-list
elements can each own a stateful anonymous interface object. The local
collection insertion shape now retains `BoxedDynTrait` from its storage slot,
so `Vec::push` receives the owned `Box<dyn Trait>` rather than a borrowed
trait view. The Java oracle
[`repro/AnonymousInterfaceCollection.java`](repro/AnonymousInterfaceCollection.java)
and `fixtures::{static_array_element_carries_anonymous_interface_state,
local_generic_collection_element_carries_anonymous_interface_state}` return
`4` for both paths. Static generic-list mutation is now also covered for
`add`, indexed insertion, `set`, and `remove(index)`: a
zero-argument static collection initializer uses the ordinary collection
constructor lowering, and these operations use the guarded persistent vector
after evaluating its argument. The
[`repro/AnonymousInterfaceStaticList.java`](repro/AnonymousInterfaceStaticList.java)
oracle and `fixtures::static_generic_collection_element_carries_anonymous_interface_state`
return `4` for aliasing and `307` after structural edits. An owned
`List.get`-as-`List.add` argument reads the same static before the lock and
clones the anonymous-object handle; `set` and `remove(index)` return owned
interface values without double boxing, preserving aliases after removal.
Other mutating static collection methods are not covered;
they can still act on the getter's cloned vector unless given the same
storage-aware lowering.

Default-bearing interface traits now emit shared-handle forwarding from their
canonical method signatures, including default methods; calls are qualified
by the exact trait to avoid parent/child name collisions such as `as_any`.
The inherited-default and colliding-parent default fixtures both compile and
run through an `Rc`-backed anonymous object.
Trait-to-implementation forwarding now uses the canonical rendered trait
parameter shape rather than a stale signature-plan shape; an owned `String`
or `Vec` trait argument is borrowed when the concrete implementation accepts
`&str` or a slice. Guards:
`fixtures::dyn_trait_interface_parameters_use_trait_object_views`,
`fixtures::interface_forwarder_borrows_owned_array_for_slice_implementation`,
and `representation_lowering::trait_impl_forwarded_arg_tests`.

The same adapter now emits the whole source-superinterface chain with distinct
contracts (including each comma-separated direct Java interface) when the
anonymous target is a derived interface. This is required by Rust's explicit
supertrait implementation rule and preserves inherited Java defaults without
turning the object back into a closure. A repeated inherited abstract
declaration is collapsed to one method in the derived Rust trait, preventing
two unrelated Rust methods from representing Java's one contract. Qualified interface-default
calls (`Derived.super.defaultMethod()`) lower through that same adapter and see
the current interior-mutable parent state. Class-`super` remains outside this
interface-object slice. Colliding abstract signatures from unrelated parents
now become one canonical Java method contract on the combined Rust trait;
adapters implement every parent plus that canonical target surface, and named
implementations use the same plan. The key is method name plus erased parameter
types, so same-arity overloads do not collide. Guards:
`fixtures::fieldless_anonymous_interface_inherits_default_method`,
`fixtures::anonymous_interface_collapses_matching_parent_contracts`, and
`fixtures::anonymous_interface_matches_overloads_by_erased_parameter_contract`.
This also applies inside a derived-interface default body: when two
abstract-only parents contribute one erased contract, the derived trait owns
that canonical method and redundant parent bounds are omitted so ordinary Rust
method lookup is unambiguous. Guard:
`fixtures::anonymous_interface_default_method_uses_canonical_colliding_parent_contract`.
For a derived trait without default methods, the anonymous adapter also
implements that contract when it comes from only one parent, and when the
child directly redeclares it. A local declared as
the parent interface now retains the parent's boxed-trait dispatch instead of
the initializer child's dispatch, so the owned `From<Box<dyn Child>>` upcast
runs before later calls. This covers both the first and second parent in a
comma-separated `extends` declaration. Guards:
`fixtures::anonymous_interface_upcasts_to_parent_at_local_assignment` and
`fixtures::anonymous_interface_redeclared_parent_contract_has_both_trait_impls`.
Covariant inherited contracts are now selected independently of parent order:
`Object value()` and `String value()` combine as `String value()` on the child,
for both anonymous adapters and named implementers. Calls through either child
resolve as `String`, while a named implementation of the `Object` parent boxes
its covariant result for that parent trait. The executable
[`AnonymousCovariantParentContract.java`](repro/AnonymousCovariantParentContract.java)
oracle prints `16`, matching
`fixtures::anonymous_interface_collisions_select_covariant_return_independent_of_parent_order`.
This verifies the `Object`/`String` slice, not arbitrary covariant reference
hierarchies.
Generic specialization has a passing bounded interface-chain path, but remains
incomplete for general defaults and shared mutable generic carriers. In the executable
[`AnonymousGenericContractCollision.java`](repro/AnonymousGenericContractCollision.java)
oracle, `Source<T>.value()` and `Named.value()` meet as `String value()` on a
`Source<String>, Named` child and print `5`. The canonical contract planner
now substitutes arguments through successive `extends` edges, so the derived
trait emits `value(): String`; a focused unit test also checks that
`Source<T>.echo(T)` becomes `echo(String): String` through `Relay<String>`.
The generic parent traits now declare their type parameters, and parameterized
trait-object slots, adapter implementations, and owned upcast carriers agree on
`Source<String>` and `Relay<String>` rather than leaving `T` or `U` free.
The longer [`AnonymousGenericContractChain.java`](repro/AnonymousGenericContractChain.java)
oracle prints `12` for a `Source<T> <- Relay<U> <- Joined` chain with both
`value()` and `echo(T)`. A passing generated-crate contract test,
`fixtures::anonymous_generic_parent_contract_specializes_adapter_and_upcast`,
checks the result `12`, including inherited methods on `Relay`. The
parameterized `Joined` to `Source<String>` assignment now reaches its generated
owned upcast; the formerly reported same-trait coercion was a stale local
dispatch shape and is fixed for non-generic parent assignments.
The hierarchy reachability check now traverses every interface parent and
erases type arguments only for matching, while preserving explicit package
qualification and ambiguous-name safeguards (guard:
`lower::tests::source_subtype_conversion_follows_parameterized_interface_chain`
plus the existing combined-context identity tests).
The ancestor-instantiation planner now composes `Relay<String>` and
`Source<String>` from the chain (guard:
`representation_lowering::type_emission_tests::ancestor_instantiations_compose_type_arguments_across_interface_edges`),
and type arguments are threaded through trait emission, object slots, adapters,
forwarding implementations, and upcasts for this chain. Generic immutable
interfaces use direct `Rc`/`Arc` forwarding only: direct and `RefCell`/`Mutex`
blanket implementations for the same parameterized trait overlap under Rust
coherence. A distinct carrier type is still needed for both handle forms to
coexist, and this fixture does not prove general generic defaults or arbitrary
reference-covariant collisions.

Codex verification 2026-09-19: the bounded generic-default path now declares
`SourceWithDefault<T>` with an owned `T` return and lowers a default body that
returns another owned `T` without inventing a `Clone` bound. A call on a
derived boxed interface is routed through its parameterized parent upcast,
so the inherited default dispatches on the same anonymous adapter. Upcasting
an existing boxed interface value clones its handle before conversion: Java's
source and parent variables remain usable aliases. The executable
[`AnonymousGenericDefault.java`](repro/AnonymousGenericDefault.java) and
`fixtures::anonymous_generic_parent_default_method_preserves_specialized_contract`
agree on `10` for the generic return and `15` for an inherited default with
an integer argument; all 81 anonymous fixtures and 1,091 library tests passed
after the added argument check.
This verifies one generic default body and inherited call shape, not every
generic default, collision, or mutable shared-carrier combination.

Codex verification 2026-09-18: each derived-parent pair emits an owned carrier
and `From<Box<dyn Derived>> for Box<dyn Parent>`. The borrowed-parameter path
now selects that conversion with fully named source and target box types before
projecting `&dyn Parent`, avoiding Rust's ambiguous `Into`/`AsRef` inference.
The executable Java oracle [`repro/InterfaceUpcast.java`](repro/InterfaceUpcast.java)
and `fixtures::anonymous_interface_upcasts_to_parent_at_source_call_argument`
pass. Class-anonymous carriers use a separate bounded path: a local anonymous
Rust struct is returned through its source hierarchy carrier, not as the
concrete Java superclass struct.
The class-super oracle establishes the required representation boundary:
[`repro/AnonymousClassSuper.java`](repro/AnonymousClassSuper.java) now lowers
through `BaseValue::Anonymous(Rc<dyn BaseAnonymousDispatch>)`. The local
adapter composes its initialized `Base`, forwards `value(int)` through the
carrier, and its Java `super.value(input)` calls that composed base directly;
the executable fixture returns `14`. An inherited base field read through the
returned carrier (`make().offset`) is dispatched through a virtual getter and
returns `3`. A value-copy fallback is semantically invalid and is not used.
Within that same bounded class-adapter grammar, one override may call another
through `this` (`this.doubled(input)`), which lowers to the adapter's virtual
method and preserves the anonymous override; the oracle's `forwarded()` path
returns `14`.

The class carrier's anonymous dispatch handle is now shared independently of
whether an outer storage slot also selects `Rc`. Body lowering preserves the
carrier shape rather than turning it into erased `Object`, and slot-aware fresh
construction applies the representation plan's handle placement. The
[`repro/AnonymousClassAlias.java`](repro/AnonymousClassAlias.java) Java oracle
and `fixtures::anonymous_source_class_assignment_shares_mutable_instance_state`
both return `12` after writes through two base-typed aliases. General
anonymous-class statement lowering still lies outside this bounded adapter.

For a class carrier reachable from a process-wide static, the anonymous
variant instead stores `Arc<Mutex<dyn BaseAnonymousDispatch + Send>>`; its
forwarding and field access lock the payload and identity uses `Arc::ptr_eq`.
Static field initialization now routes an anonymous constructor through the
same slot-aware lowering rather than falling back to a closed-enum default.
The [Java oracle](repro/AnonymousClassStatic.java) and
`fixtures::static_reachable_anonymous_class_keeps_dispatch_and_state` return
`4`. The related directly static-stored interface case now uses a `Send`
supertrait and only `Arc<Mutex<T>>` shared forwarding; eligible fieldless
one-method objects use the sendable object adapter instead of the local `Rc`
closure adapter. The
[`repro/AnonymousInterfaceStatic.java`](repro/AnonymousInterfaceStatic.java)
Java oracle and
`fixtures::{static_reachable_anonymous_interface_keeps_dispatch_and_state,
static_reachable_single_method_anonymous_interface_uses_send_object}` agree
on `4`, inherited default `8`, and single-method `5`. Detection currently
covers directly declared process-wide static interface fields, not transitive
reachability through every source object graph.
The same class slice preserves a sequential run of ordinary initialized locals
before the return, including a local initialized by a direct `super` call (`int
base = super.value(input); return base * 2;`). Each initializer is rebound
through the adapter before it becomes a Rust local, retaining Java evaluation
order. Broader local control flow and mutation remain outside this class path.
Initialized immutable primitive or `String` anonymous-class fields are now
adapter-owned
state rather than discarded local syntax. Bare and `this.field` reads rebind
to `self` in the class-super path; the oracle's
`super.adjusted(input) + this.bonus` returns `9`. Field mutation and
arbitrary reference/shared captures remain separate work. A primitive factory
value may initialize that adapter
field (`int capturedBonus = bonus`); its `this.capturedBonus` read survives the
return boundary and is execution-guarded by the same oracle. An owned `String`
field may likewise initialize from a borrowed factory parameter; the oracle
returns `"left"` through its `label()` override after the carrier boundary.
The same oracle also covers the exact primitive update form
`int next = this.total + input; this.total = next; return total;`: the adapter
stores `total` in `Cell<i32>`, evaluates the local before the write, and two
`tick` calls through one returned `Base` carrier retain the state and sum to
`7`. The equivalent parser-normalized `+=` form is also covered. General
class-anonymous mutation/control flow and
arbitrary reference/shared captures remain outside this class path.
Read-only lexical captures now become class-adapter fields rendered with their
planned slot shapes, so `captureOnly(11)` returns a carrier whose later method
call reads `11` without referring to the factory's Rust scope. A captured
mutable-object field read is also receiver-aware: after the caller changes
`CapturedNode.value` from `3` to `12`, the returned carrier observes `Ok(12)`
through its planned shared handle; the oracle's `node.read()` call uses the
same projected receiver, including when reached through an anonymous-only
`this.helperObserved()` call. One projected captured-object mutating-call/read pair
is also covered: an override uses `node.set(17)` then `node.read()`, and the
caller observes `Ok(17)` through its original handle. Broader captured-object
statement/control flow remains outside this class path.
For the exact owned-`String` local-and-update form `String next = value;
this.mutableLabel = next; return mutableLabel;`, the class adapter uses
`RefCell<String>`: replacement owns the borrowed parameter and reads clone the
value before it leaves the `&self` dispatch boundary. The oracle sets and then
reads `"right"` through the same returned carrier. Arbitrary reference fields
and compound String expressions remain outside this class path.
The primitive class path also covers the exact guarded update
`if (enabled) { this.total += input; } return total;`. A simple local or
parameter condition controls one adapter-field write; the oracle writes `4`
with `true` and observes `4` after a later `false` call. General branches and
control-flow nesting remain outside this class path.

The guarded primitive update now also accepts an `if/else` whose two branches
write the same adapter `Cell` field. The class oracle applies `+2` in the true
branch and `+2 * 3` in the false branch, observing `8` after both calls;
bare Java field reads in the final return are rebound to the same cell.
The identical bounded grammar is valid for an owned `String` field through
`RefCell<String>`: `conditionalLabel(true)` writes `"yes"`, and the following
false call writes and returns `"no"` through the escaped carrier.

Receiver-aware ordinary locals now have a bounded class-adapter form: a method
may initialize any sequential run of locals from adapter fields, lexical
captures, earlier locals, a direct composed-base call, or `this.helper()` and
return a local or primitive arithmetic rooted in it. Initializers are rendered
through the adapter receiver (`self.captured_bonus` / `self.helper_total()`),
so they do not refer to the factory scope after the anonymous object escapes.
The class-super fixture's `localPlus()` returns `9` from `int local =
this.capturedBonus; return local + input;`; `chainedLocalPlus()` verifies a
two-local chain; and `manyLocalPlus()` verifies a three-local sequence whose
first initializer calls the sibling helper before it combines captured state.
That same adapter-local renderer now supports local reassignment, ordinary
expression statements, and nested `if`/`else` blocks whose conditions read
adapter state or locals. `localControl(true, 4)` follows a nested branch and
returns `10`; the false branch returns `4`. Field writes remain on the separate
`Cell`/`RefCell` storage path but are now emitted by this renderer too, so they
can interleave with local control flow: `interleavedUpdate` initializes a local
from `this.total`, writes the `Cell` in each branch, and returns `5`/`6` for
the true/false paths. Loops/try scopes and unmodeled reference-field layouts
are not yet admitted by this tier.
Captured mutable-object receivers are rebound through their adapter field in
the same renderer. `capturedConditional` calls `node.set(value)` only in its
true branch and then calls `node.read()`; the caller observes the shared node
through both paths (`12 + 12 = 24`), demonstrating that the branch did not
fall back to a factory-scope reference or a value copy.
Class `super` is available in branch-local returns as well: `branchSuper` uses
`super.branchSuper(input) + this.capturedBonus` only in its true branch and
returns `9`, while the false branch returns the captured adapter field (`5`).
An unlabeled `while` loop now uses the same renderer: `loopUpdate` advances an
adapter-local counter and writes `this.total` on each iteration, returning `3`
through the escaped carrier. A basic unlabeled C-style `for` is lowered with
the same initializer/condition/body/update order as an explicit Rust `while`;
`forUpdate` exercises `local++` and returns `4`. An unlabeled `break` is also
safe in the rendered loop body: `breakUpdate` exits after two adapter `Cell`
writes and returns `2`. Unlabeled `continue` is renderer-contextual: it is a
direct Rust continue for `while`, runs the post-body update before continuing a
C-style `for`, and rechecks the condition before continuing a `do`/`while`.
The three guards return `2`, `2`, and `1` respectively. Bounded ordinary and
labeled blocks are now covered by the same renderer; enhanced-for forms beyond
the bounded primitive-array parameter case and try scopes remain outside this tier.
The same post-body update route also covers assignment-form updates:
`forAssignmentUpdate` uses `local = local + 1` and returns `3` after three
adapter `Cell` writes.
The class adapter also preserves labeled `while`, C-style `for`, and `do` loops
with labeled `continue` and `break`. Its capability check admits only active
loop labels; the renderer carries each target's continuation action so a `for`
update or `do` post-condition executes before a labeled jump, including a
jump from a nested loop. The mutation scan sees writes nested in those loops.
The Java
[`AnonymousClassLabeledWhile.java`](repro/AnonymousClassLabeledWhile.java)
oracle and
`fixtures::anonymous_source_class_labeled_while_rebinds_receiver_and_control_flow`
agree on `9`, `15`, `13`, and `11` for the three loop forms
and an inner-to-outer labeled continue. The 81 anonymous fixtures and 1,091
library tests passed at that checkpoint. Bounded labeled blocks are now
covered as noted above; other enhanced-for forms remain open.
Receiver-aware branch expressions now include unary `!`, `-`, and `+` for the
primitive tier. `unaryBranch(false, 4)` negates its condition and returns the
adapter capture (`5`); the true path returns `-4`.
An unlabeled `do`/`while` emits a Rust `loop` with its condition checked after
the rebounded body. `doWhileUpdate(0)` therefore writes the escaped `Cell`
once and returns `1`, demonstrating body-before-condition ordering. Adapter
local declarations retain their Java primitive/String type in generated Rust,
so literal initializers used in later wrapping arithmetic do not become
ambiguous.
Direct arithmetic needs no synthetic local: `directFieldPlus()` returns
`this.capturedBonus + input` through the same receiver-aware adapter operands,
also yielding `9` in the executable carrier fixture.

### Next class-adapter milestone

The remaining class-anonymous boundary is no longer a carrier representation
choice. It is general method-body rebinding: transform anonymous `this` field
and helper projections, captures, and `super` calls into an adapter-local IR
before ordinary statement lowering. The current renderer owns primitive/String
locals, adapter-field writes, and nested conditionals, but a full pass must
still cover arbitrary reference fields, other labeled statements, broader enhanced `for` and try
scopes, and broader receiver expressions.

The planner and renderer must advance together: a receiver-field write through
an adapter-local is not admitted until the same per-anonymous-body capability
record proves that normal statement lowering can consume the local's slot
shape. This keeps an unsupported body from silently falling back to a nominal
base value while the general adapter-local IR is introduced.

The class-carrier reproducer
[`repro/AnonymousClassDirectMemberWrite.java`](repro/AnonymousClassDirectMemberWrite.java):
`this.node.value = value` in a `Base` override must mutate the caller's node
through the adapter field rather than make anonymous construction fall back to
the nominal base. The class capability record now recognizes the exact
write/read pair, storage facts mark the retained declared-class field mutable,
and lowering emits its `Rc<RefCell<Node>>` receiver write. The fixture guard
`fixtures::anonymous_class_carrier_writes_through_named_reference_field`
also preserves a primitive method local before that write:
`int next = value + 1; this.node.value = next; return this.node.value;`
returns `24`. Replacement of the adapter field, arbitrary member targets, and
control-flow-containing writes remain separate work.

Storage planning now also walks nested anonymous method blocks when it records
direct member writes through an anonymous field. Thus an `if (enabled) {
this.node.value = value; }` body selects `SharedLocal` for `Node`. The class
capability record and local-statement renderer now admit that same direct
nested target: a local `next` is computed first, the true branch writes `24`
through the shared receiver, while the false branch writes `26` through that
same caller-visible handle.
Guards: `storage_strategy::tests::nested_anonymous_field_member_write_marks_retained_type_mutable`
and `fixtures::anonymous_class_carrier_writes_through_named_reference_field`.
The same class field initializer now clones an existing same-typed factory
handle rather than moving it into the carrier. That lets the factory retain its
alias after construction; a `while` body writes `29` through the adapter field
and the factory observes `29` through its original handle. Guard:
`fixtures::anonymous_class_carrier_writes_through_named_reference_field`.

The first named reference-field milestone now uses that required slot plan: an
anonymous class field's declaration is rendered from its owned storage shape
and its initializer is lowered into that same assignment slot. Thus `new
CapturedNode(7)` becomes `Rc<RefCell<CapturedNode>>` in both places, and a
`this.localNode.read()`, `this.localNode.value`, or
`this.localNode.set(value)` receiver is lowered via the ordinary shared-handle
projection rather than a raw `self.local_node` receiver.
`AnonymousClassSuper.fieldNodeValue()` and `fieldNodeMutate()` return `7` and
`17` through the escaped `BaseValue` carrier. The same field may be
initialized from an externally aliased `CapturedNode`: later external mutation
is read as `12`, and an adapter mutation is read as `17` through the original
handle. This does not yet admit arrays, generic/reference assignment, nullable
reference fields, or arbitrary reference-valued control flow.
The same named-field slot may now be replaced by a plain construction: a
mutable `CapturedNode localNode` is stored as
`RefCell<Rc<RefCell<CapturedNode>>>`, and `this.localNode = new
CapturedNode(value)` lowers the RHS into the field's existing slot before
replacing that outer cell. The escaped carrier's subsequent read returns the
replacement value (`21`). Assignment from arbitrary reference expressions,
nullable/generic fields, and nested reference control flow remain open.
Assignment between same-typed adapter fields also preserves the planned
handle: `this.targetNode = this.sourceNode` clones the source handle into the
target field's outer cell rather than cloning a `CapturedNode` payload. The
carrier fixture reads `11` afterward. Cross-typed conversion and arbitrary RHS
expressions still require the general typed-local rebinding pass.
Codex verification 2026-09-19: a nested `if` can replace a named anonymous
class field's handle without replacing a separately held source alias. The
[`AnonymousClassNestedReference.java`](repro/AnonymousClassNestedReference.java)
oracle and
`fixtures::anonymous_class_nested_reference_field_replacement_preserves_source_alias`
both return `373`: the adapter first reads `3`, then reads its replacement
`7`, while the source alias still reads `3`. This narrows, but does not close,
the nested-reference-control-flow boundary above.

A parameter-to-field ownership boundary is now covered for a shared,
single-threaded named source class. The Java oracle
[`AnonymousClassParameterFieldAlias.java`](repro/AnonymousClassParameterFieldAlias.java)
returns `733` after two factory calls, one replacing its field and one keeping
the source reference. The translated `make` now accepts
`Rc<ParameterBranchNode>`, its call sites clone that handle, and the anonymous
field retains a clone in `RefCell<Rc<ParameterBranchNode>>`. Guard:
`fixtures::anonymous_class_factory_parameter_field_keeps_shared_handle`
asserts that ABI shape and runs the generated crate to `733`. It also checks
`this.node == original` before and after replacement through `Rc::ptr_eq`:
both factory objects initially share the caller's handle, and only the first
loses that identity after replacing its own field. The parameter
ownership choice is restricted to methods that retain the parameter in an
anonymous field; a blanket shared-immutable parameter widening did not pass
the broader fixture run and would change unrelated method ABIs. Cross-thread
`Arc` capture and indirect
reference initializers remain outside this proven slice.
That rebinding pass now has its first named-local slice: a `CapturedNode`
local is declared with its planned shared slot type, retained in the adapter
block environment, and ordinary member access uses the normal handle receiver.
The carrier oracle reads `31` from `local.value`; assigning that local to an
anonymous reference field then reads `29`. Reassignment uses the same local
slot: `local = new CapturedNode(41)` replaces the handle and later reads `41`.
A same-typed anonymous field may initialize that local by cloning its planned
handle; the local reads `7`, then `local.set(43)` is observed through the
original adapter field as `43`. Nested scopes and arbitrary reference
initializers remain open.
Two same-typed named locals now use the same owned-slot conversion: the second
clones the first handle, so `second.set(48)` is observed through `first` as
`48` rather than moving the handle or copying its payload.
The same conversion applies on reassignment: after `second = first`, a
mutation through `second` is visible through `first` as `52`.
Codex verification 2026-09-19: nested class-adapter `if` and `while` bodies
inherit the method-local slot environment. A `CapturedNode local =
this.localNode` handle can be used inside a branch or loop, and writes through
it remain visible through the escaped carrier field. The executable
`AnonymousClassSuper` fixture checks `localNodeNestedBranch()` returns `53`
and `localNodeNestedLoop()` returns `9`. The same environment handoff is now
used by `do/while`; C-style `for` also includes its initializer declarations
in the loop-local shape environment. The executable fixture now checks both
additional loop forms with a named reference local and two writes through its
shared handle, each returning `9` through the escaped carrier field.
The fixture also caught a borrow-order defect: `local.set(local.read() + 1)`
previously panicked because the mutable receiver borrow started before the
argument's read. The adapter statement renderer now evaluates arguments into
temporaries before calling through a local receiver when an argument reads
that same receiver. The loop guard executes that exact self-referential form
twice and observes the final value `9` through the carrier field.
Capture discovery for class-anonymous methods now tracks local declarations
within their lexical blocks and selects each method's own free reads from the
adapter's captured fields. A method-local `bonus` returns its assigned `13`
rather than the factory's `5`; sibling methods can still read the factory
capture while their parameter or local also bears that name. The
`AnonymousClassSuper` fixture checks the combined result `27`, and
[`repro/AnonymousCaptureShadow.java`](repro/AnonymousCaptureShadow.java)
compiles and prints `27` with Java 19.
Object-backed interface adapters now use the same scoped free-read discovery
and select captures per implementation method. Their field/capture rebinder
stops converting a bare name after a local declaration shadows it, while an
explicit `this.field` still addresses the instance. The focused
`stateful_anonymous_interface_distinguishes_capture_from_sibling_local_shadow`
fixture returns `25` across a factory capture, sibling local, and explicit
field read. The Java reproducer
[`repro/AnonymousInterfaceCaptureShadow.java`](repro/AnonymousInterfaceCaptureShadow.java)
also prints `25`. The related 19 stateful anonymous-interface fixtures pass.
The read-only object adapter now also admits reassignment of a method local
declared earlier in the same lexical scope, including from a nested `if/else`
branch. Its representation gate checks the same declaration condition, so a
bare anonymous-field write cannot enter that local path. The executable
`stateful_anonymous_interface_reassigns_receiver_aware_local_in_branch`
fixture and Java
[`repro/AnonymousInterfaceLocalBranch.java`](repro/AnonymousInterfaceLocalBranch.java)
both return `6` when the false branch reads the factory capture and `7` when
the true branch uses a local with the same name. General reference-valued
statements remain open.
The same declaration-aware capability and scoped rebinder now admit ordinary
method-local updates in unlabeled `while`, `do/while`, and C-style `for`
statements without mutating anonymous instance fields. The executable
`stateful_anonymous_interface_reassigns_locals_across_loop_forms` fixture and
Java [`repro/AnonymousInterfaceLocalLoops.java`](repro/AnonymousInterfaceLocalLoops.java)
agree on six results: `5`, `3`, `5` for the basic loop forms, `3` for a `while`
break, `5` for a `for` continue, and `2` when a `for` initializer shadows the
factory capture. Bounded labeled `while`, `for`, and `do/while` loops are now
covered separately above; other labeled statement forms, enhanced-for beyond
primitive array parameters, try scopes, and
general reference-valued interface statements remain outside this capability.
The current focused `AnonymousClassSuper` fixture is retained as the executable
baseline while that generalized rebinding pass is introduced.
Anonymous-only helper methods are emitted in an adapter inherent impl, so a
base override may call `this.helperTotal()` without treating the helper as a
base override. The oracle writes `6` into the adapter `Cell` then reads `6`
through that helper path.
Carrier forwarding also reserves a collision-free internal payload binding: a
Java parameter named `__java_carrier_payload` remains the method argument and
returns `19`, while the generated match arm picks a suffixed payload binding.

The same carrier contract now includes inherited source-class instance methods.
An anonymous override of `Parent.inherited(int)` on a `Base extends Parent`
adapter dispatches `super.inherited(input)` to its composed `Base`, while
adapters that do not override the inherited method forward it normally. The
class-super oracle verifies the override returns `7`; the representation-level
guard verifies that the object-safe dispatch trait includes the inherited
signature.

The representation planner now records every unambiguous source class used as
an anonymous-class target (`anonymous_class_base_requires_virtual_carrier`),
including a regression guard for `new Base() {}`. It promotes that base to a
carrier with an anonymous virtual payload and suppresses the diagnostic only
for the verified class-super slice; broader adapter bodies remain diagnostic
visible.
Investigation of the oracle also established a prerequisite that was not
visible at the representation boundary: source method signatures currently
render `Base` directly even when slot dispatch chooses `BaseValue`. Therefore
carrier-aware source signature rendering is now guarded even when signature
planning began with a conservative concrete shape. Ordinary named-base
construction and the virtual payload remain the next steps. Source carrier
forwarding now also accepts parameterized instance methods (rather than only
zero-argument accessors), which is required for the `value(int)` class-super
oracle. The anonymous-base planning signal now also emits the global,
object-safe `BaseAnonymousDispatch` surface from that callable base API; the
local adapter implements this trait. Its boxed trait object has a generated
clone companion, so the carrier can retain ordinary Java assignment and return
flow. Promoting the plan before named construction and the payload exist would
still leave anonymous construction unsupported.
That payload also supplies composed-base forwarding for every non-overridden
object-safe method, so an empty anonymous subclass is executable rather than a
planner-only signal: `new Base(8) {}` returns the carrier and inherited
`value(1)` returns `9` in the class oracle.

That tier now includes interior-mutable primitive scalars: the exact
single-field read-modify-write form (including `+=` after parser normalization)
uses `Cell<T>`, allowing an anonymous functional object to retain state across
calls with the trait's `&self` receiver. The multi-method form covers ordinary
primitive assignment too, including `boolean`/`Cell<bool>`. This is local
anonymous-instance storage only; it does not change `SharedLocal`/`SharedSync`
object-handle policy or trait forwarding.

An initialized `String` field has a separate narrow route through
`RefCell<String>`: setters replace it, while reader and setter-return methods
clone it before crossing the `&self` trait boundary. This avoids escaping a
`Ref` guard and is not yet general reference-field support. Guards:
`fixtures::mutable_stateful_multi_method_anonymous_interface_preserves_string_field`
and
`fixtures::mutable_stateful_multi_method_anonymous_interface_returns_updated_string`.
The `RefCell<String>` route also supports a read-modify-write string RHS by
cloning the field before replacement; guard:
`fixtures::mutable_stateful_anonymous_interface_appends_string_field`.

These narrow mutable slots compose in one adapter: a supported primitive and a
supported `String` field each retain independent state through their respective
interior-mutable cells. Guard:
`fixtures::mutable_stateful_multi_method_anonymous_interface_keeps_multiple_fields`.

The primitive-cell adapter also supports guarded void updates,
`if (enabled) { this.total += value; }`, plus `if/else` when both branches
update the same field. A condition that reads another cell-backed anonymous
field projects it through `.get()` before branching. The selected branch updates
the same `Cell`, retaining the instance state. This is intentionally narrower
than general mutable control flow or definite-assignment reasoning.
Guards: `fixtures::mutable_stateful_anonymous_interface_allows_guarded_primitive_update`
and `fixtures::mutable_stateful_anonymous_interface_allows_two_branch_primitive_update`,
plus `fixtures::mutable_stateful_anonymous_interface_reads_cell_backed_condition_field`.
A guarded mutator may also return that same primitive field after the branch;
the adapter returns its current `Cell` value rather than attempting to preserve
a source-level local across the conditional. Guard:
`fixtures::mutable_stateful_anonymous_interface_returns_after_guarded_update`.
This guarded assignment/owned-return route also covers the existing
`RefCell<String>` field slice; it clones the value across the trait boundary
instead of leaking a `Ref` guard. Arbitrary reference fields remain open. Guard:
`fixtures::mutable_stateful_anonymous_interface_allows_guarded_string_update`.

The interface adapter also admits one receiver-aware mutable reference form:
`this.node.value = value` where `node` is an initialized named source-object
field. Representation eligibility and rendering share that exact capability;
ordinary lowering projects the planned `Rc<RefCell<Node>>` receiver for the
write, so the adapter and its original caller observe the same `8`. This is
not general reference-field mutation or a general mutable-body grammar. Guard:
`fixtures::stateful_anonymous_interface_writes_through_named_reference_field`.

The approved read-only `if`/`return` grammar now recursively rebinds bare
anonymous field reads as well as explicit `this.field` reads, keeping every
branch on the adapter's instance state. Guard:
`fixtures::stateful_anonymous_interface_allows_read_only_branching`.
Nested safe expression forms use the same rebinding traversal; guard:
`fixtures::stateful_multi_method_anonymous_interface_rebinds_nested_field_reads`.
The specialized one-method stateful adapter shares that traversal; guard:
`fixtures::stateful_anonymous_functional_interface_rebinds_nested_bare_field_reads`.

The counter recognizer canonically matches Java receiver-field spelling, so an
explicit `this.totalValue` and the declaration's normalized `total_value` name
remain one adapter field. Guard:
`fixtures::stateful_anonymous_counter_rebinds_camel_case_source_field`.

For a multi-method anonymous interface, that interior-mutable tier also admits
the strict non-void update form that returns the updated primitive field. It is
still one `Cell<T>` field and not a general mutable-object representation.
Guard:
`fixtures::mutable_stateful_multi_method_anonymous_interface_returns_updated_counter`.

The same adapter now preserves Java statement order for initialized primitive
or `String` locals preceding that update: `int next = total + delta;
this.total = next; return total;` becomes a local computed from `Cell::get()`,
followed by one `Cell::set()` and the post-update return. A `String` field uses
an owned clone from `RefCell<String>` before its replacement. Multiple such
locals, their local reassignments, and bounded `if`/`else` local updates are
supported before that final write; loops, try scopes, receiver mutation before
it, and general control flow remain outside this tier. Guards:
`fixtures::mutable_stateful_anonymous_interface_updates_field_through_local_variable`
and
`fixtures::mutable_stateful_anonymous_interface_updates_string_field_through_local_variable`,
and
`fixtures::mutable_stateful_anonymous_interface_conditionally_updates_string_local_before_field_write`,
and
`fixtures::mutable_stateful_anonymous_interface_conditionally_updates_local_before_field_write`.

For the read-only object-backed path, captured factory values are now emitted
as cloned adapter fields and rebound through `self` in the generated trait
methods. That keeps captured state associated with the returned anonymous
object rather than an invalid Rust function-item environment. The supported
slice is cloned capture reads; mutation of a captured local remains a separate
shared-handle problem. Guard:
`fixtures::stateful_anonymous_interface_retains_captured_factory_values`.
Owned `String` captures use the same cloned adapter-field route; guard:
`fixtures::stateful_anonymous_interface_retains_captured_string_value`.
The mutable-alias oracle is
[`repro/AnonymousCapturedMutableAlias.java`](repro/AnonymousCapturedMutableAlias.java).
It requires one `SharedLocal` `Node` handle to flow from the factory argument
into the adapter field and remain observable after a caller-side mutation;
direct cloned capture is intentionally not accepted for that shape. The storage
planner now detects the anonymous method's lexically free declared-class locals
and marks them locally aliased before adapter lowering; the complete path is
guarded by
`fixtures::stateful_anonymous_interface_preserves_captured_mutable_alias`.
Assignment to a captured Java local remains outside this adapter tier.
Read-only local declarations use the same field rebinding and remain aligned
between representation analysis and lowering; guard:
`fixtures::stateful_anonymous_interface_reads_field_through_local_variable`.

Codex verification 2026-09-18 shared trait forwarding: a mutating trait method
now emits cell-handle implementations for both `Rc<RefCell<T>>` and
`Arc<Mutex<T>>`; each call acquires its borrow or lock at the boundary. The
generic-bound fixture `shared_mutating_interface_implementation_satisfies_generic_bound`
checks the complete `T extends Counter` path and alias-observable result.
The thread-reachable counterpart now checks the `Arc<Mutex<T>>` path through
the same generic bound, including alias-observable mutation. Guard:
`fixtures::shared_sync_mutating_interface_implementation_satisfies_generic_bound`.
Reference-returning methods now forward through direct `Rc<T>`/`Arc<T>`
handles, whose borrow is tied to their `&self` receiver. They remain excluded
from cell/lock handles because a `Ref`/`MutexGuard` cannot soundly escape the
forwarding implementation. Guard:
`representation_lowering::shared_handle_forwarding_tests::borrowed_returns_forward_through_direct_handles_only`.

The mutable interface-getter ABI is now covered. For
`interface ItemSource { Item item(); }` implemented by a `Runnable` holder and
called through `<T extends ItemSource>`, a parsed source `Item` remains
concrete despite the generic call's unknown-runtime observation. Since the
returned storage is shared and mutable, the trait and implementation return a
cloned `Rc<RefCell<Item>>` handle rather than borrowing an `Item` through a
temporary cell guard. Guard:
`fixtures::shared_interface_getter_preserves_mutable_item_handle` proves the
update is visible through a second holder alias.
That owned carrier is also safe through the `SharedSync` forwarding tier: the
temporary `MutexGuard` is dropped after producing the cloned item handle.
Guard: `fixtures::shared_sync_interface_getter_preserves_mutable_item_handle`.
The alias diagnostic treats that implemented `SharedSync` storage as a real
shared carrier, matching the `Arc::clone` lowering rather than reporting an
imaginary copied-object divergence.

Codex recheck 2026-09-18: the remaining borrowed-return forwarding restriction
has no current Java-source reproducer. Source-interface object getters use an
owned handle ABI (`Rc<RefCell<T>>` or `Arc<Mutex<T>>`), so their temporary
borrow/lock guard is dropped before the value crosses the trait boundary. The
only `-> &...` forwarding case is the internal direct-handle rule, which
deliberately excludes cell/lock handles because returning a `Ref` or
`MutexGuard`-backed reference would be unsound. Retain the guard
`representation_lowering::shared_handle_forwarding_tests::borrowed_returns_forward_through_direct_handles_only`
if a future facade introduces a source-visible borrowed ABI.

## What one decision buys

This table is the original design framing. Current status: `SharedImmutable`
handles and [0028](done/0028-reference-equality-lowered-as-structural-equality.md)
identity lowering have landed; the promoted mutable-alias and inner-capture
paths use `SharedLocal` or `SharedSync`; unsupported shared-receiver surfaces
are now limited to borrowed-reference returns through a cell/lock handle, which
would outlive its guard.

| issue | today | under this design |
|---|---|---|
| 0002 | `Node b = a` copies; writes through `a` are invisible through `b` | `Rc::clone` — one object, two handles |
| 0028 | **landed for the SharedImmutable scope** | `Rc::ptr_eq` — *exactly* Java's reference identity |
| 0023 | ordinary outer-method inner construction still calls the panic fallback until a shared outer receiver can be passed to the existing companions | `__outer` stops being a one-off; it becomes the general receiver case |
| 0006 | clone/borrow recovery is the largest rule cluster in `lower.rs` | for shared types the answer is always `Rc::clone`, and the cluster does not apply |
| concurrency | synchronized forms fail loudly until monitor semantics exist | `Arc<Mutex<T>>` gives `synchronized (o)` an object to lock |

0028 is worth pausing on: it is not "also fixed", it is fixed *because the
representation is finally the right one*. Reference identity is unimplementable
on values and free on handles. That is the strongest argument that the current
representation is the defect rather than any individual symptom.

## The choice: per **type**, not per allocation site

Per-allocation-site is more precise and is the wrong call here. A translator
must emit one struct definition and one set of method signatures per Java type;
if two allocation sites of `Node` had different Rust types, `Node b = a;`
between them would not typecheck. Precision that cannot be spent is not
precision.

So: one decision per source type, taken as the **join over every use** of that
type. This mirrors how `HierarchyStrategy` (`src/facts.rs:1415`) is already
decided.

## The lattice

```rust
/// How values of a source type are held.
///
/// Ordered by how much sharing must be supported; the plan takes the join over
/// all uses, so the order must be monotone — adding a use can only move a type
/// up.
pub enum StorageStrategy {
    /// No observable aliasing anywhere. Today's behaviour, and now justified
    /// rather than assumed.
    Value,
    /// Aliased, never mutated after sharing. `Rc<T>` / `Arc<T>` — no borrow
    /// flag, no panic risk.
    SharedImmutable,
    /// Aliased and mutated, single-threaded. `Rc<RefCell<T>>`.
    SharedLocal,
    /// Aliased and mutated, reachable across a thread boundary.
    /// `Arc<Mutex<T>>`.
    SharedSync,
}
```

`Value < SharedImmutable < SharedLocal < SharedSync`. Join is max. Monotone, so
the fixpoint terminates.

This is the `Local`/`Shared` policy axis CONCURRENCY-SCOPE asks for, with
`SharedImmutable` split out because it is common (value objects, keys, strings)
and strictly cheaper than `SharedLocal`.

## The inputs, and where each already exists

| input | question | source |
|---|---|---|
| **escapes** | can a value of `T` be reachable from two live places? | `storage_strategy::Escape` (`LocalOnly \| Returned \| Stored \| UnknownBoundary`) computed by the storage-strategy planner |
| **locally aliased** | `T b = a;` with both live | `src/aliasing.rs`, built for 0002 detection |
| **mutated** | any field write or `mutates_self` method | `MethodUsageFacts.mutates_self` in `src/usage.rs` |
| **threaded** | reachable from `Runnable.run` / `Thread` / `ExecutorService.submit` | `CallGraph` in `src/usage.rs` |

**Every input already exists.** Not one of them needs to be built. That is the
main finding of this design pass, and it is why the work is large but not deep:
it is wiring plus a lowering rewrite, not new analysis.

Decision rule:

```
escapes(T) ∈ {Stored, UnknownBoundary} ∨ locally_aliased(T)
  ? (mutated(T) ? (threaded(T) ? SharedSync : SharedLocal) : SharedImmutable)
  : Value
```

`UnknownBoundary` must land on the shared side: an unmodelled external call
could retain the reference, and getting this wrong is a silent wrong answer,
whereas over-sharing costs a pointer indirection.

## What every use site becomes

This table is the real cost of the change; everything else is bookkeeping.

| Java | `Value` (today) | `SharedLocal` |
|---|---|---|
| `new T(…)` | `T::new(…)` | `Rc::new(RefCell::new(T::new(…)))` |
| `T b = a;` | `a.clone()` ⚠ | `Rc::clone(&a)` |
| `a.f` | `a.f` | `a.borrow().f` |
| `a.f = v` | `a.f = v` | `a.borrow_mut().f = v` |
| `a.m()` read-only | `a.m()` | `a.borrow().m()` |
| `a.m()` mutating | `a.m()` | `a.borrow_mut().m()` |
| `a == b` | structural ⚠ | `Rc::ptr_eq(&a, &b)` |
| `a == null` | `Option::is_none` | `Option::is_none` (unchanged) |
| field `T f;` | `T` / `Option<Box<T>>` | `Option<Rc<RefCell<T>>>` |
| param `T x` | `&T` or `T` | `&Rc<RefCell<T>>` |
| return `T` | `T` | `Rc<RefCell<T>>` |

`SharedSync` is the same with `Arc`/`.lock().unwrap()`. `SharedImmutable` is the
same with `Rc` and no `borrow()`.

Note the recursive-field case gets *simpler*: `Option<Box<Node>>` becomes
`Option<Rc<RefCell<Node>>>`, and the `Box` special-casing that
[0002](0002-object-aliasing-has-no-representation.md)'s derive fix had to teach
`rust_type_supports_struct_derive_clone` about stops mattering, because `Rc<T>`
is `Clone` regardless of `T`.

## The hazard: `RefCell` turns a Java-legal program into a panic

```java
void a() { helper.callBack(this); }   // `this` re-entered through another handle
```

Java allows re-entry through a second handle to the same object. `RefCell` does
not: a `borrow_mut()` live across that call panics with *already borrowed*. This
is the exact analogue of the reentrant-monitor problem in
[CONCURRENCY-SCOPE](../CONCURRENCY-SCOPE.md) §1, and it is worse, because it
converts a silent wrong answer into a runtime abort.

**The design rule that avoids it: never hold a borrow across a call.** Borrow at
each access and drop the guard at the end of the statement.

```rust
a.borrow_mut().f = v;              // guard dropped here — safe
let g = a.borrow_mut(); g.m();     // guard live across the call — forbidden
```

This must be a property lowering *guarantees*, not a convention. It costs a
re-borrow per access, which is a branch on a flag — cheap, and the price of
matching Java's aliasing.

`Mutex` under `SharedSync` has the same problem and is not solved by short
guards, because Java monitors are genuinely reentrant. CONCURRENCY-SCOPE §1
already lists the options; the middle one (lock at public entry points, call an
unlocked inner method internally) reuses the call graph and is the best fit.

## Migration, so the corpus does not explode

The audit set's own discipline, applied:

1. **Land `StorageStrategy` computing `Value` for everything.** No output
   change. Assert the plan is stable and the corpus is byte-identical. This
   proves the wiring before it can break anything.
2. **Turn on the real decision rule but keep lowering unchanged**, and *report*
   the disagreement: every type the plan says should be shared but is lowered by
   value is an `observable_alias`-class diagnostic. This gives a work queue and
   a size estimate before a single lowering changes.
3. **Implement `SharedImmutable` first.** It is the largest population and the
   only tier with no `borrow()` and no panic risk, so it isolates "did the
   handle plumbing work" from "did the borrow discipline work".
4. **Then `SharedLocal`**, gated per type. `tests/differential/known-gaps/Aliasing.java`
   moves up a level as the regression test.
5. **`SharedSync` only after** [0030a](0030-java-statics-are-thread-local.md),
   per CONCURRENCY-SCOPE's sequencing — mutable statics are wrong today under
   both models and must not be baked in.

Steps 1 and 2 are cheap, independently useful, and answer the question everyone
actually wants answered — *how much of the corpus is affected?* — before the
expensive part starts.

## What this design does **not** settle

- **Cycles leak.** `Rc` cycles (a parent/child graph, a doubly-linked list) never
  free. Java's GC collects them. This is a real semantic difference that no
  choice here fixes; `Weak` requires knowing which edge is the back edge, which
  is a genuinely open analysis. For a transpiler whose output is run rather than
  audited for memory, leaking is the right first answer, but it should be
  *stated* rather than discovered.
- **Which `PartialEq`.** With `Rc::ptr_eq` for `==`, Java's `.equals()` needed a
  separate path. That was resolved by the 0028 landing; it remains background
  for why identity and `.equals()` must stay distinct.
- **Whether `SharedImmutable` needs interior mutability for lazy fields**
  (`hashCode` caching is the classic case). Probably yes, probably `Cell`.

## Recommendation

Historical note: this recommendation was superseded by the 2026-08-03 update
below. Steps 1 and 2 are done; the remaining implementation question is the
mutable shared placement/receiver policy for `SharedLocal` and `SharedSync`.

Do steps 1 and 2 next. They are small, they cannot regress anything, and they
convert "we do not know how big 0002b is" into a number. Everything after that
should be decided with that number in hand.

---

## Update 2026-08-03 — steps 1 and 2 are implemented, and the number is 47

Status: **steps 1–2 done.** `src/storage_strategy.rs` computes the plan and
reports the disagreements. Lowering is unchanged and the corpus output is
byte-identical, which was step 1's whole point.

### The number

Across the 248 files under `examples/`, `claude_audits/repro/` and
`tests/differential/`:

| tier | distinct types |
|---|---|
| `SharedLocal` (`Rc<RefCell<T>>`) | **34** |
| `SharedImmutable` (`Rc<T>`) | **14** |
| `SharedSync` (`Arc<Mutex<T>>`) | **0** |
| **total needing sharing** | **47** |

For comparison, [0002](0002-object-aliasing-has-no-representation.md)'s
`observable_alias` reports **5** hits over the same corpus. The gap is the
point: `observable_alias` finds aliases whose divergence it can *prove* at a
specific write, while this counts every type whose representation is wrong. The
first is a lower bound on the damage; the second is the size of the change.

`SharedSync` being empty matters for sequencing: nothing in this corpus needs
`Arc<Mutex<_>>`, so the tier that CONCURRENCY-SCOPE puts last, after
[0030a](0030-java-statics-are-thread-local.md), is not on the critical path.
Steps 3 and 4 (`SharedImmutable`, then `SharedLocal`) cover the entire measured
population.

### One correction to this document

> "**Every input already exists.** Not one of them needs to be built."

That was too strong. The old `EscapeLatticeValue` was only an unused descriptor
domain and has since been deleted as part of 0062. The escape input had to be
written (`storage_strategy::Escape` plus the walk that fills it), and the version
here is deliberately shallow: a field of type `T`, or an alias stored into a
field or array element, is `Stored`; a method returning `T` is `Returned`.
`UnknownBoundary` is in the live domain and is not yet produced,
because an unresolved-call escape needs the call graph.

The other three inputs *were* already there and were used as-is:
`locally_aliased` (the `T b = a;` shape, the same one that makes lowering
clone), `mutated` (`MethodUsageFacts.mutates_self`, already a fixpoint over
self-calls), and `threaded`.

That correction matters for the estimate: the escape input is the one that will
move the 47 up or down as it gets more precise, and it is currently biased
*low* (no `UnknownBoundary`), so 47 is a floor.

### Design points confirmed in code

- The lattice is `Value < SharedImmutable < SharedLocal < SharedSync` with join
  = max, and the order is asserted monotone by a unit test — the property that
  makes the join terminate.
- `Returned` alone does **not** force sharing: the value moves out, it does not
  become reachable from two places. `Stored` does. That distinction is pinned by
  a test, because it is the one most likely to be got wrong later.
- The decision is per **type**, taken as the join over uses, mirroring
  `HierarchyStrategy`.

### Recommendation, unchanged in shape but now costed

Do step 3 (`SharedImmutable`) next: 14 types, no `borrow()`, no panic risk, so
it isolates "did the handle plumbing work" from "did the borrow discipline
work". Then step 4 (`SharedLocal`, 34 types) with the
*never hold a borrow across a call* rule as a guarantee rather than a
convention. Step 5 has no population to serve yet.

---

## Update 2026-08-05 — `UnknownBoundary` is now produced, and the "47" is reconciled

### The correction the previous update promised

> `UnknownBoundary` is in the domain and is not yet produced … the escape input
> is the one that will move the 47 up or down, and it is currently biased *low*.

It is now produced. A handle passed as an argument to a call the translator
cannot see through escapes: the callee may retain the reference, and
under-sharing is a silent wrong answer while over-sharing costs a pointer
indirection.

No new analysis was needed, which is this design's recurring finding one input
later. `usage` already classifies every call argument as `InternalSameType`,
`KnownContentRead` or `Unknown`, and a binding already carries its declared Java
type; `collect_unknown_boundary_escapes` reads both.

`ReceiverForCall` is deliberately **not** an escape. Passing an argument gives
the callee a handle it can store; being the receiver does not.

### First: the "47" was never a well-defined number

Three different counts were being conflated, and the document's own table shows
it — `14 + 34 = 48`, while the stated total is `47`. Both are right under
different rules, and the missing sentence is that **a type can appear at two
tiers**, because the plan is computed per program and each fixture is its own
program. The distinct-name count is therefore the sum of the per-tier counts
minus the types that appear at both.

So `47 = 14 + 34 − 1`: fourteen `SharedImmutable`, thirty-four `SharedLocal`,
one type counted in both.

Measured today over the same three roots (`examples/`,
`claude_audits/repro/`, `tests/differential/`), stating the rule each time:

| counting rule | without `UnknownBoundary` | with `UnknownBoundary` |
|---|---:|---:|
| per-file occurrences | 49 | **51** |
| distinct names, `SharedImmutable` | 12 | **13** |
| distinct names, `SharedLocal` | 32 | 32 |
| sum of the two tiers | 44 | **45** |
| **distinct names overall** | 42 | **43** |
| types appearing at both tiers | 2 | 2 |

### What is attributable to this change, and what is not

The two columns above are the **same tree and the same corpus**, with only the
new escape input toggled. That is the attributable measurement:

> `UnknownBoundary` adds **one** distinct type (`SharedImmutable` 12 → 13) and
> two per-file occurrences.

Small — smaller than "will move the 47 up or down" implied. Worth stating
plainly rather than dressing up: the corpus's unresolved-call argument passing
mostly involves types that were already shared for another reason.

The remaining difference from the documented figure (47 distinct then, 43 now)
is **not** attributable to this change and is not claimed to be. The corpus grew
from 248 to 255 files, and the translator itself moved a great deal in between —
several of this round's fixes touch `usage`, which feeds the `mutated` and
alias inputs. Attributing that delta would need the old tree re-measured, which
has not been done. It is recorded here as drift, not as an effect.

### Historical consequence for sequencing

At this checkpoint, `SharedSync` was still empty, so step 5 had no population.
Step 3 (`SharedImmutable`) had **13 types** and was still the right next step for
the reason the design gives: no `borrow()`, no panic risk, so it isolates "did
the handle plumbing work" from "did the borrow discipline work".

At that point lowering still did not consult the plan; this was still step 2, a
report. Later sections supersede this: `SharedImmutable` and 0028 have since
landed.

---

## Update 2026-08-05 (second) — a correction to the sequencing: `SharedImmutable` cannot be tested on its own

This document recommends step 3 (`SharedImmutable`) next, because it has no
`borrow()` and no panic risk and so "isolates *did the handle plumbing work*
from *did the borrow discipline work*". That is sound as engineering tactics and
**wrong as a plan**, for a reason the document does not state.

### The tier has no observable behaviour

`SharedImmutable` is by definition a type that is aliased and **never mutated
after sharing**. If it is never mutated, a copy and a shared handle are
indistinguishable — that is what "immutable" means. So changing the
representation of those 13 types changes no program output at all.

Measured rather than argued. At the time this section was written,
`tests/differential/known-gaps/Identity.java`:

```java
Key a = new Key(5);
Key b = a;            // alias, never mutated
Key c = new Key(5);   // distinct object, equal contents
return b.get() + "," + (a == b) + "," + (a == c) + "," + a.equals(c);
```

| | Java | translation |
|---|---|---|
| `b.get()` — read through the alias | `5` | `5` ✓ |
| `a == b` — true identity | `true` | `true` ✓ |
| `a == c` — distinct objects | `false` | **`true`** |
| `a.equals(c)` — distinct objects | `false` | **`true`** |

Everything that is not an identity comparison already agrees. Both divergences
are [0028](done/0028-reference-equality-lowered-as-structural-equality.md).

### What that means for the plan

A differential gate cannot tell step 3 from a no-op: both report "nothing
changed". The change would be validated only structurally — the emitted types
are `Rc<T>` and the corpus still compiles — which is exactly the kind of
evidence this audit set keeps finding to be insufficient.

So the revised sequencing is:

> **Step 3 lands together with 0028's identity lowering**, not before it.
> `Rc::ptr_eq` for `==` is what gives the tier a test, and
> `known-gaps/Identity.java` was that test: it failed on two of four cases
> before both landed.

This is not extra work bolted on. It is the same work in the order that makes it
verifiable: 0028 is *unimplementable* without the representation, and the
representation is *untestable* without 0028. They are one change.

Step 4 (`SharedLocal`) keeps its own test —
`tests/differential/known-gaps/Aliasing.java` — because mutation through an
alias is observable without any identity operator.

### A third finding, recorded where it belongs

`a.equals(c)` is `true` in the translation and `false` in Java. Java's
`Object.equals` **is** reference identity unless overridden, and the class in
the fixture does not override it. 0028 names this as its "other half" and
explicitly does not design it; it now has a reproducer, noted there.

---

## Update 2026-08-05 (third) — two wrong inputs, and the estimate halves

Attempting step 3 turned up two defects in the *plan itself*. Both were
over-approximations, both fired on very common shapes, and together they had
inflated the population and put it in the wrong tier.

### 1. `mutated` counted constructor field writes

`mutated` is the bit that separates `SharedImmutable` (`Rc<T>` — no borrow, no
panic risk) from `SharedLocal` (`Rc<RefCell<T>>` — borrow discipline, panic
risk). The question it is supposed to ask is whether a value is mutated **after
being shared**; this document says so in the lattice comment.

The walk counted *any* field write, including `this.v = v;` in a constructor. But
construction necessarily precedes any alias to the object — there is nothing to
observe a constructor's writes through. So a `final`-field value class, the exact
population this document names for `SharedImmutable` ("value objects, keys,
strings"), was landing in the harder tier:

```
Identity.Key needs SharedLocal (… locally_aliased=true, mutated=true …)
```

Constructor bodies are still walked for *escape*; only the mutation signal is
suppressed. A setter that happens to be called only from a constructor still
counts, which is right: it is public and can be called later.

### 2. `escape` charged the owner for `this.v = v` on an `int`

The `Stmt::Assign` arm charged `Escape::Stored` to the **owner** whenever a bare
handle was assigned into a field, with the comment "the type is not available
here without a type environment".

The field's declared type *is* available — the walk holds the owning `TypeDecl`,
and `field_type_name` was already being used two lines away for the mutation
signal. Charging the owner instead meant that **every class whose constructor
assigns a parameter to a field** was marked `Stored`, including `this.v = v`
where `v` is an `int`. That is almost every class, so the escape input reported
"shared" for types that never leave a local:

```
Unaliased.Local needs SharedImmutable (escape=Stored, locally_aliased=false …)
```

Now `this.f = x` charges `f`'s declared type. A *qualified* field or an array
element keeps the conservative owner charge, because there the element type
genuinely is not available.

### The revised measurement

Same corpus, same three roots, distinct names:

| tier | as documented | after both fixes |
|---|---:|---:|
| `SharedImmutable` (`Rc<T>`) | 14 | **19** |
| `SharedLocal` (`Rc<RefCell<T>>`) | 34 | **4** |
| `SharedSync` | 0 | 0 |
| **distinct types needing sharing** | **47** | **22** |

The population is **less than half** what this document costed, and the split
has inverted. That matters more than the total:

- `SharedLocal` is the tier with the `RefCell` hazard — the *"never hold a
  borrow across a call"* rule, and the risk of turning a Java-legal program into
  a runtime abort. It is **4 types**, not 34.
- `SharedImmutable` is 19 of 23 tier assignments, and it needs no `borrow()` at
  all.

So the "large but not deep" characterisation holds and the depth is concentrated
in four types. Whether the remaining 4 justify `RefCell` at all, or should be
handled some other way, is now a question worth asking — it was not, at 34.

### What this does *not* change

The sequencing correction from the previous update stands: `SharedImmutable`
still has no observable behaviour of its own, so step 3 still has to land with
0028's identity lowering to be testable. Fewer types, same acceptance test.

### Pinned

Three tests in `src/storage_strategy.rs`: a constructor-only writer is
`SharedImmutable`, a method writer is still `SharedLocal` (or the distinction
would be meaningless), and an unaliased type stays `Value`. They resolve the
program first, because the plan is keyed by qualified nested type names.

`scripts/gate.sh --all`: zero deltas — the plan is still report-only.

---

## Update 2026-08-05 (fourth) — step 3 attempted; the blocker is not where this document says

Status at this checkpoint: **step 3 not landed.** Attempted, measured, and
reverted. This historical update is superseded by the later landed
`SharedImmutable` updates below; what follows replaces "it is wiring plus a
lowering rewrite, not new analysis" with a number and a specific obstacle,
because that sentence is true and materially understates the wiring.

### What was tried

`shared_handle_rust_type` in `src/slot_render.rs`: wrap a `SharedImmutable`
type's rendered `RustType` in `std::rc::Rc<…>`, hooked into
`rust_type_for_java_type_with_facts_and_representations` — the rendering entry
point that has the plan.

`SharedImmutable` was chosen exactly as this document recommends: `Rc<T>` with no
`RefCell`, so reads and `&self` calls work through auto-deref and there is no
borrow discipline to get wrong.

### The result, and what it proves

At this checkpoint, `scripts/gate.sh --all` had **zero deltas**. Not "no
regressions" — the hook had no effect at all. `known-gaps/Identity.java` still emitted
`let a: IdentityKey = IdentityKey::new(5);`, unwrapped.

That is the finding: **the Rust type of a source class is never rendered through
a representation-aware path.** The plan-carrying entry point exists and is
consulted 11 times, and none of those 11 are where a source class's type is
decided.

### The actual wiring cost, counted

Type rendering has two entry points, and the split is not even:

| variant | call sites |
|---|---:|
| `rust_type_for_slot_with_facts` / `…_java_type_with_facts` (**no plan**) | **49** |
| `…_with_facts_and_representations` (has the plan) | 11 |

By file, for the facts-only variant: `lower.rs` 12, **`item_assembly.rs` 12**,
`slot_render.rs` 7, `stdlib.rs` 7, `representation_lowering.rs` 4, and five
others with 1–2 each.

`item_assembly.rs` is the one that matters and the worst placed: it renders
**struct fields, parameters and return types**, and it mentions
`representations` exactly **zero** times. Its whole public surface takes
`(shape, facts)`.

### Why it cannot be done incrementally

A type's Rust spelling has to change at *every* site at once. Render a field as
`Rc<Key>` while a parameter is still `Key` and the output does not compile — so
unlike every other migration in this audit set, there is no monotone
"tag-first, text-fallback" path where a half-done state is merely less precise.
The switch is atomic.

That is what makes this expensive, and it is not "a lowering rewrite" — it is a
signature change across 49 call sites in 10 files, which must land before any
behaviour changes.

### The order that makes it verifiable

This document's own step 1 discipline applied, one level down:

1. **Thread the plan through all 49 sites without consulting it.** A pure
   signature change; the gate must show zero deltas. Large, mechanical, and
   provably inert.
2. **Then flip the switch for `SharedImmutable` only** — 19 types — together
   with 0028's identity lowering, since (per the previous update) the tier had
   no observable behaviour without it and `known-gaps/Identity.java` was the
   acceptance test.
3. `SharedLocal` is **4 types** and can wait, or be reconsidered entirely.

Step 1 was the honest next unit of work at this point. Later sections supersede
this path; the representation-aware type/value paths needed for `SharedImmutable`
and 0028 are now landed.

### Reverted

The hook was removed rather than left in place. An inert hook that looks like an
implementation is worse than no hook: it would read as "shared representation:
started" when nothing reaches it.

---

## Update 2026-08-05 (fifth) — step 3 implemented end to end; it works, and it costs 17 tests

The previous update said the blocker was threading `RepresentationPlan` through
49 sites. **That was the wrong diagnosis.** `FactSet` already reaches all 49,
and it already carries per-type decisions (`HierarchyStrategy`,
`TypeCapabilityOverride`). Putting `StoragePlan` there made the storage decision
visible everywhere for one field and one population line.

So step 3 was implemented, and it **works**:

```rust
let a: std::rc::Rc<IdentityKey> = std::rc::Rc::new(IdentityKey::new(5));
let b: std::rc::Rc<IdentityKey> = a.clone();          // Rc::clone — one object
let c: std::rc::Rc<IdentityKey> = std::rc::Rc::new(IdentityKey::new(5));
std::rc::Rc::ptr_eq(&a, &b)   // true
std::rc::Rc::ptr_eq(&a, &c)   // false
```

`tests/differential/Identity.java` — promoted from the known-gap directory after
0028 landed — **passes against the Java oracle**: both toolchains print
`5,true,false,false`. That is 0002's aliasing, 0028's `==`, and 0028's
`.equals()` half, all satisfied for the `SharedImmutable` tier.

Four pieces, each small: the plan on `FactSet`; a wrap in
`rust_type_for_java_type_with_effective_facts`; `Rc::new` at the one source
constructor site; and `Rc::ptr_eq` for `==` and for an inherited `equals`.

### Two soundness defects it exposed, both now fixed

**`mutated` must over-approximate, and did the opposite.** It saw field writes
through `this` and `this.f` only, so `arr[i].f = v` and `other.f = v` were
invisible. A type mutated that way was classified `SharedImmutable`, became
`Rc<T>` with no `RefCell`, and produced

```
error[E0594]: cannot assign to data in an `Rc`
```

This document's "over-sharing costs a pointer indirection" argument does not
cover it: **under-detecting mutation does not cost an indirection, it does not
compile.** `mutation_provably_absent` now records whether the walk could see all
writes, and `SharedImmutable` requires it. A field write the walk cannot
attribute clears the flag for every type.

**Enums and interfaces were in the lattice at all.** A Java `enum` constant is a
singleton by construction — nothing to alias, no identity question — and it
lowers to a Rust `enum`, where a shared handle breaks `match`, derived equality
and the switch lowering. That alone was 10 of the initial 32 failures. The plan
is now classes only.

### What it still costs, and why it was reverted

After both fixes: `differential` green, `slot_shape_design` −2, **`idiom_gaps`
538 → 521**. Seventeen regressions, and they do not cluster:

```
borrowed_downcast_source_subtype_argument_upcasts_after_clone_recovery
reused_source_subtype_argument_upcasts_after_clone_recovery
constructor_nested_arg_reads_assigned_field_without_cloning_self
implicit_source_super_constructor_initializes_inherited_fields
source_self_referential_instance_fields_use_boxed_optional_storage
source_borrowed_getter_initializers_recover_owned_locals
source_interface_object_getter_returns_borrowed_field
generic_source_map_storage_return_adds_clone_bound
emitted_source_call_uses_authoritative_owned_param_signature
context_translation_does_not_emit_opaque_facade_for_sibling_source_type
inherited_source_summary_field_types_emit_opaque_support_shells
source_summary_direct_field_type_emits_opaque_support_shell
private_static_singleton_getter_lowers_to_owned_source_value
erased_object_set_uses_hash_free_vec_carrier
exception_self_as_throwable_context_argument_uses_facade_boundary
foreach_lambda_throw_wraps_enclosing_method_in_result
nullable_custom_equals_method_uses_null_safe_custom_equals
```

Each is a separate place that assumes a source class's Rust type is the bare
struct — subtype upcasts, clone recovery, super-constructor field
initialisation, self-referential `Box` storage, borrowed getters, facade shells.
That is the real cost of the change, and it is **the list, not a number**: it is
a work queue with seventeen entries, each individually small.

The switch was reverted so the gate stays green. What is kept is everything that
is inert or independently correct:

- `FactSet::storage_strategies` and its population — the plumbing, verified
  inert;
- the constructor-mutation and escape-precision corrections (previous update);
- `mutation_provably_absent`;
- classes-only.

### Revised recommendation

Do **not** treat step 3 as a design problem any more. It is implemented, it is
four small pieces, and the acceptance test passes. What remains is working the
seventeen-item list above, which can be done one at a time with the gate as the
check — unlike the switch itself, those *are* individually attributable.

The realistic sequence: reapply the four pieces on a branch, then fix the list
top-down. `SharedLocal` (4 types) still needs `Rc<RefCell<T>>` and the
never-hold-a-borrow-across-a-call rule, and is still last.

---

## Update 2026-08-05 (sixth) — the 17 triaged, and one of them is a design conflict with 0030

The seventeen-item list from the previous update was worked. Triaging it changes
what it is: **five** are genuinely broken output, **eleven** are tests asserting
the old spelling, and **one** of the five is not a wiring bug at all.

### `Rc` and process-wide statics are incompatible

```
error[E0277]: `Rc<Standards<NamedLine>>` cannot be sent between threads safely
  --> static STANDARDS_REGISTRY_LINES:
       std::sync::Mutex<Option<std::rc::Rc<Standards<NamedLine>>>> = …
  = note: required for `Mutex<Option<Rc<…>>>` to implement `Sync`
  = note: shared static variables must have a type that implements `Sync`
```

This document anticipates `Sync` propagation, but attributes it to *threading*:

> **`Sync` propagates.** A `static Mutex<T>` requires `T: Send`, which pushes
> `Send`/`Sync` requirements through the object graph.

The trigger is not threading. [0030](done/0030-java-statics-are-thread-local.md)
made every Java `static` a process-wide `std::sync::Mutex`, which is required
for correctness and is not negotiable — so **any type reachable from a Java
`static` field must be `Send + Sync`**, in a single-threaded program, with no
`Thread` anywhere in the source.

`Rc` is neither. So `SharedImmutable` cannot be `Rc<T>` for such a type; it must
be `Arc<T>`.

That is a change to the lattice, not to the wiring. The tier decision needs a
fourth input — *reachable from static storage* — and it selects `Arc` for the
same reason `threaded` does, independently of any thread existing. Concretely:

```
SharedImmutable + (threaded ∨ static-reachable)  ->  Arc<T>
SharedLocal     + (threaded ∨ static-reachable)  ->  Arc<Mutex<T>>
```

`SharedSync` in this document is defined by threading alone and is measured at
**0** types. Adding static-reachability will make it non-empty, and the
sequencing note that "`SharedSync` is not on the critical path" is therefore
wrong — it is on the critical path for any corpus with a static registry, which
is a common Java shape.

### The other four are the conversion machinery

```
CompoundLine: From<Rc<FormatLine>> is not satisfied      (subtype upcast)
Box<dyn Any>: From<Rc<FormatLine>> is not satisfied      (erasure)
mismatched types                                          (custom equals, ×2)
```

All one thing: a conversion that used to see `T` now sees `Rc<T>` and does not
know to reach through it. The handle is transparent for *reads* (auto-deref) and
for *cloning* (`Rc::clone`), and opaque for every **conversion** —
`From`/`Into`, erasure to `Box<dyn Any>`, and supertype upcast. Those are the
sites the change actually has to teach, and there are four of them, not
seventeen.

### And eleven stale expectations (historical)

The remaining eleven asserted the old spelling and were later updated —
including `source_self_referential_instance_fields_use_boxed_optional_storage`,
which asserted `Option<Box<Node>>`. That one changed as predicted: with a shared
handle the `Box` is redundant, because `Option<Box<Rc<T>>>` is an indirection
through an indirection.

### Revised cost, and the revised order

| | count | kind |
|---|---:|---|
| conversion sites to teach | **4** | real work |
| stale test expectations | 11 | update |
| `Rc` vs static storage | 1 | **design change to this document** |

So the honest sequence is now:

1. **Settle the tier scheme first.** Add static-reachability to the lattice and
   accept that `Arc` — not `Rc` — is the common case wherever Java statics hold
   objects. Doing the wiring before this would produce code that has to be
   redone.
2. Teach the four conversion sites.
3. Update the eleven expectations.

Step 1 was not visible before this pass, and it is why step 3 has been reverted
a second time rather than pushed through: landing `Rc` now would bake in a
handle that 0030 has already made wrong for a whole class of types.

---

## Update 2026-08-06 (seventh) — the handle hierarchy, and the residual is now 14

Step 3 was reapplied with the corrections from the previous passes, verified,
and reverted again. What changed is the design, and the residual count.

### The `Rc`/`Arc` conflict is resolved by a hierarchy, not by picking one

The sixth update established that `Rc` cannot be used for anything a Java
`static` reaches, because a Rust `static` must be `Sync` and
[0030](done/0030-java-statics-are-thread-local.md) makes every Java `static` a
real one. Demonstrated, rather than argued — `Mutex<Option<Rc<T>>>`,
`OnceLock<Rc<T>>` and a bare `static Rc<T>` all fail, and the `const` escape does
not apply because `Rc::new` is not `const`.

The answer is **not** to use `Arc` everywhere. `StoragePlacement` now carries
three independent axes, and the handle follows from one of them:

```text
rust_type(T) = handle( cell( T ) )

  cell(T)   = T | RefCell<T> | Mutex<T>     interior_mutable, then sync
  handle(X) = X | Rc<X>      | Arc<X>       shared,           then sync
```

`RefCell`/`Mutex` and `Rc`/`Arc` are chosen by the **same** `sync` bit, so they
cannot disagree — an invariant the previous `StorageForm` match maintained by
hand. `sync` is `threaded ∨ static_reachable`, the second being a transitive
closure over fields from any `static`.

Measured over the corpus: **zero** types need `Arc`. All 15 shared types are
`Rc`, so the cheap rung covers everything today and the atomic rung exists for
correctness rather than for use. That is the argument for the hierarchy over
"just use `Arc`".

The type form and the value form are derived from the same placement axes:
`wrap` gives `Rc<RefCell<T>>`, while `slot_render::storage_placement_construction_expr`
builds `Rc::new(RefCell::new(v))` as structured `RustExpr` calls after a
hand-written construction string got the nesting wrong. A law test asserts they
nest identically, alongside the lattice laws (commutative, associative,
idempotent, extensive) checked exhaustively over the 8-element domain.

### The acceptance test passes

`tests/differential/Identity.java`, against the Java oracle: both toolchains
print `5,true,false,false`. Aliasing, `==` and the inherited `.equals()` half,
all satisfied for the shared tier. This fixture was promoted when 0028 landed;
older references to `known-gaps/Identity.java` below are historical.

### The residual, and where it now sits

Reapplied with the narrower rules — the wrap at **owned slots only**, since a
borrowed slot sees through the handle — the cost is **14** `idiom_gaps` tests,
down from the 17 the fifth update measured. Every one is a *conversion*:
`E0308` where a downcast yields `T` into an `Rc<T>` slot, `E0277` where an
upcast wants `T` from an `Rc<T>`.

First, a `HandleAdaptation` sibling field was added to `SlotExprConversionPlan`
to answer that in one place — the outer plan holds both shapes and the facts, so
the decision seemed to belong there and applying it seemed like execution. It
did not reduce the count, and **the reason is now known**:

`apply_slot_conversion_plan` has **four** callers, and two of them
(`coercion.rs:711`, `coercion.rs:3670`) take a bare `&SlotConversionPlan` with
no access to a sibling field on the outer plan. Putting the crossing beside the
conversion meant only the applier that was taught about it could see it — the
same "one decision, several places" shape this audit set keeps finding, arrived
at by writing it. That sibling field has since been deleted; the current inert
plumbing is only the in-plan variant below.

Fixed by moving the crossing **inside** the plan:

```rust
SlotConversionPlan::Handled { handle: HandleAdaptation, inner: Box<SlotConversionPlan> }
```

`apply_slot_conversion_plan` unwraps it, so all four callers get the crossing
whether or not they know it exists. This is landed and **inert** — with values
held inline `plan_handle_adaptation` returns `None`, so no `Handled` is ever
constructed and the gate shows zero deltas.

The re-measurement was then run, and it gives a **negative result worth keeping**:
with all five pieces on, the count is 15 — the crossing inside the plan did not
reduce it. Tracing one failure shows why:

```rust
let that: std::rc::Rc<FormatLine> = *o.downcast::<FormatLine>().expect("invalid Java cast");
```

The downcast is built by the instanceof/cast binding path, which constructs its
`RustExpr` **directly** rather than routing through `plan_slot_conversion`. So
`apply_slot_conversion_plan` was never the single choke point; it is one of
several places that build a converted expression, and the others do not consult
a plan at all.

That is the corrected shape of the residual. It is not "teach the conversion
machinery" and not "one enum variant": it is **teach each expression-building
path that produces a value for a slot**, of which the cast/instanceof binding is
one and the four plan appliers are others. The `Handled` variant is still the
right home for the ones that *do* plan, and is kept and inert.

### Kept, and independently useful

- `StoragePlacement`, its three axes, structured construction via
  `slot_render::storage_placement_construction_expr`, and the law tests.
- The `Rc`/`Arc` hierarchy and static-reachability.
- `FactSet::storage_placement_for` — the plan visible to every renderer.
- `mutation_provably_absent`, the classes-only restriction, and the
  constructor-mutation and escape-precision corrections.

### Honest status

Four audits — 0002, 0002b, 0023, 0028 — are one change. It is implemented,
tested against the oracle, and not landed, because landing it means 14
regressions in a suite whose value is that it sits at baseline.

It is no longer a design question. The five pieces to re-enable are named in
this file and the crossing is in place for the planned paths.

One further correction, from following that query. The erased-object downcast
*does* go through a plan — `apply_erased_object_downcast` is inside
`apply_shape_coercion_plan`. The gap was one level up again: the crossing was
decided in `plan_slot_conversion`, while plans are also built directly by
`slot_conversion_plan_with_facts`, which several callers use. Moving the
decision into that function reaches every producer.

With that done and step 3 on, the residual got **worse** — 15 → 16. So the
adaptation *rule* is not yet right: it wraps or unwraps at sites where neither
is wanted. That is the actual remaining work, and it is not a plumbing question.

### What five attempts have established

Each attempt corrected the diagnosis, and the corrections are the durable part:

| attempt | believed blocker | what it actually was |
|---|---|---|
| 1 | thread the plan through 49 render sites | `FactSet` already reaches all of them |
| 2 | `Rc` everywhere | `Rc` is illegal under a Java `static`; needs the `Rc`/`Arc` hierarchy |
| 3 | 17 diffuse regressions | 4 conversions + 11 stale expectations + 1 design conflict |
| 4 | teach `apply_slot_conversion_plan` | it has four callers; the crossing belongs *inside* the plan |
| 5 | the downcast bypasses the planner | it does not; the *plan builder* was the wrong level, and fixing that exposed that the rule itself is wrong |

The rule is where it stops. `Wrap` when the target is handled and the source is
not, `Unwrap` in the reverse — that is too coarse, and the sites it damages have
not been characterised. Characterising them is the next unit of work, and it
needs the failure list read one by one rather than another structural move.

## What the sixth attempt measured

The five earlier attempts each ended in a revert with a *guess* at the blocker.
This one ends with numbers. All figures are `idiom_gaps`, baseline **538/146**.

| configuration | result | vs baseline |
|---|---:|---:|
| crossing only, `Wrap` + `Unwrap`, no type wrap | 534/150 | −4 |
| crossing only, `Wrap` alone, no type wrap | 538/146 | **0** |
| type wrap + `Wrap` alone | 525/159 | −13 |
| type wrap + `Wrap` + `Unwrap` | 522/162 | −16 |

### `Unwrap` is pure loss — delete it from the lattice

Diffing the failure *sets* between the last two rows, rather than the counts:

- tests `Unwrap` moves fail → pass: **none**
- tests `Unwrap` moves pass → fail: **three**
  (`source_borrowed_getter_initializers_recover_owned_locals`,
  `source_interface_object_getter_returns_borrowed_field`,
  `source_receiver_generic_result_upcasts_to_supertype_assignment_slot`)

So `HandleAdaptation::Unwrap` is not a rule that needs refining; it is the wrong
mechanism, and the asymmetry has a structural reason:

> A conversion plan can **wrap** — it constructs the value, so it owns the point
> where the handle is created. It cannot **unwrap**, because the sites that need
> the unwrap are not conversion plans at all.

The sites are `Coercion`s, applied by `coercion::apply_one(expr, coercion)` —
which takes no `FactSet` and therefore *cannot* ask whether a type is shared.
That is why five attempts at moving the decision to a better level all failed:
every level tried was still inside the plan, and the plan never reaches these.

### The 13 regressions, classified

Split by whether the generated Rust still compiles:

**8 are stale expectations, and the new output is the better one.** Two checked
in full:

- `source_self_referential_instance_fields_use_boxed_optional_storage` asserts
  `parent: Option<Box<SelfLinkedNode>>`; the wrap emits
  `Option<Rc<SelfLinkedNode>>`. `Box` gives each node *exclusive* ownership of
  its parent, which no Java object graph has. The test encodes 0002.
- `private_static_singleton_getter_lowers_to_owned_source_value` — the name is
  the defect. The wrap emits
  `static CONFIG_INSTANCE: Mutex<Option<Arc<Config>>>` with
  `get_instance() -> Arc<Config>`; the asserted form returns `Config` *by value*,
  so every caller gets a copy and the singleton is not one. Note `Arc`, not
  `Rc` — the static-reachability closure picked the tier correctly with no
  special-casing.

**5 are real compile breaks, and three are one defect** —
`CompoundLine: From<Rc<FormatLine>>`, `Box<dyn Any>: From<Rc<FormatLine>>`:
`lines.push(line.into())` where `line` is now a handle. The conversion is
applied *to the handle* instead of to its contents.

### What lands this

1. Delete `HandleAdaptation::Unwrap` (measured pure loss above). **Done in the
   follow-up audit pass**; only `Wrap` remains in the inert `HandleAdaptation`
   enum.
2. Teach the five upcast sites. Two candidate designs, neither yet built:
   - resolve the placement where the `Coercion` is *constructed*
     (`external_conversion_adapter_policy` has a `&FactSet`) and carry it in the
     variant, so `apply_one` unwraps without needing facts; or
   - generate `impl From<Rc<Sub>> for Super` alongside the existing `From<Sub>`,
     so the conversion surface follows the storage form and no unwrap is needed.
     This also lets `Box<dyn Any>` hold the `Rc` and *keep* identity through the
     erasure, which the unwrap would destroy.

   The second design is the better one for the three `From` cases, and the
   impls are generated in one place — `source_supertype_conversion_impl_for_pair`
   in `src/lower.rs`. But it does **not** stretch to cover the erasure case, and
   the reason is a hard language rule rather than a design choice:

   ```rust
   impl From<FormatLine> for Box<dyn Any> { .. }      // generated today; compiles
   impl From<Rc<FormatLine>> for Box<dyn Any> { .. }  // error[E0117]
   ```

   `Rc` is not `#[fundamental]` (only `Box` and `&` are), so `Rc<Local>` does
   not count as a local type and the orphan rule rejects the companion. Checked
   against real rustc, not assumed.

   So the erasure site needs the *other* design: emit `Box::new(handle)` in place
   of `handle.into()`. That is the identity-preserving lowering anyway — the
   erased carrier holds the `Rc`, and a downcast recovers the same object rather
   than a copy — so the two designs are not competitors. Locally-emitted
   supertypes take the companion impl; `Box<dyn Any>` takes the changed
   coercion.
3. Update the 8 expectations, each of which asserts a 0002 symptom.

Steps 2 and 3 are ordinary work. Step 1 is the finding: the previous five
attempts were all trying to place a rule that should not exist.

## The seventh attempt: three of five fixed, and the actual blocker found

Working the five compile breaks individually — which is what the previous
section said to do — closed three of them with changes that are principled
rather than local patches, and turned the remaining two into a single sentence.

Progression, `idiom_gaps`, baseline **538/146**:

| step | result |
|---|---:|
| type wrap on (+ `Box` suppression) | 525/159 |
| + `From<Rc<Sub>>` companion impls | 526/158 |
| + `Box::new(handle)` at the erasure | 527/157 |
| + downcast recovers the handle | **528/156** |
| + wrap fresh `new` at slot boundaries | 527/157 (worse — dropped) |

### What fixed the three

- **`From<Rc<Sub>>` companions**, generated next to the existing `From<Sub>` in
  `source_supertype_conversion_impl_for_pair`, gated on the subtype being
  shared. Not emitted for `Box<dyn Any>` — E0117, above.
- **`Box::new(handle)` instead of `handle.into()`** at the two upcast sites that
  target an erased carrier. This is the identity-preserving lowering: the
  carrier holds the shared handle, so a downcast recovers *the same object*
  rather than a copy.
- **The downcast asking for the handle** (`downcast::<Rc<T>>`) — the paired half
  of the above. On its own it is silently wrong: `plausible_rust_nominal_type_name`
  judged a target by its *first character*, so `std::rc::Rc<FormatLine>` was
  rejected and the downcast was **dropped without a diagnostic**, letting the
  value flow on at the wrong type. That check now judges the innermost
  identifier. **Kept — it is an independent latent bug** (any qualified or
  generic downcast target hit it) and is gate-clean on its own.

### The blocker, stated exactly

The last two breaks are both a producer handing back a bare `T` where the slot
now holds `Rc<T>`:

```rust
let mut reader: std::rc::Rc<Reader> = future.get();          // external facade
pub fn lines() -> std::sync::Arc<Standards<NamedLine>> {
    return Standards::new();                                  // constructor
}
```

Fixing these needs one question answered at the use site: *is this expression
already behind a handle?* It cannot be answered.

> `RustSlotShape` records `java_ty` and `ownership`. It does **not** record
> whether the value sits behind a handle, because the handle is added by
> `slot_render` — *after* the shape, at rendering time. Two values with
> identical shapes render as `Rc<FormatLine>` and `FormatLine`.

That is why all six earlier attempts failed to place the crossing, and why no
amount of moving it helps: `Wrap` and `Unwrap` are both functions of a
distinction the IR does not carry. Restricting the wrap to `new` — the one
producer that is *structurally* known to be bare — was tried and measured
**worse** (528 → 527), because a `new` feeding a slot that is already handled
then double-wraps.

### So the next change is not in the crossing at all

Move the handle **into the shape**: a field on `RustSlotShape` set where the
storage decision is made, with `slot_render` reading it instead of re-deriving
it from `FactSet`. Then "is this a handle" is answerable at every site, the
crossing becomes a total function of two shapes rather than a guess, and the two
remaining breaks — plus the ~11 the corpus will have — are decided rather than
patched. This is the same defect [0007](0007-rust-ir-is-mostly-opaque-strings.md)
describes: a property that exists only in rendered text cannot be reasoned about.

Everything except the `plausible_rust_nominal_type_name` fix was reverted; the
gate is at baseline on all 18 targets.


## Update 2026-08-06 — four of the five breaks closed; the fifth names the change

Re-applied the stack and worked the remaining breaks one at a time. Where the
previous round left two unexplained, this round closed one more and turned the
last into a specific, non-obvious requirement.

| step | idiom_gaps |
|---|---:|
| baseline | 538/146 |
| type wrap + `Box` suppression | 525/159 |
| + `From<Rc<Sub>>` companions | 526/158 |
| + `Box::new(handle)` at the erasure | 527/157 |
| + downcast recovers the handle | 528/156 |
| + construction wrap at the static initializer and the return slot | **529/155** |

Nothing regressed at any step after the first; each addition was checked by
diffing failure *sets*, not counts.

### Two dead ends, both measured

- **Wrapping every `new` at `lower_expr_for_slot`** fixed nothing and broke
  `exception_self_as_throwable_context_argument_uses_facade_boundary`. The
  reason is worth keeping: the return path calls
  `lower_expr_for_slot_with_refinements` *directly*, so the wrapper is not on
  the path at all. Moving the wrap to the inner function did not help either --
  the value it needed to reach is not lowered from Java source.
- **Wrapping the slot default** (`default_expr_for_slot_with_effective_facts`)
  changed nothing either way. Correct in principle, unreachable in practice for
  these cases.

What did work was narrower and better justified: a **static field initializer**
and a **folded static read in return position** are both fresh constructions, so
they can be wrapped without answering "is this already a handle?". That is the
same argument the design gives for `wrap_construction`, applied at the two
places where a bare construction actually meets a handle-typed slot.

### The last break states the requirement precisely

```rust
let mut reader: std::rc::Rc<Reader> = future.get();   // expected Rc<Reader>, found Reader
readers.insert(name.clone(), reader);                 // expected Reader,     found Rc<Reader>
```

`Reader` is a source class and genuinely shared -- it is aliased into a map. The
two errors point in **opposite directions**, which is the whole finding: the
*local* slot is handle-wrapped and the *map element* is not. The wrap is applied
by `shared_handle_slot_rust_type`, which needs `OwnershipShape` to know that a
borrow must see through the handle. Generic arguments are rendered from a
`JavaType` by `rust_type_for_java_type_with_effective_facts`, where no ownership
is available -- so element positions cannot be wrapped at the level the wrap
currently lives.

Making a shared type shared *everywhere* -- which is the only consistent rule --
therefore requires the handle decision to travel with something that carries
ownership. That is the `RustSlotShape` migration this file already names, and
this is now a second, independent argument for it: not just "the crossing cannot
be decided", but "the wrap cannot be applied uniformly".

Reverted to baseline; the re-apply script survives at
`<scratchpad>/apply_0002b.py` and reproduces 528/156 in one run.


### Tested and rejected: wrapping at the Java-type level

The obvious way to reach generic positions is to move the wrap down into
`rust_type_for_java_type_with_effective_facts`, on the reasoning that a generic
argument is always *owned* by its container -- a borrow only ever appears at the
top level of a slot -- so no `OwnershipShape` is needed there.

The reasoning about elements is correct. The change is still wrong, and measured
**522/162**, worse than the slot-level wrap's 525/159. That function renders the
top-level type of *borrowed* slots too, so it produced `&Rc<T>` where `&T` was
wanted, breaking exactly the two tests `Unwrap` broke earlier
(`source_borrowed_getter_initializers_recover_owned_locals`,
`source_interface_object_getter_returns_borrowed_field`) -- the same two, from a
different direction, which is a good sign the borrow boundary is the real edge.

So the wrap needs **both** levels, distinguished:

- top level, ownership-aware — `shared_handle_slot_rust_type`, already written;
- nested element/argument positions, unconditional — not yet placed, because the
  generic arguments of a collection are not rendered through a single
  identifiable call. `class_token_arg_rust_type` handles class tokens,
  `SliceView`/`Optional` handle their own elements, and the collection carriers
  render their arguments elsewhere again.

Finding that one place — or introducing it — is the next concrete step, and it is
smaller than the full `RustSlotShape` migration. The migration is still needed
for the *crossing*; it is not needed for the *wrap*.

## Update 2026-08-06 (second pass) — all five compile breaks closed

The generic-argument position turned out to have a single, already-named home:
`rust_type_for_java_generic_arg_with_effective_facts` (9 call sites, all of them
argument positions). Wrapping there — unconditionally, since a generic argument
is always owned by its container — fixed the last break:

```rust
let mut readers: HashMap<String, std::rc::Rc<Reader>> = HashMap::new();
let mut reader: std::rc::Rc<Reader> = future.get();
readers.insert(name.clone(), reader);          // now type-checks
```

With that, **529/155, and the nine remaining failures appeared assert-only**.
That reading held until the sixth expectation exposed invalid generated Rust
below. The wrap therefore needs exactly two placements, and the earlier "cannot
be applied uniformly" conclusion was wrong:

| position | wrap | why |
|---|---|---|
| top level of a slot | ownership-aware (`shared_handle_slot_rust_type`) | a borrow must see through the handle |
| generic argument | unconditional (`..._generic_arg_...`) | always owned by its container |

Five of the nine expectations were then updated and pass. Two were renamed,
because the names asserted the defect: `..._use_boxed_optional_storage` →
`..._use_shared_handle_storage`, and `..._lowers_to_owned_source_value` →
`..._lowers_to_shared_handle`.

### What stopped it: a latent bug the wrap made visible

Updating the sixth expectation surfaced **invalid generated Rust**:

```rust
let mut __java_self = ChildProblem {
    max: max.clone(),
    context: std::rc::Rc<ExceptionContext>::new(),   // not valid Rust
};
```

Something in constructor lowering renders the field's Rust **type** and appends
`::new()`. With a bare `ExceptionContext` that happens to read correctly, so the
bug has always been there and never showed; the moment the type is `Rc<T>` it
emits a syntax error. (`Rc::<T>::new()` would be the valid spelling — the
generated form is not.)

`implicit_source_super_constructor_initializes_inherited_fields` does **not**
call `cargo_check_generated_modules`, which is why an assert-only failure was
hiding a non-compiling program. That is [0018](done/0018-the-compile-error-metric-cannot-see-silent-failures.md)'s
blind spot inside the test suite itself.

Not located: it is not `static_storage_initial_value_expr`,
`lower_static_initialized_singleton_field_for_slot`, `render_slot_default_policy`,
or `plan_constructor_field_values_*` — all of those build the path from
`rust_name_for_java_named_type`, which is a name mapper and cannot produce
`Rc<…>`. Finding the real site is the next step, and it is worth doing on its own
merits: **a construction path that is a rendered type is a bug with or without
0002b.**

### Landing checklist, all of it now specific

Historical note: this reverted-pass checklist was superseded by the third pass
and the 2026-08-06 landing notes below. The constructor-path defect named here
was found and fixed; the remaining live work is no longer this four-item
checklist.

1. Find and fix the `{rendered_type}::new()` construction path above.
2. Add `cargo_check_generated_modules` to
   `implicit_source_super_constructor_initializes_inherited_fields` so it cannot
   regress silently again.
3. Update the last four expectations (`constructor_nested_arg_reads_assigned_field_without_cloning_self`,
   `emitted_source_call_uses_authoritative_owned_param_signature`,
   `foreach_lambda_throw_wraps_enclosing_method_in_result`,
   `implicit_source_super_constructor_initializes_inherited_fields`).
4. Record the new baseline; 0002, 0023 and 0028 then unblock in that order.

Reverted to baseline. `<scratchpad>/apply_0002b.py` re-applies steps 1-5 and,
with the generic-arg wrap above, reproduces 529/155 in one run.

## Update 2026-08-06 (third pass) — 535/149, three left, and the last defect named

The constructor-path bug from the previous pass was found:
**`src/call_lowering.rs:978`** builds a constructor's owner path by rendering the
owner's *slot type*:

```rust
let owner_type = crate::emit::emit_type(&rust_type_for_slot_with_facts(&target.owner_shape, facts));
```

A path position is not a value position. With a bare `T` that reads correctly by
accident; the moment the slot type carries a handle it emits
`std::rc::Rc<ExceptionContext>::new()`, which is not valid Rust. Fixed by
rendering the owner without storage decoration
(`path_type_for_slot_with_facts_and_representations`), which closed
`implicit_source_super_constructor_initializes_inherited_fields` and took the run
to **530/154**. That fix is worth landing on its own: a constructor path built
from a rendered slot type is wrong regardless of 0002b.

Updating six expectations then reached **535/149 — three failures left**:

| test | why |
|---|---|
| `constructor_nested_arg_reads_assigned_field_without_cloning_self` | `Some(Wrapper::new(..))` into an `Option<Rc<Wrapper>>` field |
| `emitted_source_call_uses_authoritative_owned_param_signature` | not yet read |
| `foreach_lambda_throw_wraps_enclosing_method_in_result` | not yet read |

### The one remaining shape of defect

All the value-side wraps that landed share a property: they act where a *fresh
construction* meets a handle-typed slot, and each was placed at a different
point — the slot default, the static field initializer, the folded static read.
The constructor-field case does not yield to the same treatment:

```rust
// wrapping before the optional wrapper:
Some(std::rc::Rc::new(Some(Wrapper::new(..))))   // the Some is applied twice
// wrapping after it:
std::rc::Rc::new(Some(Wrapper::new(..)))         // handle outside the Option
```

The wrap has to go **between** the element value and its `Some`, and that point
is inside `lower_expr_for_slot`, which does not distinguish "the element of an
optional slot" from "the slot". Wrapping every `Expr::New` there was tried twice
and is a net loss (it breaks
`exception_self_as_throwable_context_argument_uses_facade_boundary` and fixes
nothing else).

So the value side needs one thing the type side already has: a way to name the
**element position** of an optional slot as a place where a value can be
produced. The type side got this for free because `rust_type_for_slot_*` recurses
through `Option` before rendering. The value side has no such recursion — and
that, not the crossing, is the last structural gap.

Reverted to baseline. The stack is reproducible: `<scratchpad>/apply_0002b.py`
plus the generic-argument wrap and the `call_lowering.rs` path fix gives 530/154
before any expectation edits.

## Update 2026-08-06 (Codex follow-up) — unknown-boundary escapes also include field arguments

The `UnknownBoundary` producer consumed local bindings passed to unresolved
calls, but ignored the parallel `FieldUse::PassedToCall` facts that usage
already records. A field value crossing an unmodelled call can be retained by
that callee just like a local value, so the escape input was still biased low
for `helper.consume(this.cached)` / `helper.consume(cached)`.

`collect_unknown_boundary_escapes` now walks unknown-boundary field arguments
and attributes the escape to the field's declared storage type, using the same
array/generic-aware helper as field storage. This means `Cell[] cached` and
`List<Cell> cached` passed to an unknown call mark `Cell` as
`UnknownBoundary`, not merely the owner type.

Pinned by storage-strategy tests for a direct field argument and a generic field
argument.

## Update 2026-08-06 (Codex follow-up) — constructor mutation exemption was too broad

The storage plan correctly stopped counting direct constructor writes to
`this.field` as post-sharing mutation, but the exemption was too wide: it also
skipped constructor writes through local variables and parameters. A constructor
body can mutate a pre-existing object passed as a parameter:

```java
Owner(Cell cell) {
    cell.v = 2;
}
```

That is not initialization of the newly constructed owner. The object may
already be shared, so `Cell` must still be considered mutable after sharing.

The fact walk now carries a local type environment from parameters and variable
declarations. Constructor field writes are exempt only when the receiver is
`this`; field writes through a typed local or parameter attribute mutation to
that receiver type. Pinned by
`constructor_write_through_parameter_still_makes_that_type_mutable`.

## Update 2026-08-06 (Codex follow-up) — records, loop locals, and wildcard fields are in the storage input

Three more under-approximations were still in the report-only storage plan.

Records are Java reference objects, but `collect_declared_types` admitted only
`TypeKind::Class`. Records now participate in the same lattice as classes; enums
and interfaces remain excluded for the reasons above.

The statement walk treated `for` like a plain block and skipped initializer
declarations, so `for (Cell b = a; ...) { b.v = 2; }` missed both the local alias
and the receiver type for the mutation. `for` initializers and enhanced-for
locals now seed a scoped local environment before walking the body.

Finally, stored generic contents now recurse through wildcard bounds:
`List<? extends Cell>` and `List<? super Cell>` can retain `Cell` values just as
`List<Cell>` can. Static reachability and field-storage tests cover both bound
directions.

## Update 2026-08-06 (fourth pass) — re-run against the current tree

Re-applied the stack on top of the committed tree. Two things had changed: the
constructor-owner-path bug is **already fixed** (`constructor_owner_path_type` in
`src/call_lowering.rs` now builds the path from the type *name*, which is
stronger than stripping decoration from a rendered slot type), and the shifted
tree moves the numbers.

Progression this pass, against the committed 538/146:

| step | result | regressions |
|---|---:|---:|
| stack re-applied | 523/161 | 15 |
| user-generic arguments left unwrapped | 526/158 | 12 |
| optional element handle on the construction plan | 526/158 | 12, but only **1** still fails to compile |

### Correction: the generic-argument wrap was too broad

The previous pass recommended wrapping *every* generic argument, on the grounds
that an argument is always owned by its container. Owned, yes — but that is not
sufficient:

```
error[E0599]: the method `repair` exists for reference `&Rc<Standards<Rc<FormatLine>>>`,
              but its trait bounds were not satisfied
```

`Standards<T extends Named>` instantiated with `Rc<FormatLine>` requires
`Rc<FormatLine>: Named`, which does not hold — the impl is on `FormatLine`. So
the wrap is safe for **collection elements** (containers we generate, no bounds)
and unsound for **user generic type parameters** (bounds we do not control).
Scoping it to the collection arms fixed three tests.

### The optional element position, solved

The element position did not need a new concept: `OptionalSlotConstructionPlan`
already models it. Adding `element_handle: Option<StoragePlacement>`, populated
where the plan is *built* (which has a `FactSet`) and applied where it is
rendered (which does not), produces the shape the handoff asked for:

```rust
__java_self.wrapper = Some(std::rc::Rc::new(Wrapper::new(..)));
```

The wrap is gated on an explicit `value_is_fresh_construction` flag threaded from
the `Expr::New` test, not inferred — inferring it is the unanswerable question.
Keeping the handle decision *on the plan* is also what stops
`Some(Rc::new(Some(..)))`: the `Some` and the handle are now one decision instead
of two that can disagree.

#### Follow-up 2026-08-26 — optional element fixture actually closed

Codex re-ran
`constructor_nested_arg_reads_assigned_field_without_cloning_self` and found the
fixture still failing. The generated constructor assignment had two residual
problems:

```rust
Some(std::rc::Rc::new(std::rc::Rc::<Wrapper>::new(Holder::new(..))))
```

The outer duplicate wrap came from treating `Rc::<T>::new(..)` as different from
`Rc::new(..)` in `storage_placement_construction_expr_is_wrapped`. The
constructor call itself came from rendering a constructor owner path through the
storage-decorated slot type, so a path position used `Rc::<Wrapper>::new` rather
than `Wrapper::new`.

Both are now fixed:

- storage wrapper detection recognizes generic path calls such as
  `std::rc::Rc::<Wrapper>::new(..)`;
- constructor owner paths are rendered from the source Java owner name, not the
  storage-decorated slot type.

The pinned idiom fixture now emits:

```rust
__java_self.wrapper = Some(std::rc::Rc::new(Wrapper::new(Holder::new(..))));
```

### The one remaining break is a design fork, not a missing fix

```rust
repair(*line.downcast::<std::rc::Rc<FormatLine>>().expect("invalid Java cast"))
//     expected `FormatLine`, found `Rc<FormatLine>`
```

Two rules that are each individually right now conflict:

- the erased carrier holds `Rc<T>`, so a downcast recovers *the same object*
  rather than a copy (this is what makes 0028 identity observable through
  erasure);
- user generic parameters stay bare, so `T extends Named` bounds still hold.

A value crossing from the first into the second needs an unwrap, and nothing can
currently decide that: the downcast result's shape does not record that it is a
handle. Reverting the erasure pair instead costs two other tests (13 vs 12), so
it is not the cheaper side.

**The fork, to be decided before the next attempt:**

1. *Wrap user generics too*, and generate blanket forwarding impls
   (`impl<T: Named> Named for Rc<T>`) so bounds are satisfied. Uniform, but the
   impls have to be generated for every source interface a type parameter is
   bounded by.
2. *Store bare values in the erased carrier*, accepting that identity is lost
   through erasure. Uniform and much smaller, but it gives up part of 0028 —
   the part the original identity fixture did not cover, since that fixture
   never erases.

Option 2 is the smaller move and keeps `SharedImmutable` landable; option 1 is
the one that finishes 0028 properly. This is a decision about scope, not a
technical unknown.

Reverted to the committed baseline; the gate is green on all 18 targets.

## Update 2026-08-06 (fifth pass) — the fork is decided, by evidence

The previous section left a fork: wrap user generics too (option 1), or store
bare values in the erased carrier (option 2). I recommended option 2 as the
smaller move and then implemented it. **It does not work, and the reason
generalises.**

Option 2 was built as one rule replacing the earlier erasure pair: unwrap the
handle at the erasure boundary, where both sides are visible. That much is fine
and fixed `erased_object_set_uses_hash_free_vec_carrier`. Two breaks remained,
and they are the same defect from two directions:

```rust
// E0614: the erasure unwrap fired on a value that was not a handle
line = (*self.formats.as_ref().unwrap().repair(..)).clone().into();
//     `FormatLine` cannot be dereferenced

// E0308: the downcast produced a bare value into a handle-typed slot
let that: std::rc::Rc<FormatLine> = *o.downcast::<FormatLine>().expect(..);
//        expected `Rc<FormatLine>`, found `FormatLine`
```

`repair` is declared on `Standards<T extends Named>`, so it returns a **bare**
`T` — because option 2 leaves user generic arguments unwrapped. So:

> Under option 2, a value whose Java type is shared may or may not be behind a
> handle, depending on **where it came from**. Under option 1 it always is.

That is the whole difference. Every wrap/unwrap decision in this design is
currently made from a `RustSlotShape`, which carries a type and an ownership and
no provenance. Option 1 keeps "shared type ⇒ behind a handle" a global
invariant, so those decisions stay derivable from the type. Option 2 makes them
depend on the producer, which is exactly the information the IR does not carry —
the same gap this file has hit from four other directions.

So the earlier framing was wrong. It is not "thorough versus small". It is:

- **option 1** — wrap user generic arguments too, and generate forwarding impls
  (`impl<T: Named> Named for Rc<T>`) for each interface a type parameter is
  bounded by. Cost: those impls. Benefit: the invariant holds, and every wrap
  site stays a pure function of the type.
- **option 2** — cheaper per site, but it removes the invariant, and then no
  amount of local cleverness makes the boundary cases decidable. Both failures
  above are unfixable without provenance.

**Recommendation: option 1.** The forwarding impls are mechanical and generated
from information already collected (the bounds are in the source), and they are
much less work than adding provenance to the IR.

Measured state at the end of this pass, against the committed 538/146: 525/159,
13 regressions, 11 of them assert-only stale expectations and 2 the breaks above.
Reverted; the gate is green on all 18 targets.

### Option 1's sharp edge, found while scoping it

The forwarding impls go where `dyn_trait_anonymous_impl` and
`boxed_dyn_trait_clone_impl` already go -- appended to each generated trait in
`src/representation_lowering.rs` (`trait_item_for_type`,
`trait_item_for_type_with_program`). That part is mechanical.

The edge is the receiver:

```rust
impl<T: Named> Named for std::rc::Rc<T> {
    fn id(&self) -> String { (**self).id() }        // fine
    fn set(&mut self, v: i32) { (**self).set(v) }   // impossible: Rc has no DerefMut
}
```

A trait with **any** `&mut self` method cannot have a forwarding impl for
`Rc<T>`. So option 1 is not purely additive: it feeds back into the tier
predicate. A type whose interface has a mutating method cannot be a wrapped
generic argument, which means `is_implemented()` -- currently
`!interior_mutable` -- has a second condition:

> a shared type must also have a forwardable interface surface, i.e. every trait
> it is used through must be `&self`-only.

That is checkable from the same information the placement analysis already reads,
and it is better found now than after the impls are generated. It also explains
why option 1 is not merely "more work": it narrows which types can take the
`SharedImmutable` tier, and the narrowing must be computed *before* the wrap is
applied, not discovered as a compile error afterwards.

Concrete order for whoever picks this up:

1. Extend the placement analysis with the forwardable-surface condition and
   report the new `SharedImmutable` census (it will be smaller than 19).
2. Generate the forwarding impls for the traits that qualify.
3. Re-wrap user generic arguments and restore the handle-carrying erasure pair.
4. Update the 11 stale expectations and record the baseline.

### Step 1 of option 1 is done and landed

`interface_surface_is_handle_forwardable` (`src/storage_strategy.rs`) computes
the new precondition, and `report_storage_disagreements` emits
`storage_strategy_surface_not_forwardable` for any shared type that fails it.
Output-inert: it adds a warning and changes no generated Rust. `--lib` 166 -> 169,
every other suite unchanged, `idiom_gaps` still 538/146 with identical failure
names.

The check earns its place by catching a case that cannot be read off
`TypeStorageFacts`:

```java
interface Shape { int size(); void grow(); }
class Fixed   implements Shape { /* never mutates */ }
class Growing implements Shape { public void grow() { s = s + 1; } }
```

`Fixed` is `SharedImmutable` -- `mutated=false`, `mutation_provably_absent=true`
-- and still cannot take the tier, because `Growing` forces `Shape::grow` to
`&mut self` and `Rc<T>` has no `DerefMut`. **The blocker is the trait's surface
across all implementors, not the candidate's own mutability.** Pinned by
`a_mutating_sibling_implementor_blocks_the_whole_trait`.

This also means the `SharedImmutable` census is a *join over implementors*, not a
per-type property, so the earlier "19 SharedImmutable / 4 SharedLocal" split is
an upper bound. Anyone continuing should re-run the census on a real corpus
before generating impls -- the diagnostic now prints exactly which types drop
out and why.

Remaining, unchanged: steps 2-4 (generate the forwarding impls for traits that
qualify, re-wrap user generic arguments and restore the handle-carrying erasure
pair, update the 11 stale expectations and record the baseline).

### Step 2 landed: the forwarding impls exist

`shared_handle_forwarding_impl` (`src/representation_lowering.rs`) generates
`impl<__JavaShared: Tr + 'static> Tr for std::rc::Rc<__JavaShared>` beside every
generated trait, forwarding each method as `(**self).m(args)`. Wired into all
**three** trait emission paths -- `trait_item_for_type`,
`trait_item_for_type_with_program`, and `source_class_bound_trait_item_for_type`.
Gate-clean: `idiom_gaps` unchanged at 538/146, one exact-output expectation
updated, `--lib` 169 -> 174.

Three things this got wrong first, each now pinned by a test:

- **`T` collides.** A Java interface method may declare its own type parameter
  (`<T> T id(T value)`), and an inner generic shadowing the impl's is E0403.
  Hence `__JavaShared`.
- **An empty trait still needs the impl.** A class used as a bare bound
  (`<T extends HeaderLine>` where `HeaderLine` declares no methods) generates an
  empty trait; returning `None` for "no methods to forward" left
  `Rc<FormatLine>: HeaderLineBound` unsatisfied. An empty trait is *trivially*
  forwardable.
- **Class bounds are a third emission path.** Wiring only the two interface
  paths missed `HeaderLineBound` entirely, which is the trait a
  `<T extends SomeClass>` bound actually lowers to.

The forwardability precondition from step 1 was also extended to walk the
`extends` chain, since a class-bound trait is built from the whole hierarchy's
methods, and `TypeStorageFacts` gained `surface_forwardable` so a non-forwardable
type falls back through the existing `is_implemented()` path rather than
producing code that does not compile.

### Where step 3 stands

Re-applying the wrap on top of steps 1-2 gives **523/161, 15 regressions**, down
from 16 before the empty-trait fix. Reverted, because it is not green. The
remaining set is the familiar one: ~11 stale expectations plus a handful of
conversion sites, and it is now unblocked by the trait machinery rather than by
the fork, which is closed.

### Step 2 completed: both handles

The forwarding impl is emitted for `Rc` **and** `Arc`. The tier picks `Arc` for
anything reachable from a Java `static` (which must be `Sync`) and `Rc`
otherwise, so a bound like `Standards<T extends Named>` has to be satisfiable by
either; emitting only the `Rc` form left `Arc<NamedLine>: Named` unsatisfied.

### Step 3: 525/159, two conversion sites left

With steps 1-2 in place the wrap now costs **13** rather than the 16 it cost
before the trait machinery existed: **11 stale expectations and 2 compile
breaks**. The progression, each step fixing a distinct site:

| after | regressions |
|---|---:|
| wrap re-applied on steps 1-2 | 15 |
| empty-trait forwarding impl | 15 -> 14 |
| optional element handle on the plan | 14 |
| `Arc` forwarding impl | 14 -> 13 |
| rewrap on clone-recovery from a borrow | 13 |

The two survivors are the same shape, and it is the one this file has been
circling from the start:

```rust
let repaired: HeaderLine = self.formats.as_ref().unwrap()
    .repair(line.clone()/* TODO */)   // expected `Rc<FormatLine>`, found `FormatLine`
```

`line` is a **borrowed parameter** of a shared type. A borrowed slot deliberately
sees *through* the handle, so cloning out of one produces a bare value while the
callee's slot still wants the handle. `rewrap_clone_recovered_shared_value`
handles this for the argument path it was added to; these two reach the call
through a different one (a generic *receiver*), so the rewrap never runs.

That is the whole remaining shape of the work: **every place a freshly produced
value meets a handle-typed slot needs the same rewrap, and they must be found
one at a time** because `RustSlotShape` cannot say whether a value is already
behind a handle. Four such sites are now handled (slot default, static field
initializer, folded static read, optional element, clone-recovery-from-borrow);
these two are not.

Reverted step 3 again; steps 1 and 2 are in the tree and green.

### Why step 3 keeps not landing, stated once and for all

Every attempt this session ended the same way, and the count kept falling
without reaching zero: 16 -> 15 -> 14 -> 13. Each fix was correct and each
revealed another site. The sites handled so far all share one description --
*a freshly produced value meets a handle-typed slot* -- and they are:

| site | where the fix lives |
|---|---|
| slot default | `default_expr_for_slot_with_effective_facts` |
| static field initializer | `static_storage_accessors_for_field` |
| folded static read in return position | `lower_return_stmt_with_refinements` |
| optional element | `OptionalSlotConstructionPlan::element_handle` |
| clone recovery from a borrow (argument path) | `rewrap_clone_recovered_shared_value` |

A central attempt was made and is worth recording as a negative: putting
`shared_handle_recovery` on `ShapeCoercionPlan`, so `coercions()` re-wraps after
*any* borrow-to-owned recovery, does **not** catch the two survivors. They reach
the callee through a generic *receiver* path that does not build a
`ShapeCoercionPlan` for the argument at all. So there is no single choke point;
the recovery paths are genuinely plural.

The two survivors:

```rust
let repaired: HeaderLine = self.formats.as_ref().unwrap().repair(line.clone());
//                                                               ^ bare FormatLine, slot wants Rc<FormatLine>
```

`line` is a **borrowed parameter**. A borrowed slot sees through the handle by
design -- that rule is what keeps `&T` from becoming `&Rc<T>` everywhere -- so
cloning out of one necessarily yields a bare value. The wrap must then be
re-applied wherever that clone is consumed, and "wherever" is the open set.

This is the same root the whole file keeps returning to: `RustSlotShape` records
a type and an ownership, not whether a value is behind a handle, so the question
"does this expression need the handle put back?" cannot be answered once. It has
to be answered at each consumer, and the consumers must be enumerated by running
the suite.

That is tractable -- the list is finite and shrinking -- but it is site-by-site
work, not another structural move. Anyone continuing should re-apply the stack
(`<scratchpad>/apply_0002b.py` plus the slot_render additions), read the failing
list, and add one rewrap per site until it is empty, then update the ~10 stale
expectations.

### Step 3, final measured state: one compile break left

Continuing site-by-site got the wrap from 16 regressions to **13 -- twelve stale
expectations and a single compile break**. The two sites added after the earlier
write-up:

- **Shaped-argument recovery** (`lower_args_with_plan`). The rewrap must be keyed
  on the **actual** type, not the target's. A generic method parameter is still
  `T` at the call site -- it is *not* substituted with the receiver's type
  argument -- so `storage_placement_for` on the target finds nothing. Keying on
  the actual is also correct rather than merely expedient: a concrete target of a
  shared type is wrapped, and a type-parameter target is instantiated with the
  wrapped form because generic arguments are wrapped. This closed
  `source_receiver_generic_result_upcasts_to_supertype_assignment_slot`.
- The earlier argument-path rewrap in `lower_source_subtype_argument*` remains,
  for the paths that do not reach `lower_args_with_plan`.

### The one that is left, and what it needs

```rust
line = self.formats.as_ref().unwrap()
    .repair(match line { HeaderLineValue::FormatLine(value) => value, _ => panic!(..) });
//                                                            ^ bare FormatLine, slot wants Rc<FormatLine>
```

`carrier_enum_item` (`src/representation_lowering.rs`) builds every closed-enum
and hierarchy-carrier variant as `Variant(Variant)` -- a **bare** payload. So
extracting from the carrier yields a bare value no matter what the consumer
wants, and no amount of wrapping at consumers fixes it in general.

The principled fix is to wrap the *payload type*, so the carrier holds
`FormatLine(Rc<FormatLine>)` and extraction yields the handle directly. That is
symmetric with what was already done for collection elements and generic
arguments, and it removes the site rather than patching it. It needs a `FactSet`
threaded into `carrier_enum_item`, which today takes only names, and matching
changes at the variant construction site (`coercion.rs`, `{enum}::{variant}(..)`)
so construction and extraction stay in agreement.

That is the last structural item. After it, what remains is updating twelve
expectations that assert 0002 symptoms, and recording the baseline.

### Step 3, best measured state: 12 assert-only, zero compile breaks in `idiom_gaps`

Continuing site-by-site reached **526/158 with every generated program in
`idiom_gaps` compiling**. Two more sites closed after the previous write-up:

- **Carrier payload type** (`carrier_enum_item`). A variant payload is a slot of
  its type like any other, so a shared type is stored behind its handle there
  too. Storing it bare made `match line { HeaderLineValue::FormatLine(v) => v }`
  hand back a bare value regardless of the consumer. Needs a `FactSet` threaded
  into `carrier_enum_item` and `source_hierarchy_carrier_item_for_type`.
- **Carrier construction** (`{enum_name}::{sub_name}(value)` in
  `source_supertype_conversion_impl_for_pair`, and
  `Coercion::WrapClosedEnum`). Construction and extraction must agree, so the
  `From<Sub> for Carrier` impl wraps the payload, and the coercion carries a
  `payload_handle` set only when the value arrives from a **borrowed** slot --
  an owned slot of a shared type already holds the handle.

### Why it still is not landable, and the honest limit

`slot_shape_design` then went 516 -> 514, and one of those is not an expectation
drift:

```rust
query: Option<std::rc::Rc<Cursor>>,                      // field: wrapped
let tmp: Option<Cursor> = self.query.as_ref().cloned();  // local: not
```

`slot_shape_design` does **not** run `cargo_check_generated_modules`, so this
reads as an assertion failure while being a type error. That is 0018's blind
spot again, and it means "zero compile breaks" was only ever true of the suite
that checks.

The count across this session went 16 -> 15 -> 14 -> 13 -> 12(+2 elsewhere).
Every step was a real fix; every step revealed another slot-rendering path that
does not go through the wrap. That is the honest characterisation: the wrap has
to be applied at *every* path that renders a slot type or produces a value into
one, and those paths are not enumerable by reading -- only by running the suites
and reading what breaks.

**Before the next attempt, do this first:** add `cargo_check_generated_modules`
to `slot_shape_design`'s translation tests, or at least to the ones that assert
on rendered types. Without it the suite cannot tell a stale expectation from a
type error, and the remaining work is exactly the job of telling those apart.

### The prerequisite is now landed

`cargo_check_generated_modules` lived only inside `tests/idiom_gaps.rs`, which is
why `slot_shape_design` -- 596 tests, many asserting on rendered types -- had
**zero** generated-crate checks (296 in `idiom_gaps`, 0 here). That is why the
`Option<Rc<Cursor>>` field against an `Option<Cursor>` local read as an assertion
failure rather than the type error it is.

The helper now lives in `tests/common/mod.rs` and is used by both suites, and
`optional_self_field_read_into_owned_local_borrows_then_clones_element` -- the
test that hid it -- checks its generated crate. Landed while green, so the guard
is in place *before* the next attempt at the wrap rather than after.

Extending the check to the rest of `slot_shape_design`'s translation tests is the
obvious follow-up and should be done incrementally: each one that goes red is a
pre-existing non-compiling output that nothing was measuring.

### A bug in the wrap itself, found by the new guard

The `cargo_check` added to
`optional_self_field_read_into_owned_local_borrows_then_clones_element` paid for
itself immediately. With the wrap on:

```rust
query: Option<std::rc::Rc<Cursor>>,   // field
let tmp: Option<Cursor> = ...;        // local -- disagrees
```

The cause was in the wrap, not the lowering. There are two slot-type renderers.
The `EffectiveFacts` one recurses through `Option` by re-entering *itself*, so
the element gets the handle. The representations-aware one had been changed to
recurse into the **unwrapped** inner function, so its optional elements never
did. Fixed by recursing through the wrapping entry point in both, and by having
`shared_handle_slot_rust_type` decline optional shapes -- the element is where
the handle belongs, and wrapping the outer as well gives `Rc<Option<T>>`.

Worth stating plainly: this defect was invisible to 596 `slot_shape_design`
tests and would have stayed invisible, because none of them compiled their
output. It is the first thing the new guard caught, and it argues for extending
`cargo_check` to the rest of that suite before the next attempt.

### A note for whoever extends the `cargo_check` coverage

Do not bulk-edit `tests/slot_shape_design.rs` by brace-matching to find test
bodies. The fixtures are Java in `r#"..."#` literals and are full of `{` and `}`,
so a depth counter walks straight past the end of a function. An attempt here
"found" one test out of ~155 and silently mangled its own scan. Match on the
literal `);\n}` that closes each test's final assertion, or do it by hand in
batches.

## Update 2026-08-06 — step 3 is landed

The shared handle is on. At that landing checkpoint, `scripts/gate.sh` was at
baseline on all 18 targets and `idiom_gaps` was 538/146 with **identical failure
names** -- zero new failures, zero silently-fixed ones. Later audit work moved
the checked `idiom_gaps` baseline to 816/0.

```rust
query: Option<std::rc::Rc<Cursor>>,                              // field
let tmp: Option<std::rc::Rc<Cursor>> = self.query.as_ref().cloned();  // local
static CONFIG_INSTANCE: std::sync::Mutex<Option<std::sync::Arc<Config>>>;
pub fn get_instance() -> std::sync::Arc<Config>;                 // Arc: static-reachable
```

What made it land, after eight reverts, was the generated-crate checking added in
[0041](done/0041-slot-shape-design-had-no-generated-crate-checks.md). At that
landing checkpoint, 101 `slot_shape_design` tests and 323 `idiom_gaps` tests
compiled their output; each pass named a *type error* instead of only assertion
text, and the list ran dry.

### The wrap, in one place

**Type side** -- two positions, and only two:

| position | rule |
|---|---|
| top level of a slot | ownership-aware: owned only, since a borrow sees through the handle |
| generic argument | unconditional: an argument is always owned by its container |

Both recurse through `Option` at the **element**, giving `Option<Rc<T>>`; the
outer wrap declines optional shapes so it cannot also produce `Rc<Option<T>>`.

**Value side** -- the handle goes on wherever a *fresh* value meets a handled
slot. "Fresh" is never guessed; each site has a structural reason:

| site | why the value is known to be bare |
|---|---|
| slot default | nothing has run yet |
| static field initializer | a field initializer is a construction |
| return of a folded static read | restricted to a bare `T::new(..)` |
| local declaration initializer | same restriction |
| optional element (`OptionalSlotConstructionPlan::element_handle`) | decided *with* the `Some`, so the two cannot disagree |
| argument recovered out of a borrow | a borrow saw through the handle |
| subtype upcast (`.into()`) | an upcast returns a fresh supertype value |
| carrier variant payload | construction and extraction share one placement |

### Four defects the landing exposed, all fixed

- **`Rc<T>` nested in a collection cost its struct the `Clone` derive.** The
  top-level arm of `rust_type_supports_struct_derive_clone` knew a handle is
  `Clone`; the element arm did not.
- **A type parameter sharing a name with a source class got wrapped**
  (`input: Rc<ELEM>`). `JavaType` has no type-variable variant and slot rendering
  has no type-parameter scope, so the plan now excludes any name bound as a type
  parameter anywhere -- conservative in the safe direction.
- **The two slot-type renderers disagreed on `Option` recursion**, so the
  representations-aware one never wrapped optional elements.
- **The return-slot wrap fired on values that were already handles**, giving
  `Arc<Arc<T>>`; restricted to a bare constructor call.

### Codex follow-up 2026-08-12 — constructor freshness guard asks structured calls first

The value-side guard that decides whether a value is fresh enough to wrap for a
handled slot still rendered the expression and parsed `Owner::new(..)` from the
result. That kept the guard dependent on value-position text even when lowering
had a structured `RustExpr::Call`.

`rust_expr_is_bare_constructor_call` now recognises structured call function
paths directly and keeps the old rendered parser only for raw expressions. This
does not broaden the fresh-value rule; it just makes the existing bare
constructor precondition structural when the IR already carries the call shape.

The shared path parser also handles generated turbofish constructor calls and
spacing before the call parenthesis, so `T::new_1::<U>(...)` is still recognised
as the same fresh bare constructor as `T::new_1(...)`.

### Codex follow-up 2026-08-12 — shared handle construction preserves call structure

`slot_render::shared_handle_construction_expr` still wrapped fresh defaults and
fresh local initializers by rendering the inner value and returning one raw
`Rc::new(...)` or `Arc::new(...)` string.

For the implemented `SharedImmutable` tier, the helper now returns a structured
`RustExpr::Call` with the original expression as its argument. The later
`wrap_construction` string fallback has since been deleted; value construction
now goes through `slot_render::storage_placement_construction_expr`.

### Codex follow-up 2026-08-12 — optional element handle construction reuses the structured wrapper

The optional-construction lowering path still had its own
`placement.wrap_construction(emit_expr(value))` calls for fresh elements meeting
handled optional slots. Those are the same value-side handle wrap as defaults and
fresh local initializers, but they flattened `Some(Rc::new(value))` through a raw
string before the `Some` plan saw it.

Both optional-construction entry points now use the shared structured placement
helper, so implemented `SharedImmutable` elements keep the nested
`Some(Rc::new(value))` call shape. That structured helper is now the only
production construction route.

### Codex follow-up 2026-08-12 — subtype/erasure fresh-constructor wraps use the shared helper

The source-subtype upcast and erased-object slot paths still duplicated the
fresh-constructor handle wrap with direct
`placement.wrap_construction(emit_expr(...))` calls. Those branches are the same
value-side SharedImmutable construction rule as defaults, locals, and optional
elements.

The fresh-constructor arms now delegate to the shared placement helper too. The
outer erased-object boxing remains raw because it still composes a cast
expression, but the handle construction inside it uses the centralized
implemented-placement path.

### Codex follow-up 2026-08-12 — recovered shared arguments use the shared wrapper

`rewrap_recovered_shared_argument` was the remaining argument-path rewrap named
above: after lowering recovered a bare value from a shared borrow, the callee's
owned generic slot still required the handle. That helper still rendered the
recovered value, checked the rendered wrapper prefix, and returned a raw
`placement.wrap_construction(...)` string.

It now delegates to the same placement helper as defaults, optional elements,
and subtype/erasure fresh-constructor wraps. The existing idiom-gaps fixture
`source_receiver_generic_result_upcasts_to_supertype_assignment_slot` continues
to pin the emitted `Rc::new(line.clone(...))` shape.

### Codex follow-up 2026-08-12 — borrowed subtype erasure uses the shared wrapper

The borrowed/mutable-borrow erased-object subtype branch still cloned the value,
rendered it, and passed the string through `placement.wrap_construction(...)`
before the outer `Box<dyn Any>` erasure. That is the same implemented-placement
construction rule as the owned fresh-constructor branch.

It now delegates to the centralized placement helper after building the cloned
payload expression. The outer erasure remains raw because it still composes the
`Box::new(...) as ...` cast, but the handle construction inside it stays on the
same structured path as other SharedImmutable value wraps.

### Codex follow-up 2026-08-12 — enum carrier conversions use the shared wrapper

Source-hierarchy enum conversions still had one generated-code duplicate of the
implemented-placement rule: `impl From<Sub> for SuperEnum` built the enum payload
with `placement.wrap_construction("value")` when the subtype was stored behind a
shared handle.

That conversion now renders the centralized placement helper applied to the
`value` expression. The surrounding `impl From` body is still raw generated Rust,
but the SharedImmutable construction spelling now has one owner.

### Codex follow-up 2026-08-12 — same-outer constructor storage uses the shared wrapper

The captured-inner constructor-body rewrite had the last non-central value-side
call to `placement.wrap_construction(...)`: after rewriting a same-outer sibling
constructor to pass `__outer.clone()`, it rendered the full constructor
expression and wrapped the string for storage.

It now passes the rewritten expression to the centralized placement helper. The
helper keeps implemented SharedImmutable handle construction structured and
retains the raw fallback for future cell placements.

### What this does *not* do

`SharedImmutable` only. `is_implemented()` still excludes any type whose mutation
cannot be proved absent, and now also any type whose trait surface is not
`&self`-forwardable. `SharedLocal`/`SharedSync` -- the `RefCell`/`Mutex` rungs --
remain off, and with them the reentrancy discipline this file describes.

## Update 2026-08-11 (Codex follow-up) — stale gap diagnostics removed for implemented identity

Two diagnostics were still describing landed `SharedImmutable` cases as if they
were open gaps.

`storage_strategy_disagreement` now fires only for shared placements that are not
implemented. An `Rc`/`Arc` placement is no longer "lowered by value"; the
surface-forwardability warning remains separate and still reports the cases that
block handle forwarding.

The reference-identity diagnostic pass now receives the same `FactSet` as
lowering. For source types whose storage placement is shared and implemented,
`==`, `!=`, and inherited `Object.equals` no longer report the old structural
equality warning; those are the cases 0028 lowers with `Rc::ptr_eq`/`Arc::ptr_eq`.
String identity and unimplemented reference-identity cases still warn. The
regression is `implemented_shared_identity_equality_has_no_gap_diagnostics`,
using `tests/differential/Identity.java`.

## Update 2026-08-29 (Codex follow-up) — nullable dyn-trait returns stay modeled

`nullable_external_dyn_trait_return_constraints_keep_trait_owned_carrier` now
pins the current modeled-external contract: a nullable return from a dyn-trait
external pack keeps owned/nullability constraints and trait dispatch
constraints, rather than collapsing back to erased-object dispatch. The old test
name and assertions described the pre-shared-representation carrier choice.

## Update 2026-08-29 (Codex follow-up) — erased external handle fixture matches nullable storage

`erased_object_external_handle_lowers_store_pass_and_return_through_same_carrier`
now pins the current carrier flow: the erased field is nullable storage,
`store(Dog)` receives the erased `Box<dyn Any>` carrier directly, and passing the
field to another erased argument null-checks and clone-recovers that same
carrier. The old assertions expected boxing at the assignment site and a
non-nullable getter shape.

## Update 2026-09-07 (Codex follow-up) — owned source-call signature row is baseline-clean

`emitted_source_call_uses_authoritative_owned_param_signature` now matches the
landed `SharedImmutable` source-class spelling. The assertion was stale on two
axes: nested source class names now preserve the owner boundary with an
underscore, and fallible string-concat lowering wraps the source call in
`Result<String, JavaException>`. The generated call still passes the solved
owned `Rc<Alignment>` parameter without borrowing, and the row was removed from
the idiom-gaps failure baseline after a focused generated-crate check.

## Update 2026-09-09 (Codex follow-up) — construction strings are no longer a storage API

`StoragePlacement::wrap_construction` has been deleted. Production value wrapping
already went through `slot_render::storage_placement_construction_expr`, which
builds structured `RustExpr::Call` nodes and recovers parseable raw leaves only
at the compatibility boundary. The placement law test now renders that
structured helper to keep the type/value nesting invariant without preserving a
public raw construction-string API.

## Update 2026-09-09 (Codex follow-up) — `SharedLocal` is blocked at the receiver boundary

**Superseded by the 2026-09-17 verification above.** This records the pre-rollout
state; `SharedLocal` is now enabled for the guarded single-threaded receiver
paths. `SharedSync` remains the unimplemented tier.

The type and value renderers already know how to spell the mutable shared
placements structurally: `storage_placement_type` builds
`Rc<RefCell<T>>`/`Arc<Mutex<T>>`, and
`storage_placement_construction_expr` builds the matching nested constructor
calls. The remaining switch is `StoragePlacement::is_implemented`, which still
returns false for `interior_mutable`.

That gate should not be flipped by itself. Field reads/writes, method receivers,
identity comparisons, erased conversions, optional slots, and captured inner
construction all need a coordinated `borrow()`/`borrow_mut()` policy with the
documented "do not hold a borrow across a call" rule. The narrow boundary tests
that currently pin this are:
`inner_class_capture::tests::outer_this_construction_needs_an_implemented_mutable_shared_placement`
and
`storage_strategy::placement_law_tests::the_type_and_construction_forms_nest_the_same_way`.

Codex follow-up 2026-09-14: one receiver-order hole in that policy is now fixed
for already-enabled `SharedLocal` contexts. Optional `Rc<RefCell<T>>` source
field receivers are projected out of `Option` before the `borrow()`/
`borrow_mut()` receiver adapter runs, so calls emit
`self.field.as_mut().unwrap().borrow_mut().m(...)` instead of invalid
`self.field.borrow_mut().m(...)`. Guards:
`idiom_gaps::optional_field_mutating_overload_call_uses_signature_name_and_mut_receiver`
and
`idiom_gaps::context_translation_uses_sibling_overload_signature_for_call_target`.

## Update 2026-09-10 (Codex follow-up) — the implementation gate is pinned directly

`storage_strategy::storage_placement_implementation_gate_stays_explicit` now
checks the four active placement rungs without going through a Java fixture:
inline values, immutable shared `Rc`/`Arc`, and single-threaded
`Rc<RefCell<T>>` placements are implemented, while `SharedSync` remains off.
The test is now a tripwire against accidentally enabling `Arc<Mutex<T>>` by
weakening `StoragePlacement::is_implemented`; the covered `SharedLocal`
receiver and field-access policy is exercised by the promoted differential
fixtures above.

## Update 2026-09-10 (Codex follow-up) — inner capture is pinned to `SharedLocal` end-to-end

**Superseded by the promoted fixtures.** `InnerCapture` and
`QualifiedInnerCapture` now execute through the implemented SharedLocal outer
handle companions; this historical output-marker expectation describes the
former diagnostic-only state.

`output_markers::inner_capture_gap_reports_mutable_shared_storage_work_queue`
now translates `tests/differential/known-gaps/InnerCapture.java` and requires the
same output to contain the inner-construction runtime handoff, the
`inner_class_construction_panics` diagnostic, and a
`storage_strategy_surface_not_forwardable` message naming `InnerCapture` as a
shared type whose interface surface cannot yet be handle-forwarded. That keeps
0023's ordinary inner-constructor failure attached to this audit's remaining
shared receiver/borrow-policy gate.

## Update 2026-09-10 (Codex follow-up) — mutable aliasing is pinned to `SharedLocal` too

Historical note: `known_gap_fixtures_are_valid_java_and_match_expected_failure_mode`
used to require `tests/differential/known-gaps/Aliasing.java` to report
`storage_strategy_disagreement` messages for `Aliasing.Node needs SharedLocal`
and `Aliasing.Counter needs SharedLocal`, in addition to the error-level
`observable_alias` diagnostic. That fixture has since been promoted to
`tests/differential/Aliasing.java` after the direct local/storage alias,
callback-parameter, stored-action, and narrow zero-argument anonymous-action
SharedLocal slices landed. Keep the note as the provenance for why mutable
aliasing belongs to this audit; do not treat it as current known-gap status.

## Update 2026-09-12 (Codex follow-up) — placement axes are private-backed

`StoragePlacement` still carries the same three monotone axes (`shared`,
`interior_mutable`, `sync`) and the same implementation gate, but callers now
construct it through `StoragePlacement::new(...)` and read the axes through
accessors. That keeps storage placement as a named decision boundary instead of
a public field bag that lowering, rendering, and tests can mutate or pattern
match directly. Guard:
`layering::declaration_context_usage_lookups_use_java_callable_keys`; law guards
remain in `storage_strategy::placement_law_tests`.

The same API fence now applies to `TypeStorageFacts`: the storage planner still
owns mutation of escape, aliasing, mutation, threading, forwardability, and
static-reachability evidence while it collects facts, but cross-module fixtures
and callers build/read those axes through named setters and accessors. This
keeps the evidence record inspectable without making its raw tuple layout public
API.

## Update 2026-09-15 (Codex follow-up) — zero-argument anonymous returns no longer fall through

Anonymous implementations of single-method, zero-argument interfaces now lower
through a block-local adapter around the closure body instead of falling back to
`<dyn Trait>::anonymous()` whenever the method returns a value. The adapter
handles local source traits with object-support methods and external facade
traits whose emitted surface only requires the functional method plus
`Display`. Promoted behavioral guard:
`differential::anonymous_functional_interface_fixtures_match_java_output`
(`tests/differential/AnonymousSupplier.java`); regression guard:
`fixtures::anonymous_interface_return_uses_boxed_dyn_trait_slot`.

## Update 2026-09-18 (Codex follow-up) — functional anonymous adapters carry arguments

The same closure adapter now supports a single abstract interface method with
parameters and a return value. Its closure bindings receive the trait method's
slot shapes, and the generated adapter implements the corresponding method
signature rather than falling through to `<dyn Trait>::anonymous()`. The
`AnonymousInterface.IntPredicate` fixture verifies both positive and negative
calls through the returned trait object. This remains a functional-interface
slice, not general anonymous object lowering: anonymous bodies that require
their own `this`/`super` identity remain loud unsupported cases.

Codex follow-up 2026-09-17 closes the specific mutable-capture counterexample:
storage analysis now descends into anonymous method bodies, so writes through a
captured outer handle select `Rc<RefCell<T>>` before closure lowering. The
context `Action` fixture executes and observes `"23"` through its alias. This
does not claim general anonymous-object support: interfaces with parameters,
anonymous bodies using `this`/`super`, and more complex receiver adaptation
remain outside the closure-adapter boundary.

## Update 2026-09-18 — interface-upcast handoff is closed for owned arguments

The generated source trait surface includes an identity-preserving carrier and
`From<Box<dyn Derived>> for Box<dyn Parent>` implementation for each Java
interface parent. At a receiverless/static-call argument, lowering now invokes
that `Into` implementation with both box types explicit, then projects the
result to `&dyn Parent`. The Java oracle
[`repro/InterfaceUpcast.java`](repro/InterfaceUpcast.java) and the generated
fixture pass. Class-anonymous carriers and class `super` remain separate open
work.

## Update 2026-09-19 — bounded class-anonymous enhanced-for lowering

The class-anonymous adapter now accepts an enhanced `for` over a primitive
array method parameter, including receiver-state writes and labeled
`break`/`continue`. Its dispatch and carrier methods use a borrowed slice for
that array parameter, matching the ordinary source-method ABI instead of
passing a `Vec` across the virtual boundary. The Java oracle
[`repro/AnonymousClassEnhancedFor.java`](repro/AnonymousClassEnhancedFor.java)
prints `13`; `fixtures::anonymous_source_class_enhanced_for_rebinds_array_parameter_and_state`
executes the translated result with the same value. This is a bounded path:
enhanced-for over other collection or array expressions, mutable array views,
and the remaining anonymous-object cases are not established by this fixture.

## Update 2026-09-19 — virtual array ABI follows solved method contracts

A class-anonymous carrier now renders primitive-array parameter types from the
source method's solved signature and carries its exception result type through
the dispatch trait, carrier, and non-overridden adapter forwarder. This avoids
advertising `&[i32] -> i32` when the base method actually requires
`&mut [i32] -> Result<i32, JavaException>`. The exact resolved source call
also marks a caller array local mutable even when semantic usage classifies the
boundary as `KnownContentRead`; that classification does not cancel a `&mut`
callee ABI. [`repro/AnonymousClassArrayMutation.java`](repro/AnonymousClassArrayMutation.java)
prints `10`, matched by
`fixtures::anonymous_source_class_inherited_method_mutates_array_argument`.
The signature-level regression is
`signature::tests::mutable_array_argument_marks_caller_local_binding_mutable`.
This establishes mutable-array forwarding through an inherited base method,
not general enhanced-for over mutable arrays or all fallible overrides.

## Update 2026-09-19 — anonymous override honors a fallible base ABI

The class-anonymous override renderer now receives the base method's solved
parameter shapes and exception result contract. For supported ordinary
statements it returns `Ok(...)` at explicit value returns and lowers checked
primitive-array reads in propagating mode. Two Java oracles pin both sides:
[`repro/AnonymousClassFallibleOverride.java`](repro/AnonymousClassFallibleOverride.java)
prints `0` when an override replaces a throwing base array access with a
non-throwing `length` read;
[`repro/AnonymousClassFallibleBody.java`](repro/AnonymousClassFallibleBody.java)
prints `7` and `caught` when the override itself selects `values[1]` and is
called with a valid then empty array. The translated fixtures
`anonymous_source_class_override_preserves_fallible_base_contract` and
`anonymous_source_class_override_propagates_its_own_array_failure` pass. This
does not establish other `super` targets, all throwing expressions, or every
special-case anonymous method renderer under a fallible contract.

## Update 2026-09-19 — fallible same-method class-super forwarding

An anonymous override can now return its base method's fallible result
directly, or bind it to a method local with `?` before computing a further
value. The latter used to assign `Result<i32, JavaException>` to an `i32`
local. Because anonymous `super` resolution does not always supply the
ordinary callable-analysis result bit, the adapter also recognizes a
same-method `super` call under its solved fallible contract. The Java oracle
[`repro/AnonymousClassFallibleSuper.java`](repro/AnonymousClassFallibleSuper.java)
prints `7`, `8`, `caught`, `caught`; the translated fixture
`anonymous_source_class_fallible_super_return_preserves_result` verifies both
values and both error paths. At this checkpoint, calls to a different `super`
method and more complex control flow remained outside the verified slice.

## Update 2026-09-19 — class-super calls use the selected target

The class-anonymous adapter no longer substitutes the override's own method
name for `super.other(...)`. It uses the resolved Rust target when available;
otherwise it selects a source declaration by exact Java name and argument
types, rejecting ambiguous matches. That avoids treating `checked` and
`checkedViaLocal` as one Rust prefix family, and selects `base(int)` when a
`base(String)` overload is present. The same selected declaration supplies
fallibility for a local initialized from `super.checked(values)`, so an array
error propagates before the local is bound. The Java oracle
[`repro/AnonymousClassOtherSuperTarget.java`](repro/AnonymousClassOtherSuperTarget.java)
printed `21`, `8`, `caught` at this checkpoint; the translated fixture
`anonymous_source_class_super_call_uses_the_resolved_other_method` matches.
This does not establish every argument coercion or nested expression form for
class-`super` calls.

## Update 2026-09-19 — nested class-super arithmetic preserves target and errors

The class-anonymous return-expression renderer now selects the actual
`super.base(value + 1)` target when the call is an arithmetic operand, rather
than substituting the current override's base method. It handles either
operand position, and applies `?` to a fallible `super.checked(values)` call
before `wrapping_add`, so empty-array failures propagate through the fallible
override contract. Numeric literals used as the receiver of `wrapping_add`
retain their Java `int`/`long` type. The expanded Java oracle
[`repro/AnonymousClassOtherSuperTarget.java`](repro/AnonymousClassOtherSuperTarget.java)
printed `21`, `9`, `9`, `8`, `8`, `8`, then three `caught` lines at this
checkpoint. The translated
fixture `anonymous_source_class_super_call_uses_the_resolved_other_method`
matches these values and error paths. The 1,092 library tests and 87
anonymous fixtures passed. A separate fallibility-plan gap remained: an
otherwise-infallible base method is not yet promoted to a `Result` contract
solely because its anonymous override calls a fallible different `super`
method. At this checkpoint, the fixture gave its `checkedExpr` contracts
their own checked array access.

## Update 2026-09-19 — anonymous override fallibility reaches the virtual slot

The call graph now records an explicit relationship between an anonymous
method and the exact inherited or direct base method contract it overrides.
Fallibility equalizes across that relationship; constructing the anonymous
object does not itself call the override and therefore does not make the
factory fallible. Within an anonymous method, a class-`super` call can target
the selected source method even when a same-named overload exists: the
resolved Rust name is used when available, otherwise known Java argument
types disambiguate the candidate. The expanded
[`repro/AnonymousClassOtherSuperTarget.java`](repro/AnonymousClassOtherSuperTarget.java)
prints `21`, `9`, `9`, four `8` lines, then four `caught` lines. The translated
fixture verifies the same values and errors, including `checkedByOverride`,
whose base declaration returns `-4` and is fallible only because its anonymous
override calls `super.checked(values)`. The unit test
`exception_plan::tests::fallible_anonymous_override_promotes_base_contract_not_factory`
checks the contract, caller, factory, and nonthrowing `String` overload
separately. At this checkpoint, all 1,093 library tests and 87 anonymous
fixtures passed. Direct throwing expressions in an otherwise-infallible
anonymous override were not yet separately seeded into the fallibility plan.
The broader verification
remains red in this worktree: `cargo test --test layering -- --quiet` has seven
source-structure failures, and `cargo test --test fixtures -- --quiet` has
20 failures, including non-anonymous simple-class and stream cases. These
results are not evidence of overall audit closure.

## Update 2026-09-19 — direct anonymous-body throws join the contract plan

The call graph now retains anonymous method declarations under their callable
keys. The existing fallibility seed checks can therefore inspect anonymous
bodies for integer division, `Math.toIntExact`, nullable-receiver dereference,
and explicit `throw`, then equalize the result with the selected base virtual
contract. The class-anonymous statement renderer also supports an explicit
throw of a newly constructed exception with literal arguments, including a
guarded throw followed by a literal return. Java oracles
[`repro/AnonymousClassDirectArrayContract.java`](repro/AnonymousClassDirectArrayContract.java)
and [`repro/AnonymousClassDirectThrowContract.java`](repro/AnonymousClassDirectThrowContract.java)
each print a value followed by `caught`; the translated fixtures verify `5`
and `7` respectively and an error for each throwing path. The expanded
`exception_plan::tests::fallible_anonymous_override_promotes_base_contract_not_factory`
also checks direct array and explicit-throw promotion without making the
factory fallible. All 1,093 library tests and 89 anonymous fixtures pass.
The same seven layering tests remain red; the full fixture suite was not
rerun at this checkpoint. Other exception expression shapes and the broader
anonymous-object requirements remain open.
