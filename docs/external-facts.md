# External Facts File

The translator accepts a line-oriented facts file through `--facts`.
These facts are parsed into `FactSet` before representation, signature, shape,
and coercion planning run. The intent is to inject corpus analysis or user
knowledge without adding local lowering special cases.

Blank lines are ignored. `#` starts a comment.

## Hierarchy Closure

```text
closed Expr
closed Expr true
closed Expr false
```

`closed Type` is shorthand for `closed Type true`.

## Representation Overrides

```text
concrete User
closed-enum Expr ExprValue Add Num
dyn-trait Sink SinkTrait
boxed-dyn-trait Sink SinkTrait
erased-object ExternalNode
```

These select the object representation strategy for a Java hierarchy root.

## Type Capabilities

```text
copy ExternalId
copy ExternalId true
copy ExternalId false
clone ExternalHandle false
string-like ExternalLabel true
sequence-like ExternalIds ExternalId
extends Dog Animal
implements Dog Pet Named
final Dog
final Dog true
object-safe-trait Pet false
object-safety-violation Pet static-method of
object-safety-violation Pet self-return next
object-safety-violation Pet nested-self-return nextAll
object-safety-violation Pet self-param accept other
object-safety-violation Pet nested-self-param acceptAll others
object-safety-violation Pet field VERSION
object-safety-violation Pet constructor
object-safety-violation Pet unsupported-member
```

Capability facts feed the same `FactSet` queries used by signature solving,
shape projection, representation planning, and coercion. For example,
`sequence-like ExternalIds ExternalId` lets stream and indexed projection treat
`ExternalIds` as a sequence of `ExternalId`, while `copy ExternalId` lets the
element shape become a copied value instead of a borrowed view.

The parser currently expects Java type names without spaces inside a fact line.
Generic type spellings that the Java type parser accepts as one token can be
used as capability values.

`object-safe-trait Type false` only records the boolean capability. The
`object-safety-violation` forms additionally record why a dyn/boxed dyn trait
mapping must fall back to erased object representation. These reasons are
consumed by representation policy diagnostics; lowering and coercion should not
inspect interface syntax to rediscover them.

## External API Effects

```text
external-api getValue * getter read-only
external-api setValue ExternalCounter setter mutates-receiver
external-api get ExternalList indexed-getter read-only
external-api put ExternalMap indexed-setter mutates-receiver
```

`external-api METHOD RECEIVER ROLE MUTABILITY` records a user override for an
untranslated or unresolved library call. `RECEIVER` is a Java type name, or `*`
for a global method-name fact. Receiver-specific facts are preferred over global
facts for matching receiver types.

Supported roles are `getter`, `boolean-getter`, `setter`, `mutator`,
`fluent-mutator`, `indexed-getter`, `indexed-setter`, and `unknown`.

Supported receiver mutability values are `read-only`, `mutates-receiver`, and
`unknown`.

These facts feed semantic analysis and signature planning. Downstream lowering
must consume the facts and plans rather than adding new name heuristics for
external getters, setters, or mutators.

## Field Nullability

```text
field-nullability User name definitely-non-null
field-nullability Cache.value maybe-null
field-nullability Node.parent unknown
```

`field-nullability TYPE FIELD STATE` and `field-nullability TYPE.FIELD STATE`
record user overrides for Java field storage nullability. Supported states are
`definitely-non-null`, `maybe-null`, `definitely-null`, and `unknown`.
Aliases `non-null`, `nullable`, and `null` are accepted.

External field-nullability facts override constructor and method proofs. This
allows corpus-level or user-provided knowledge to force Rust storage to direct
`T`, `Option<T>`, or an unknown conservative shape through the same fact query
used by signature planning.
