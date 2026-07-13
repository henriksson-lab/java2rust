# GATK Rust Idiom Sweep

Date: 2026-07-13

Generated Rust root:
`/tmp/new_java2rust_gatk_sweep/stdout`

Status/source map:
`/tmp/new_java2rust_gatk_sweep/status.tsv`

This was an output audit only. Parser and lowering implementation files were not
edited.

## Commands

```sh
find /tmp/new_java2rust_gatk_sweep/stdout -maxdepth 2 -type f | wc -l
sed -n '1,20p' /tmp/new_java2rust_gatk_sweep/status.tsv
rg -n 'compile_error!|todo!|unimplemented!|panic!|Box<dyn|Option<|Vec<|HashMap|Rc<|RefCell|clone\(|\.into\(\)|\.unwrap\(\)|\.expect\(|get\(|set\(|new\(|String::from|to_string\(\)|as_ref\(\)|as_mut\(\)|static mut|unsafe|pub struct|pub enum|impl' /tmp/new_java2rust_gatk_sweep/stdout
rg -n 'for .* in .*\.iter\(\)|while |loop |match |if let|return |let mut|self\.' /tmp/new_java2rust_gatk_sweep/stdout
rg -n 'HashSet|BTree|Iterator|collect\(|map\(|filter\(|flat_map\(|zip\(|enumerate\(|fold\(|sorted|stream|lambda|closure|\|' /tmp/new_java2rust_gatk_sweep/stdout
python3 /tmp/gatk_idiom_sweep.py
nl -ba /tmp/new_java2rust_gatk_sweep/stdout/00001.rs | sed -n '40,90p'
nl -ba /tmp/new_java2rust_gatk_sweep/stdout/00003.rs | sed -n '1,80p'
nl -ba /tmp/new_java2rust_gatk_sweep/stdout/00011.rs | sed -n '84,96p'
nl -ba /tmp/new_java2rust_gatk_sweep/stdout/00051.rs | sed -n '45,75p'
nl -ba /tmp/new_java2rust_gatk_sweep/stdout/01496.rs | sed -n '110,135p'
```

The helper script used for aggregation was kept at `/tmp/gatk_idiom_sweep.py`.
It counted regex-defined idiom signals and mapped numeric output IDs back to
`status.tsv`.

## Counts

| Metric | Count |
|---|---:|
| Generated `.rs` files audited | 1595 |
| Files with at least one non-placeholder idiom signal | 1312 |
| Files without a counted non-placeholder idiom signal | 283 |
| Files in that remainder containing only unsupported-placeholder signal | 46 |

Top counted patterns:

| Pattern | Occurrences | Files | Top output IDs |
|---|---:|---:|---|
| Explicit `return` where tail expressions would fit | 13182 | 1335 | `01577`, `01253`, `00435`, `01227`, `00422` |
| Java-style collection APIs (`add`, `size`, `entry_set`, etc.) | 8443 | 785 | `00457`, `00225`, `00422`, `01221`, `01144` |
| Runtime `.downcast::<T>().expect("invalid Java cast")` | 7799 | 1050 | `01577`, `00437`, `00422`, `01144`, `00690` |
| Dereferenced dynamic casts (`*expr.downcast...`) | 7499 | 1050 | `01577`, `00437`, `00422`, `01141`, `00690` |
| Spelled-out acronym identifiers (`g_a_t_k...`) | 6850 | 635 | `01573`, `00018`, `00431`, `00626`, `01376` |
| Java static owner/module spellings (`arrays.`, `collections.`, `double.`) | 6496 | 818 | `00457`, `00437`, `01577`, `01050`, `01141` |
| `None` used as Java `null` against concrete types | 3174 | 731 | `00435`, `00437`, `00678`, `01468`, `01046` |
| Java stream/collector chains emitted literally | 2379 | 383 | `00690`, `01577`, `00437`, `01036`, `00813` |
| Synthetic switch loop state machines | 1953 | 75 | `01447`, `00435`, `01195`, `01260`, `01159` |
| Java array creation as empty `Vec::new()` | 456 | 182 | `01455`, `00833`, `00868`, `01490`, `00850` |
| Java `!`/parenthesized boolean syntax | 27 | 22 | `01144`, `00691`, `00777`, `01138`, `00138` |
| Compound assignment inside expressions | 99 | 47 | `01496`, `01226`, `01151`, `01447`, `00900` |

Unsupported placeholders were not treated as findings by themselves. They are
only cited below where the surrounding emitted Rust exposes an idiom issue, such
as literal Java stream chains or array-construction shape.

## Findings

### Dynamic Java Casts In Typed Rust

Representative output:

```rust
// 00001.rs:46, htsjdk/samtools/SAMRecordSparkCodec.java
let mut attribute: SAMBinaryTagAndValue =
    *alignment.get_binary_attributes().downcast::<SAMBinaryTagAndValue>().expect("invalid Java cast");
while (attribute != None) {
    attribute = *attribute.get_next().downcast::<SAMBinaryTagAndValue>().expect("invalid Java cast");
}
```

This is non-idiomatic because Rust normally expresses this through concrete
return types, enums, traits, `Option<T>`, or `Result<T, E>`, not erased values
with runtime downcasts and panicking `expect`. The leading `*` also assumes a
boxed object shape that is not visible in the surrounding type.

Add minimized fixture/tests:
`examples/idiom-gaps/TypedGetterAvoidsDowncast.java`, covering a typed getter
assigned to a concrete local, and
`examples/idiom-gaps/NullableLinkedNode.java`, covering a nullable next pointer
that should become `Option`-shaped instead of `T = None`.

### Java `null` Rendered As `None` Without `Option`

Representative output:

```rust
// 00003.rs:53-58, org/broadinstitute/hellbender/Main.java
pub fn run_command_line_program(program: CommandLineProgram, raw_args: Vec<String>) -> Object {
    if (None == program) {
        return None;
    }
    return *program.instance_main(main_args).downcast::<Object>().expect("invalid Java cast");
}

// 00003.rs:70-72
let mut program: CommandLineProgram = None;
```

This is non-idiomatic and often invalid Rust: `None` is only meaningful for an
`Option<T>`, and comparing a concrete object to `None` loses the nullability
contract. The generated shape should make nullability explicit at the type
boundary.

Add minimized fixture/test:
`examples/idiom-gaps/NullableParameterAndLocal.java`, with a nullable parameter,
nullable local initialization, and null return.

### Java Collections Emitted As Java APIs

Representative output:

```rust
// 00003.rs:29-33, org/broadinstitute/hellbender/Main.java
let package_list: Vec<String> = ArrayList::new();
package_list.add_all(arrays.as_list("org.broadinstitute.hellbender".to_string()));
package_list.add_all(arrays.as_list("picard".to_string()));
return package_list;

// 00517.rs:138-151, PSScorer.java
let tax_ids_to_scores: Map = HashMap::new();
let invalid_ids: Set = HashSet::new();
hit_invalid_tax_ids.add(tax_id);
invalid_ids.add_all(hit_invalid_tax_ids);
```

This is non-idiomatic because the declared Rust containers (`Vec`, `HashMap`,
`HashSet`) should use Rust methods (`push`, `extend`, `insert`, `len`,
`is_empty`) and standard iterator adapters. Java collection wrapper names like
`ArrayList`, raw `Map`, and raw `Set` obscure ownership and element types.

Add minimized fixture/tests:
`examples/idiom-gaps/CollectionAddAllAndSize.java` and
`examples/idiom-gaps/MapSetMutation.java`, asserting Rust container methods
rather than Java collection method names.

### Java Streams And Collectors Kept As Streams

Representative output:

```rust
// 00011.rs:91, GATKAnnotationPluginDescriptor.java
let annotations: Set =
    *self.tool_default_groups.stream()
        .map(compile_error!("unsupported Java expression: lambda"))
        .flat_map(compile_error!("unsupported Java expression: method reference"))
        .collect(collectors.to_set())
        .downcast::<Set>()
        .expect("invalid Java cast");
```

The unsupported lambda/method-reference placeholders are expected gaps, but the
surrounding idiom is also wrong: Rust should lower stream pipelines to iterator
chains over typed collections, using `.iter()`/`.into_iter()`, `.map`,
`.flat_map`, `.filter`, and `.collect::<HashSet<_>>()` or `Vec<_>`.

Add minimized fixture/tests:
`examples/idiom-gaps/StreamMapFlatMapCollectSet.java` and
`examples/idiom-gaps/StreamFindAnyMatch.java`, separate from parser lambda
coverage, to assert the emitted Rust pipeline shape once expressions are known.

### Identifier And Static Owner Mangling

Representative output:

```rust
// 00003.rs:1-15, org/broadinstitute/hellbender/Main.java
pub struct Main {
    g_r_e_e_n: String,
    b_o_l_d_r_e_d: String,
    c_o_m_m_a_n_d_l_i_n_e_e_x_c_e_p_t_i_o_n_e_x_i_t_v_a_l_u_e: i32,
}

// 00001.rs:53, htsjdk/samtools/SAMRecordSparkCodec.java
self.binaryCodec.write_int(s_a_m_record.NO_ALIGNMENT_REFERENCE_INDEX);
```

This is non-idiomatic because acronym and constant handling splits every letter,
making names unreadable and inconsistent with Rust `snake_case` and
`SCREAMING_SNAKE_CASE`. Static Java owners are also rendered with field-style
dot access rather than Rust module/associated-item syntax.

Existing fixture coverage already includes
`examples/idiom-gaps/AcronymIdentifierNames.java`. Add a smaller static owner
fixture, `examples/idiom-gaps/StaticConstantOwner.java`, covering
`Owner.CONSTANT` and `Math.max`/`Arrays.fill`-style utility calls.

### Java Array Creation As Empty Vectors

Representative output:

```rust
// 00001.rs:78-81, htsjdk/samtools/SAMRecordSparkCodec.java
let mut qualities: Vec<i8> = alignment.get_base_qualities();
if (qualities.len() as i32 == 0) {
    qualities = /* Java array creation: i8[alignment.get_read_length()] */ Vec::new();
    arrays.fill(qualities, 0xFF);
}
```

This is non-idiomatic and behaviorally wrong: `new byte[n]` is not an empty
vector. It should become a length-preserving allocation such as `vec![0; n]`,
or a typed allocation followed by idiomatic fill/mutation.

Existing fixture coverage includes
`examples/idiom-gaps/ToArrayEmptyStringArray.java`; add
`examples/idiom-gaps/PrimitiveArrayWithComputedLength.java` for `new byte[len]`
and `Arrays.fill`.

### Synthetic Switch State Machine

Representative output:

```rust
// 00051.rs:50-75, org/broadinstitute/hellbender/engine/AlignmentContext.java
let __java_switch_value = type_;
let mut __java_switch_matched = false;
'java_switch: loop {
    if !__java_switch_matched && (__java_switch_value == c_o_m_p_l_e_t_e) {
        __java_switch_matched = true;
    }
    if __java_switch_matched {
        return *self.downcast::<AlignmentContext>().expect("invalid Java cast");
    }
}
```

This preserves Java fallthrough mechanics mechanically, but for non-fallthrough
switches it is not idiomatic Rust. Simple switches should become `match`
expressions or `match` statements. Fallthrough cases may need explicit grouped
arms or a documented fallback, but the state-machine loop should not be the
default output.

Add minimized fixture/tests:
`examples/idiom-gaps/SwitchReturnsEnumMatch.java` for a no-fallthrough switch
and `examples/idiom-gaps/SwitchGroupedCases.java` for grouped Java labels.

### Java Syntax Leaks In Expressions

Representative output:

```rust
// 00138.rs:30-31, FlowBasedHmerBasedReadFilterHelper.java
if (!((first && start_hard_clipped) || (last && end_hard_clipped))) {
    if (!impl.test_hmer(values, ofs, hmer_length)) {
        return false;
    }
}

// 01496.rs:116-122, RecalUtils.java
report_table.set(row_index, column_names.get(column_index += 1).get_left(),
                 covariates.get_read_group_covariate().format_key(keys[key_index += 1 as usize]));
```

Rust allows `!expr`, but `if (!cond)` is Java-shaped and noisy. Compound
assignment inside an expression is a stronger problem: Java's `++`/assignment
expression semantics do not translate to Rust expression positions. These need
statement extraction before the call.

Add minimized fixture/tests:
`examples/idiom-gaps/BooleanNegationStyle.java` and
`examples/idiom-gaps/IncrementInsideCallArgument.java`.

## Priority Fixture Set

Add these first under `examples/idiom-gaps` with assertions in
`tests/idiom_gaps.rs`:

| Fixture | Assertion target |
|---|---|
| `NullableParameterAndLocal.java` | Uses `Option<T>` shape or a deliberate unsupported nullability marker, not `T = None`. |
| `CollectionAddAllAndSize.java` | Emits `Vec::new`, `extend`/`push`, and `.len()`, not `ArrayList::new`, `.add_all`, or `.size()`. |
| `PrimitiveArrayWithComputedLength.java` | Emits a length-preserving vector allocation, not `Vec::new()`. |
| `StreamMapFlatMapCollectSet.java` | Once lambdas are supported, emits Rust iterator/collect shape, not `.stream()`/`collectors`. |
| `SwitchReturnsEnumMatch.java` | Emits a `match` shape for non-fallthrough switch. |
| `IncrementInsideCallArgument.java` | Extracts mutation before the call; no `+=` inside an argument/index expression. |
| `StaticConstantOwner.java` | Emits Rust module/associated item spelling for Java static constants/utilities. |

These fixtures are intentionally smaller than the GATK examples. They isolate
the output idiom issue from unsupported inheritance, exception, lambda, and
method-reference gaps.
