# 0042 — "does this call mutate the collection?" is five hand-written lists, and they disagree

Status: **fixed** — the duplicated mutability/recognition decisions now route
through shared typed collection policy. Remaining table-driven collection
lowering dispatch would be cleanup, not the audited five-list correctness
defect.

Recorded 2026-08-21.

Codex follow-up 2026-08-21: validated. A holding fix now adds `removeAll` /
`remove_all` to `usage::is_mutating_collection_method_name`, and
`collection_remove_all_on_field_marks_self_mutable` compiles the generated Rust
for `this.names.removeAll(other)`. The structural issue remains open: the five
collection-mutator lists are still duplicated, and static argument mutators such
as `Collections.sort(this.list)` are still not represented by the same fact.

Codex follow-up 2026-08-22: static collection argument mutators now have one
shared predicate, `stdlib::static_collection_argument_mutates`, used by both
lowering's assigned-name collection and usage's `mutates_self` analysis.
`usage_treats_static_collection_argument_mutators_as_self_mutation` pins
`Collections.sort(this.values)`. Usage's receiver-mutator check now calls the
same typed `stdlib::native_collection_receiver_mutates` policy as signature,
and `usage_collection_receiver_mutation_is_type_aware` pins that a method name
from another collection kind does not mark `self` mutable. The audit remains
open because reader/native method recognition and stdlib role tables are still
separate policies.

Codex follow-up 2026-08-22, typed native-recognition slice: signature's native
field-recognition path now calls the shared typed
`stdlib::native_collection_receiver_method_known` query instead of a private
untyped method-name list. The traversal threads the field's `JavaType`, so
`ArrayDeque.push` marks a field as a native collection receiver while `Map.push`
does not. Regression tests pin both sides. The audit remains open only for the
stdlib external-role tables, which still carry separate collection/map method
lists instead of deriving `ExternalApiRole` from the same native collection
method table.

Codex follow-up 2026-08-22, external-role slice: the stdlib external API role
path now uses `native_collection_receiver_method_role_for_name`, which resolves
the receiver to a `JavaCollectionKind` and delegates to the same typed native
collection policy before choosing `ExternalApiRole`. The old
`collection_api_method_role` and `map_api_method_role` functions are deleted,
and `native_collection_external_api_roles_share_typed_receiver_policy` pins
`List.remove_all`, `Map.put_all`, `Map.contains_key`, and the negative
`Map.push` case. The five-list disagreement is fixed; the remaining cleanup is
the broader, lower-priority idea of making collection lowering dispatch itself
table-driven from the same metadata.

Codex follow-up 2026-08-22, closure audit: independently rechecked the current
tree. Usage self-mutability, signature local/binding mutability, signature
native-field recognition, external API role/provenance classification, and
static argument mutator analysis all delegate to the shared stdlib typed
collection policies. Lowering still has collection dispatch matches, but those
choose emitted operations after a `NativeCollectionMethod` has already been
planned; they are not the original silent mutability/recognition disagreement.
0042 is closed, with table-driven lowering dispatch left as optional cleanup.

Severity: **high** — live defect, `error[E0596]` on a five-line Java class
Kind: parallel lookup tables that must agree and are not derived from each other

## Symptom

```java
import java.util.ArrayList;
import java.util.List;

public class Mut2 {
    private List<String> names = new ArrayList<>();
    public void dropAll(List<String> other) { this.names.removeAll(other); }
}
```

translates to

```rust
pub struct Mut2 { names: Vec<String> }

impl Mut2 {
    pub fn drop_all(&self, other: Vec<String>) {
        { let __java_remove_all_values = other;
          self.names.retain(|item| !__java_remove_all_values.contains(item)); };
    }
}
```

`rustc --edition 2021 --crate-type lib`:

```
error[E0596]: cannot borrow `self.names` as mutable, as it is behind a `&` reference
```

The **lowering is right** — `removeAll` really is `retain(|x| !other.contains(x))`. The
**signature is wrong**: the method got `&self`. Two different parts of the compiler
answered the same question ("is `removeAll` a mutating collection method?") and gave
opposite answers.

Same shape for `Set`:

```java
public void mSetRemoveAll(Set<String> v) { this.set.removeAll(v); }
// -> pub fn m_set_remove_all(&self, ...) { ... self.set.retain(...) }  E0596
```

and for a field passed to a modelled static mutator:

```java
public void a8() { Collections.sort(this.list); }
// -> pub fn a8(&self) { self.list.sort(); }                            E0596
```

## Verified evidence

A single class with 13 mutator methods, one per Java collection mutator, run through
`./target/release/new-java2rust`. Only three came out `&self`:

| Java call on a `this` field | emitted receiver | emitted body |
|---|---|---|
| `list.add(v)` | `&mut self` | `self.list.push(v)` |
| `list.addAll(v)` | `&mut self` | `self.list.extend(..)` |
| `list.remove(v)` | `&mut self` | `self.list.remove(v)` |
| **`list.removeAll(v)`** | **`&self`** | `self.list.retain(..)` ← E0596 |
| `list.removeIf(..)` | `&mut self` | `self.list.retain(..)` |
| `list.clear()` | `&mut self` | `self.list.clear()` |
| `map.put(k,v)` | `&mut self` | `self.map.insert(k,v)` |
| `map.putAll(v)` | `&mut self` | `self.map.extend(v)` |
| `map.remove(k)` | `&mut self` | `self.map.remove(k)` |
| `map.clear()` | `&mut self` | `self.map.clear()` |
| `set.add(v)` | `&mut self` | `self.set.insert(v)` |
| **`set.removeAll(v)`** | **`&self`** | `self.set.retain(..)` ← E0596 |
| **`Collections.sort(this.list)`** | **`&self`** | `self.list.sort()` ← E0596 |

The same calls on a **local** are fine — `let mut names: Vec<String>` is emitted and the
program compiles. The gap is specific to `this`-fields, because the field path and the
local path read *different lists*.

## The five lists

| # | symbol | file:line | what it decides |
|---|---|---|---|
| A | `native_collection_receiver_mutates` | `src/stdlib.rs:2190` | `let mut` on a **local** collection (via `signature.rs:10599 native_collection_receiver_binding_constraints`) |
| B | `stdlib::native_collection_receiver_mutates` via usage | `src/usage.rs` | typed receiver query for `facts.mutates_self` → **`&mut self`** (via `expr_mutates_self_field`) |
| C | `native_collection_receiver_method_name` | `src/signature.rs:8289` | whether a field is a *native* collection at all (via `signature.rs:8042 field_is_native_collection_receiver`) |
| D | `native_collection_receiver_method_role_for_name` | `src/stdlib.rs:2221` | `ExternalApiRole` for List/Set/Deque/Stack |
| E | `native_collection_receiver_method_role_for_name` | `src/stdlib.rs:2221` | `ExternalApiRole` for Map |

Extracted membership, verbatim:

```
A (List|Set): add addAll add_all clear extend insert push remove removeAll removeIf
              remove_all remove_if retain
A (Map)     : clear extend insert put putAll put_all remove removeAll remove_all
B           : now delegates to A's typed receiver-mutator policy
C           : now delegates to `native_collection_receiver_method_known`, the
              same typed kind-indexed policy used by A/B, plus reader methods
              appropriate to the collection kind
D/E         : now delegate to the same typed kind-indexed policy used by A/B/C,
              including reader methods mapped to `ExternalApiRole::Getter`
```

Set differences:

```
A \ B  = fixed by delegating B to A's receiver-mutator policy
A \ C  = fixed by delegating C to the shared typed receiver-method policy
A \ (D ∪ E) = fixed by delegating D/E to the shared typed role policy
```

`A \ B` was the E0596. `A \ C` was the latent `Deque.push`/`Stack.push` field
case and is now fixed by making C a typed query over the same native collection
receiver policy. `A \ (D ∪ E)` was the role-table spelling problem: A/B/C carry
both the Java and the snake-case spelling of every name (`addAll` **and**
`add_all`), while the old D/E tables carried only the Java spelling. D/E now
derive roles from the typed native collection receiver policy too.

## Root cause

There is no representation of "this method mutates its collection receiver". There are
five `matches!` arms, each written next to the consumer that needed it, each maintained
by hand, each with its own idea of

- which *methods* count (B/C/D/E now delegate to A's typed receiver policy),
- which *spellings* count (the shared query accepts Java and Rust spellings),
- which *kind* of collection it applies to (A/B/C/D/E now share the typed
  kind-indexed answer),
- and whether the mutated thing is the receiver at all (`Collections.sort(x)` and
  `Arrays.sort(x)` now share `stdlib::static_collection_argument_mutates` between
  usage and lowering, but the broader `ExternalApiRole::Mutator` model still
  cannot express arbitrary argument mutation).

Meanwhile the *lowerings* are a sixth, disjoint set of hand-written names that nobody
cross-checks against the five: `lower_native_collection_remove_all_expr_stmt`
(`src/lower.rs:12776`, gated on `"removeAll" | "remove_all"`), the `addAll`/`putAll` arm
at `src/lower.rs:13001`, `removeIf` at `src/lower.rs:14376`, and
`CallStatementPlan::NativeCollectionsSort` at `src/call_plan.rs:2259`. Every one of those
is a *promise that the receiver will be `&mut`*, and none of them is what B reads.

This is 0006's shape ("five approximations of one relation") and 0010's shape
("fallibility is decided three times and disagrees") applied to receiver mutability.

## Why it was not noticed

`removeAll` **is** tested. `tests/idiom_gaps.rs:7616
collection_remove_all_statement_uses_retain_with_contains` even calls
`cargo_check_generated_modules`, so the generated crate is compiled. But its fixture is

```java
void remove(Set<String> lines, HashSet<String> toRemove) { lines.removeAll(toRemove); }
```

— the collection is a **parameter**, and parameter mutability is decided by A, the list
that *has* `removeAll`. The one list that lacks it is only reachable through a `this`
field, and no fixture in `tests/` calls `removeAll` on a field. `retainAll` appears in no
test at all.

So the test covers the lowering and the local/parameter mutability path, and the field
path — the common one in real Java, where collections are instance state — is untested
for every method in `A \ B`.

## Proposed direction

Do not add `removeAll` to `usage.rs`. That re-syncs the lists and leaves the next
`retainAll`, `List.set`, `Map.merge` or `Deque.pop` to drift the same way.

The invariant that must hold is: **every lowering that emits a Rust call requiring `&mut`
must be the same fact that the mutability analysis reads.** That is a property of the
*lowering*, not a property of a name, so it should be attached where the lowering is
declared:

1. Give the native-collection method table one entry per `(JavaCollectionKind, java
   name)` carrying the Rust target *and* its receiver mutability — the facade ABI table
   at `src/stdlib.rs:1250`+ already has the right shape (`rust_stdlib_method_abi(owner,
   "add", 1, "Vec::push", …)`); it is missing only a `ReceiverMutability` field. Derive
   the snake-case spelling rather than listing it, which kills the `add_all`/`addAll`
   duplication in all five lists at once.
2. Make A, B, C, D and E *queries against that table*, keyed by `(kind, name)`. B then
   stops being kind-agnostic for free — today `is_mutating_collection_method_name` says
   `put` mutates a `List` and `push` mutates a `Map`, which is only harmless because it
   over-approximates in the safe direction.
3. Model argument mutation separately from receiver mutation. `Collections.sort(x)`,
   `Collections.reverse(x)`, `Collections.shuffle(x)`, `Arrays.sort(x)` and
   `System.arraycopy(..)` all mutate an *argument*; `ExternalApiRole::Mutator` cannot
   express that, and `expr_mutates_self_field` (`src/usage.rs:2659`) only ever inspects
   `Expr::Call { receiver: Some(..) }`, so a field passed to any of them is invisible.
4. Pin it with a generated-crate check whose fixture keeps the collection **in a field**.
   One `#[test]` that loops over every entry in the unified table, emits
   `class F { private List<T> f; void m(..) { this.f.<method>(..); } }`, and compiles the
   result would have caught this on the day `removeAll` was added — and would catch the
   next one.

Until (1)–(3), a cheap holding measure with real value: a `#[test]` asserting
`A ⊆ B` and `A ⊆ C` by iterating a shared const array of names. It does not remove the
possibility of drift, but it converts it from silent to loud.

## Adjacent defects surfaced by the same probe (not this audit)

The battery turned up four unmodelled mutators that emit method names Rust does not
have. They are *not* list drift — they are simply absent everywhere — but they share the
"and it also got `&self`" symptom, which is why the list gap was hard to see:

| Java | emitted | error |
|---|---|---|
| `list.set(0, v)` | `self.list.set(0, v)` | E0599 |
| `list.sort(null)` | `self.list.sort(None)` | E0061 |
| `map.putIfAbsent(k, 1)` | `self.map.put_if_absent(k, 1)` | E0599 |
| `map.merge(k, 1, Integer::sum)` | `self.map.merge(k, 1, Integer::sum)` | E0599 |
| `map.computeIfAbsent(k, f)` | `self.map.compute_if_absent(k, f)` | E0599 |
| `list.retainAll(v)` | `self.list.retain_all(v)` | E0599 |
| `deque.pop()` | `self.stack.pop()` (typed `String`) | E0308 |

`List.set` and `Map.computeIfAbsent` are common enough that they belong on the backlog
regardless. Item (1) above is the natural place to add them, because adding them to the
unified table would give them their mutability for free.
