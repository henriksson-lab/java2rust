# Current Corpus Issue Audit

Date: 2026-07-23

This audit records the latest focused reruns after the source-summary semantic
drift fixes and constructor self-initialization fix. Focused runs used
`scripts/translate-focused-package.sh --skip-resolver-bench --jobs 10
--translate-timeout-sec 60` with the release translator unless noted.

## Latest Evidence 2026-07-27

| Corpus | Output | Result |
|---|---|---|
| `jaligner` | `.tmp/corpus-rerun-20260727-jaligner-final` | 29/29 translated, 0 invalid-pattern files, `cargo_check=ok`, 0 generated-Rust errors. Simple-name external erasure no longer overrides parsed source `Matrix`, and public source methods that explicitly return `null` select optional return slots. |
| `fastqc` full corpus | `.tmp/corpus-batch-current-fastqc-refresh-20260727-after-source-erasure` | 156/156 translated, 0 translation failures, 29 invalid-pattern files, 1459 generated-Rust errors. Runtime remains dominated by global analysis/signature work: total 2,110,447 ms, translation 1,650,111 ms, global analysis 1,254,644 ms, signature solve 795,894 ms, signature fixpoint 767,748 ms, semantic analysis 456,896 ms, crate assembly 447,799 ms. |
| `fastqc` AbstractQCModule subset | `.tmp/corpus-fastqc-abstract-qcmodule-after-inherited-exception-owner-r1` | 8/8 translated, 0 invalid-pattern files, generated Rust reaches two `cannot return value referencing temporary value` rows in `SVGImageSaver`. The inherited `?`/result-shell family is gone after inherited body lowering started using the declaring type's exception plan. |
| `fastqc` AbstractQCModule subset | `.tmp/corpus-fastqc-abstract-qcmodule-after-reassigned-return-owned-r3` | 8/8 translated, 0 invalid-pattern files, `cargo_check=ok`, 0 generated-Rust errors. Reassigned string-like parameters now force owned/native parameter slots and returned-alias borrow constraints reject reassigned parameters, fixing `SVGImageSaver.optimizeSvg`. |
| `fastqc` Base64 single-source subset | `.tmp/corpus-fastqc-base64-single-after-local-type-rust-key-r1` | 1/1 translated, 1 invalid-pattern file, generated-Rust errors reduced to 64. The shared-support merger now removes the bad simple `OutputStream = Base64OutputStream` alias while preserving the real source-owned nested `Base64OutputStream` implementation, and local Rust identities suppress simple external facade stubs. The previous duplicate `OutputStream`/`Base64OutputStream` and conflicting self-conversion rows are gone. Remaining rows are real Base64 translation gaps: outer static member qualification from nested classes, constructor self-field initialization, inherited `FilterOutputStream.out` state, `java_string_from_bytes`, `File.length`, bare field assignment, and byte-array move/bitwise coercion drift. |
| `fastqc` Base64 single-source subset | `.tmp/corpus-fastqc-base64-single-after-focused-file-sync-r1` | 1/1 translated, 1 invalid-pattern file, generated-Rust errors reduced to 59. The focused wrapper now mirrors the platform `File::new_1/new_2`, `File.length()`, and `java_string_from_bytes` support surfaces, so the missing `java_string_from_bytes` and `File.length` families from the previous focused Base64 run are gone. |
| `fastqc` Base64 single-source subset | `.tmp/corpus-fastqc-base64-single-after-nested-static-owner-r1` | 1/1 translated, 1 invalid-pattern file, generated-Rust errors reduced to 41. Receiverless static calls resolved to an outer declaring source type now lower to that owner instead of `Self`, and owner-qualified associated constants are available for bare nested reads. The previous nested `Self::ENCODE`/`Self::get_decodabet` missing-owner family is gone. Remaining top families are mismatched types, inherited `FilterOutputStream.out` state, constructor `self` use before construction, bare `out` assignment, byte-array moves, and `i8 | i32` coercion. |

Current FastQC top families after the 2026-07-27 full rerun:

- mismatched types: 277 rows, mostly broad representation/facade slot drift;
- fallibility shell propagation: 126 non-Result `?` rows and 88 `?`-on-non-`Try` rows;
- AWT/Swing facade state gaps: erased receivers missing typed methods such as
  `set_color`, `set_size`, `set_double_buffered`, `print`, `add_notify`,
  `get_font_metrics_0`, `draw_line`, `draw_string`, and `fill_rect`;
- switch/state-machine scaffold locals: unresolved `x0`, `x1`, `r`, and
  similar generated names;
- Base64 stream state and byte coercion drift: the simple
  `OutputStream = Base64OutputStream` alias/stub conflict and nested outer
  static-owner rows are fixed in focused proof, while constructor self-field
  initialization, inherited `FilterOutputStream.out` state, bare `out`
  assignment, byte-array moves, and `i8 | i32` coercion remain;
- JDK/platform facade overload gaps: `File::new_1`, `File::new_2`,
  `BorderLayout::CENTER/NORTH`, `JLabel::CENTER`, and `RenderingHints` constants;
- source mutability inference gaps: methods mutating fields such as
  `self.random_data` and `self.v` still have `&self` receivers.

## Latest Evidence 2026-07-24

| Corpus | Output | Result |
|---|---|---|
| `jaligner` | `.tmp/corpus-sweep-20260724-jaligner-after-upcast-field-visibility-r1` | 29/29 translated, 0 invalid-pattern files, `cargo_check=ok`, 0 generated-Rust errors. |
| `fastqc` AbstractQCModule subset | `.tmp/corpus-fastqc-abstract-qcmodule-after-indexed-borrow-r1` | clean: 8/8 translated, 0 invalid-pattern files, offline `cargo_check=ok`, 0 generated-Rust errors. Indexed `Vec<String>` reads feeding borrowed append/optional slots no longer move out of the vector. |
| `fastqc` full corpus | `.tmp/corpus-batch-current-fastqc-after-syntax-fixes-r1` | 156/156 translated, 39 invalid-pattern files, 306 generated-Rust errors. This removed the previous `pub in` shared-support field and `Localizable:` import syntax blockers. |
| `fastqc` full corpus | `.tmp/corpus-batch-current-fastqc-after-facade-keyword-and-strip-r1` | 156/156 translated, 39 invalid-pattern files, 1401 generated-Rust errors. This removed the `await` facade keyword method syntax blocker and same-module duplicate `Localizable` trait definition, but exposed many real type/representation errors. Crate assembly regressed to 1,130,913 ms and needs follow-up optimization before repeated full FastQC sweeps. |

Current FastQC top families after the latest full rerun:

- representation/trait-object recovery: erased object to borrowed dyn trait
  placeholders for `RealDistribution`, `IntegerDistribution`, and `QCModule`;
- AWT/Swing facade state gaps: many `Box<dyn Any>` receivers for graphics and
  component methods such as `set_color`, `set_size`, `print`, and
  `get_font_metrics_0`;
- unsupported constructor/inheritance cases around Swing and stream subclasses;
- remaining switch-state-machine, array/class-literal, and `instanceof`
  placeholders from the idiom sweep;
- crate assembly performance: source-stub stripping/import assembly on FastQC
  is now a bottleneck and should be indexed before more repeated full runs.

## Local Gates

| Gate | Result |
|---|---|
| `cargo fmt --check` | passes |
| `cargo check --quiet` | passes |
| `cargo test --test idiom_gaps --quiet` | 2026-07-27 checkpoint: passes, 499 passed |
| `cargo test --test parser_gaps --quiet` | passes: 18 passed |
| targeted `idiom_gaps` indexed-borrow/IO/String regressions | passes |
| targeted `parser_gaps` local array declarator regressions | passes |
| `cargo test --test batch_translation --quiet` | passes: 5 passed |
| `cargo test --test type_coercion --quiet` | passes: 94 passed |
| `cargo test --test idiom_sweep_script --quiet` | passes: 22 passed |
| `cargo build --release --quiet --bin new-java2rust` | passes |

## Fixed In This Batch

- Source-summary field initializers are preserved, so inherited source-summary
  fields keep their nullability/initializer semantics.
- Inherited field facts use the declaring owner when applying field shape and
  nullability constraints.
- Inherited source-summary method return shapes use the declaring summary type,
  not the child type.
- Constructor assignment/delegation field names are normalized consistently.
- Non-delegating constructors that initialize a field by calling an instance
  method now build `__java_self`, assign the self-dependent field after
  construction, and recover unchecked `Result` calls with a panic path.
- `Collection.addAll`/extend lowering now routes through the shared native
  collection extension path.
- 2D numeric array literals ignore comment-only placeholder expressions before
  choosing `ndarray::arr2`.
- Source-owned nullable getter calls assigned into optional slots keep the
  planned optional return shape instead of becoming `Some(Option<T>)`.
- Nullable copy-constructor receiver field reads unwrap the receiver and use the
  owner-qualified field signature shape, so optional fields copy as
  `Option<T>` rather than `Some(Option<T>)`.
- Copy constructors that copy optional source fields through a guarded nullable
  receiver now preserve the source field's optional shape instead of wrapping
  already-optional values.
- Batch source merging keeps duplicate simple class names from different
  packages under distinct package-qualified owners, and required signature
  lookup can recover the unique package-qualified duplicate owner.
- Batch translation now emits live phase rows for parse, global analysis,
  resolver, semantic analysis, representation planning, signature solving, and
  emission. Signature solving also reports live constraint collection,
  dependency graph, SCC, pass-plan, fixpoint iteration, component, and component
  sweep progress.
- Signature propagation now skips whole scoped propagation arms when the active
  SCC does not contain any target slot kind for that arm. This preserves the
  current SCC-local saturation semantics while avoiding repeated global scans
  that cannot mutate the active component.
- Focused generated crates that emit `ndarray::` now use a generated local
  `ndarray` path shim for corpus checks. This removes the previous offline
  dependency-cache ambiguity for matrix/2D-array corpuses and exposes real
  generated-Rust diagnostics.
- Source-backed `super(args)` constructors can now inline a direct source
  superclass constructor into flattened inherited child fields when the
  superclass constructor body is simple. JDK `FileReader` superclass
  constructors are also modeled as no-state facade construction for generated
  corpus crates.
- Java-derived Rust keyword identifiers now render through raw identifiers in
  ordinary emitted identifier positions, and the focused invalid-pattern scanner
  treats raw optional locals such as `let mut r#in: Option<T> = None;` as
  optional storage rather than concrete `None` assignment.
- `System.out`/`System.err` `print`/`println` statements now lower to Rust
  `print!`/`println!`/`eprint!`/`eprintln!` macros instead of unresolved
  `Out::`/`Err::` pseudo-types. Nonzero-argument `PrintStream` support methods
  use overload-safe Rust names.
- Java exception logging now keeps `getLocalizedMessage()` available on the
  `JavaException` runtime support and lowers statement-side `printStackTrace`
  calls to an explicit omitted-stack-trace no-op instead of requiring
  `System::err` or overloaded Rust methods.
- Java string API normalization now lowers `toUpperCase`, `toCharArray`,
  literal `replace`, `replaceFirst`, `replaceAll`, and `matches` through Rust
  string methods or explicit runtime helpers. The focused wrapper imports the
  same helper surfaces for generated corpus crates.
- Constructed Java `String` returns now stay owned when the returned value is
  built from literals, concatenation, or string helper calls. Borrowed
  parameter-return propagation is limited to exact `return param;` aliases, so
  generated Rust no longer returns references to temporary `format!` or
  `to_string()` values for this VarScan family.
- Java `String` equality now compares non-optional string slots through string
  views, including array-indexed `Vec<String>` elements and borrowed string
  parameters. This removes the VarScan `String`/`&String` comparison family.
- JDK facade call planning now covers `Math.log`, `Math.exp`, `Double.isNaN`,
  `DecimalFormat.format`, `File.exists`, reader `ready`, `System.exit`, and
  `BitSet.get` with explicit return shapes and native Rust lowering. This
  removes the VarScan numeric/display/arity/file-exists/boolean-void facade
  rows from the current top-error list.
- Java collection `toArray`, including `(String[]) map.keySet().toArray(new
  String[0])`, now lowers through the native collection plan to an owned
  `Vec<_>` instead of emitting a missing `.to_array(...)` Rust method.
- Selected Rust stdlib ABI facts now seed low-precedence `FacadeAbiFact` rows
  for Java `String`/Rust `str` calls, and call argument planning can derive
  borrowed argument slots directly from `FacadeAbiFact.args` before falling back
  to older external argument-effect rows.
- Rust stdlib/core/alloc coverage is tracked as signature-fact clusters rather
  than a broad handwritten special-case model. When a Rust signature fact gives
  the same receiver, argument, return, borrow, optional, result, or index shape
  as a tailored rule, the fact-backed ABI plan is preferred and the special
  case becomes migration debt unless it preserves Java-specific semantics.
- Helper-backed Java string regex/split APIs now have stdlib signature facts
  for their mechanical Rust ABI: `matches`, `replaceFirst`, `replaceAll`,
  `split`, and `indexOf` request borrowed regex/pattern/needle slots through
  `FacadeAbiFact`, while the runtime helpers continue to own Java-specific
  regex/split/index behavior. Borrowed string literals lower generically as
  `&str` literals for borrowed string slots, so the fact-backed path keeps the
  same clean helper calls the older string-pattern branch produced.
  `contains`, `startsWith`, `endsWith`, and literal `replace` now also use the
  shared ABI slot plan during emitted lowering when the stdlib facts are
  available; the older pattern argument branch remains only as fallback for
  unshaped calls.
- Static numeric parse calls now use stdlib signature facts for their argument
  ABI. `Integer.parseInt(String)` and `Double.parseDouble(String)` select
  associated/static `FacadeAbiFact` rows with borrowed string parameters; the
  native lowering still renders the Java parse result as `i32`/`f64`.
- Common math/static numeric calls also have fact-backed mechanical argument
  ABI rows. `Math.log`, `Math.exp`, `Math.toIntExact`, `Double.isNaN`, and
  `Float.isNaN` select associated/static `FacadeAbiFact` rows with owned/copy
  primitive arguments while native lowering continues to render the Java
  operation and return type.
- Stream lambda receiver calls now reuse the shared call-argument planner for
  fact-backed shaped and explicit-slot argument plans. Lambda parameter types
  are carried into the planner, so stream calls such as `String.charAt(index)`
  can use the same `FacadeSlot::Index`/`Slot::Index` ABI path as ordinary
  calls before native rendering performs the final Rust `usize` handoff.
- Stream lambda result-shape inference now consumes shared `call_plan`
  native-Java lowered-type projection instead of carrying a stream-local table
  for `Integer.parseInt`, `Double.parseDouble`, `Math.toIntExact`,
  `String.trim`, `String.toLowerCase`, and `String.split`.
- Ordinary expression shape inference now consults that same native-Java
  lowered-type projection before legacy borrowed-string view fallback, so
  stream and non-stream expression shape no longer maintain separate result
  knowledge for the same native call targets.
- The old bespoke `lower_string_pattern_args` emitter is gone. The residual
  no-facts string-pattern compatibility branch now produces a borrowed-string
  `ArgumentLoweringPlan` and lowers through the shared argument-plan path,
  while normal translation continues to prefer stdlib `FacadeAbiFact` rows.
- Java `String.charAt(int)` now lowers to
  `.chars().nth(index as usize).expect("Java string index out of bounds")`,
  with the index argument flowing through the shared index-slot coercion path
  instead of emitting unresolved `.char_at(...)`.
- `FacadeSlot::Index` now lets Rust stdlib/facade ABI facts request that same
  index-slot coercion directly. Parsed `usize` Rust signature metadata maps to
  the index facade slot, and `String.charAt` argument planning is verified to
  select `Slot::Index` from the stdlib ABI fact rather than a call-site
  argument branch.
- HTSJDK `SAMTag` enum-variant constants now come from typed external field
  facts. Static constant support treats `ExternalFieldKind::EnumVariant` as
  owner-typed, so imported `SAMTag.CO` can lower through the qualified
  `HtsjdkSamtoolsSAMTag` facade without synthesizing a conflicting simple
  placeholder type.
- Private static singleton getters backed by `private static T instance = new
  T();` now return an owned initialized source value and lower `return
  instance;` to the source constructor instead of emitting a borrowed reference
  to nonexistent Rust static storage.
- Java platform `File` facade support now includes resolver-compatible
  constructor overload shims. `File::new_1(path)` delegates to the existing
  single-argument `File::new`, and `File::new_2(parent, child)` implements the
  Java parent/child path join. This targets the latest FastQC
  `File::new_1`/`File::new_2` missing-method family while preserving distinct
  Java overload arities.
- Java `String.getBytes(charset)` now has a stdlib ABI fact for the borrowed
  charset argument and lowers through the existing native string byte-vector
  path. This targets the latest FastQC `get_bytes_1` missing-method rows
  without adding a generated `String` extension shim.
- Java byte-array `String` constructors now lower through
  `java_string_from_bytes` / `java_string_from_byte_range` helpers for the
  byte-array, byte-array plus charset, range, and range plus charset overloads.
  This targets the latest FastQC `String::new(...)` arity rows from Base64
  decoding/encoding paths.
- Source static mutable fields now also handle Java's static-through-instance
  field assignment form when the receiver expression is a same-class static
  holder, for example `commons.jnlp = jnlp`. The field write lowers to the
  generated static storage setter instead of an unresolved local or instance
  write. Focused JAligner rerun
  `.tmp/corpus-sweep-20260724-jaligner-after-qualified-static-storage-r1`
  has 29/29 translated, 0 invalid-pattern files, and generated-Rust errors
  down from 17 to 16; the previous `cannot find value commons` row is gone.
- Public source `String` parameters that are proven read-only content inputs now
  lower as borrowed string-content slots, e.g. `&str`, while preserving the
  public boundary's nullability constraints. Focused JAligner rerun
  `.tmp/corpus-sweep-20260724-jaligner-after-public-string-content-param-r1`
  has 29/29 translated, 0 invalid-pattern files, and generated-Rust errors
  down from 16 to 14; the two `expected &String, found &str` rows for
  `load_sample_sequence(Self::SAMPLE_SEQUENCE_...)` are gone.
- Java integer arithmetic used as an array index now casts the whole expression
  to `usize`, e.g. `(a + b) as usize`, instead of casting only the right-hand
  operand as `a + b as usize`.
- Primitive-valued `Map.get` results used in arithmetic now unwrap and copy the
  optional borrowed map value before numeric lowering, including boxed Java
  primitive generics such as `HashMap<String, Integer>`.
- Java assignment expressions in value position now preserve Java's assigned
  value semantics for local variables, and assigned method parameters receive
  mutable local shadows before the body.
- Java prefix update expressions such as `++a` and `--b` are parsed distinctly
  from postfix updates and lower in value position to block expressions that
  update the local and return the updated value.
- Fallible assignment-in-condition values such as
  `while ((line = reader.readLine()) != null)` now go through optional slot
  conversion. `BufferedReader.readLine()` is modeled as
  `Result<Option<String>, JavaException>`, so generated loops assign the direct
  optional result (`line = reader.read_line()?`) while preserving optional
  carrier behavior for non-fallible unknown external calls. Compound conditions
  such as `((line = reader.readLine()) != null && ...)` assign then test
  `line.is_some()` inside the block, avoiding a move of the optional local.
- Java parameter declarators now preserve `[]` dimensions written after the
  parameter name, such as `final double result[]`, so old-style out-parameter
  arrays lower to slice/vector slots instead of scalar `f64`.
- Optional Java sequence receivers refined by null guards now normalize writes
  to `param.as_mut().expect(...)` and reads to `param.as_ref().expect(...)`
  without appending those calls after `.unwrap()`. Method parameter rendering
  marks nullable sequence out-parameters as `mut` when array-element writes
  require a mutable optional binding.
- Internal source-call signature propagation now carries `MutableBinding` back
  to caller locals when the callee parameter is a mutable-borrow slot. This
  covers local arrays lowered as `Vec<T>` and passed to source out-array
  methods through `.as_mut_slice()`.
- Resolved source-call mutable-binding propagation now covers public/static
  source helper calls without broad ownership propagation across unknown
  boundaries. Focused FastQC math-util evidence:
  `.tmp/corpus-fastqc-math-util-after-source-mut-local-abi-infra-r1` remains
  10/10 translated with 0 invalid-pattern files and 14 generated-Rust errors,
  and `ResizableDoubleArray.copy()` now declares `let mut result`. A trial
  `System.arraycopy` ABI fact was backed out because it worsened the subset to
  19 errors by exposing same-field read-after-mutable-borrow ordering; that API
  needs call-level temporary planning before enabling a simple ABI fact.
- Java `while (true)` now lowers as Rust `loop`, preserving Java definite
  assignment behavior for locals initialized inside an infinite loop before a
  `break`.
- Source `static final` primitive expression constants now emit associated
  constants and resolve parser-normalized references back to the original Java
  constant names, removing the exposed FastQC scalar constant rows such as
  `EXP_INT_TABLE_LEN`, `MASK_30BITS`, and the `F_*` constants.
- Source `static final` primitive/String array literals now emit associated
  static slice constants, including old-style field declarators such as
  `double FACT[]` and nested `double[][]` tables. Static const table reads use
  nested slice indexing, `.length` uses `.len()`, and shared slice arguments no
  longer append `.as_slice()` to an already-borrowed associated slice.
- Array initializer parsing now ignores `line_comment` and `block_comment`
  nodes, so commented literal tables do not become rejected const expressions.
- Inline 2D row replacement values for `ndarray` now borrow the generated vector
  when assigning through `ArrayView1::from(&vec![...])`.
- JDK `Thread.sleep(...)` now lowers to
  `std::thread::sleep(std::time::Duration::from_millis(...))`, lowercase parsed
  `thread` resolves to the `Thread` facade identity, Java `System.in` lowers to
  `System::r#in`, and static constant support emits the needed `InputStream`
  facade type.
- Typed constructor argument support now adds facade types that are referenced
  only through constructor argument facts, so `PrintStream::new(OutputStream)`
  no longer emits an undefined `OutputStream` type.
- `Map.get(...)` values used in display/string-concat slots now lower through
  `Option::map(...).unwrap_or_else(|| "null".to_string())`, so optional borrowed
  map values no longer flow directly into `format!`.
- Optional Java sequence receivers now unwrap through shared access planning for
  read indexing and `.length`, so nullable `String[]`/`double[][]` storage emits
  `as_ref().expect("Java null receiver")` before `[]`, `.len()`, or `.nrows()`
  instead of Rust `Option<Vec<_>>[...]`/`.length` forms. Array assignment
  targets use the write-side `as_mut()` variant and preserve the existing
  erased-object varargs no-clone regression.
- Internal erased-`Object` helper parameters keep their erased-object
  representation constraints local when a more precise caller binding flows
  into them. Same-type null-check helpers no longer erase concrete source
  parameters to `Box<dyn Any>`.
- Public source parameters whose fields or array elements are mutated now drop
  the public owned-parameter baseline before slot solving, so Java out-object
  helpers lower destination parameters as `&mut T` instead of owned values.
- Reused owned Java `String` locals now clone at repeated owned consuming
  slots, including vector initializers, local declarations, assignments, and
  later reads.
- Explicit Java primitive numeric casts now render as Rust casts before binary
  numeric promotion, and string-concat rendering no longer lets display
  conversion leak into arithmetic operands.
- Try/catch value-fallthrough with early value returns now uses an optional
  result carrier, so statement wrappers can return `String` values from inside
  fallible blocks without forcing `Result<(), JavaException>`.
- Raw Java collection return types can refine to a concrete generic collection
  shape when every explicit source-body return returns the same typed local.
- Statement-side non-void `super(...)` boundary calls now lower to an explicit
  typed no-op result propagation instead of unqualified `Default::default()?`.
- JDK `File` one-argument constructor lowering has a typed `String` argument
  slot, so borrowed indexed `String[]` values clone instead of moving out of a
  slice captured by a fallible wrapper.
- Reassigned Java `String` enhanced-for variables now iterate owned cloned
  elements with a mutable loop binding, matching Java's fresh local loop
  variable semantics.
- Java `Hashtable.put(...)` now lowers through the native map insertion path,
  nullable map fields mutate through `as_mut().unwrap().insert(...)`, queue
  `put(...)` lowers to vector-backed `push(...)`, and statement-side
  `ByteBuffer.put(...)`/`CharBuffer.put(...)` facade calls evaluate their
  argument without leaving Java `.put(...)` calls in generated Rust.
- Source-backed `super(...)` constructor lowering now resolves generic-erased
  superclass names such as `BaseSolver<FUNC>` to the unique source
  declaration, so child constructors can initialize flattened inherited fields
  through delegated source superclass constructors instead of emitting
  inheritance-lowering placeholders.
- Closed enum values can now borrow as dyn-trait parameters when source
  hierarchy facts prove that the enum implements the target interface, covering
  Apache-Commons-Math-style `LocalizedFormats` values passed as `Localizable`
  without allowing unrelated closed-enum/trait coercions.
- Nested Java interface/type names now render through valid Rust identifiers in
  trait emission, dyn-trait slot types, and overload suffixes, so names such as
  `Incrementor.MaxCountExceededCallback` no longer leak dots into generated
  Rust items or re-exports.
- Lower-case unproven receivers no longer become static owner calls, while
  explicit `javalibs` static-facade methods such as DISQ
  `HtsjdkVariantsRddStorage.makeDefault` and
  `TabixIndexWriteOption.fromBoolean` still recover canonical Rust owner calls.
- Parenthesized condition/expression parsing now skips comment nodes before
  choosing the wrapped expression, so FastQC-style
  `if (/*!encode &&*/ position >= numSigBytes)` lowers to the real comparison
  instead of a raw block-comment condition.
- Java numeric literal parsing now handles decimal float suffixes, non-decimal
  long suffixes including the two's-complement sign-bit mask
  `0x8000000000000000L`, and Java hex floating-point literals such as
  `0x1.0p-52d` and `0x1.0p-23f`.
- JDK stream-wrapper superclass construction is modeled as no-state for
  `GZIPInputStream`, `FilterInputStream`, and `FilterOutputStream` when the
  rest of the constructor body is already representable. This removes
  Base64's inner stream `super(in)`/`super(out)` placeholders and the public
  `MultiMemberGZIPInputStream` `super(input, size)` shapes in focused checks.
- Modeled no-state external `super(...)` constructors with unsupported trailing
  object graph mutation now emit a precise diagnostic instead of the broad
  inheritance-lowering placeholder. Focused
  `MultiMemberGZIPInputStream.java` translation now reports 2
  `unsupported Java constructor: super(...) followed by unsupported object graph mutation`
  diagnostics and 0
  `unsupported Java constructor: super(...) requires inheritance lowering`
  diagnostics for the private parent-linking constructors. This keeps
  `this.parent.child = this` blocked on an explicit object identity/handle
  model instead of pretending normal Rust ownership can represent it.
- Java `String.join(delimiter, value...)` varargs now lower through native
  string join by packing the values into a `Vec<String>`. Focused FastQC
  `test/integration/cli/Cli.java` translation now emits the classpath join as
  `vec![...].join(sep.as_str())` and has no
  `String.join varargs are not supported yet` placeholder.
- Source-backed `super(...)` constructor flattening now keeps supported
  trailing expression statements, including method-result calls such as
  `getContext().addMessage(...)`. In
  `.tmp/corpus-fastqc-exception-subset-after-trailing-expr`,
  `TooManyEvaluationsException` now emits the trailing `add_message` call
  after constructing `__java_self`; the remaining 2 broad `super(...)`
  placeholders in that subset are isolated to base exception constructors with
  `this(...)` delegation or external standard-exception `super(cause)` calls.
- Standard Java exception superclass calls with trailing subclass state now
  lower through the external-exception constructor path, bare constructor field
  assignments are recognized, and source-super constructor lookup handles
  varargs arity. In
  `.tmp/corpus-fastqc-exception-subset-after-varargs-super-matching`, broad
  `super(...) requires inheritance lowering` placeholders drop to 0. The subset
  still fails as generated Rust with 2 `instanceof` placeholders and 6 downcast
  rows, now exposing object-model/interface issues rather than constructor
  planning.
- Java `instanceof Serializable` checks now lower to an explicit
  `java_is_serializable(...)` JDK runtime helper. In
  `.tmp/corpus-fastqc-exception-subset-after-serializable-helper-import`, the
  focused FastQC exception subset translates 9/9 files with 0
  `compile_error!` placeholders, 0 unsupported array-creation placeholders, and
  6 remaining invalid Java downcast rows across 4 files. Generated cargo errors
  drop from 51 to 49; remaining invalid rows are all object-model/downcast
  issues.
- Constructor field assignments whose RHS reads a constructor-local declaration
  now use the `__java_self` body path instead of compacting the assignment into
  a struct literal before the local exists. `BufferedImage.getGraphics()` is
  modeled as a typed JDK AWT facade return, matching `createGraphics()`, and
  typed external returns assigned to same-typed fields now act as concrete
  native field-shape evidence. In
  `.tmp/corpus-fastqc-abstract-qcmodule-after-constructor-local-graphics-r4`,
  `SVGGenerator::new` keeps `let b: BufferedImage = ...`, assigns
  `__java_self.g = Some(b.get_graphics())`, and `g` is
  `Option<Graphics>` rather than `Option<Box<dyn std::any::Any>>`.
- Stable JDK GUI facade getter rows now carry typed return facts for
  `Component` dimensions, `Color` channels, `Font` size/bold state, and Swing
  `TableModel` counts/names/values. Erased `TableModel.getValueAt(...)`
  display uses now lower through `java_erased_object_to_string(...)`, removing
  the invalid `downcast::<String>()` row from the focused AbstractQCModule
  subset.
- Current self passed to a known Java exception supertype slot now lowers as an
  explicit facade boundary instead of an invalid erased-object downcast, and
  `HashMap<String, Object>` insert values stay in the erased-object carrier. In
  `.tmp/corpus-fastqc-exception-subset-after-object-map-erasure`, the focused
  FastQC exception subset translates 9/9 files with 0 invalid-pattern files: 0
  `compile_error!` placeholders, 0 invalid Java cast downcasts, 0 `.put` calls,
  and 0 unsupported array-creation placeholders. Generated cargo still fails
  with 43 errors, now headed by type/interface issues rather than invalid
  placeholders.
- Focused package source-type assembly now strips source-defined facade stubs
  separately from shared support cleanup. In
  `.tmp/corpus-fastqc-exception-subset-after-preserved-prelude-impls`,
  `ExceptionContext`, `Localizable`, and `ExceptionContextProvider` are exported
  as source items exactly once, while support impls such as
  `ObjectInputStream::read_object` and `ObjectOutputStream::write_object`
  survive. The subset remains 9/9 translated with 0 invalid-pattern files and
  generated cargo errors drop to 40.
- Direct JDK `StringBuilder.append(...)` and `StringBuffer.append(...)` calls
  are now modeled as fluent builder calls through shared platform API facts. In
  `.tmp/corpus-fastqc-exception-subset-after-stringbuilder-append-fact`, the
  focused exception subset remains 9/9 translated with 0 invalid-pattern files
  and generated cargo errors drop to 38. The remaining top families are
  `ExceptionContext` `Default`/`Clone`, `Localizable` trait-object recovery,
  erased-object/varargs mismatches, enum constant/interface coercion, and small
  JDK facade gaps such as locale/static constants.
- Java enum constant parsing now preserves source labels before representation
  planning, so Rust-valid uppercase constants such as `MAX_COUNT_EXCEEDED` and
  `ILLEGAL_STATE` remain stable through generated enum emission. Constructor
  delegation through Java varargs now supplies empty varargs as `Vec::new()`,
  and closed enum constants passed to source interface slots lower directly as
  `&LocalizedFormats::... as &dyn Localizable` instead of optional
  `.as_ref().expect("Java null receiver")` recovery. Source Java enums that
  implement zero-arg `String` interfaces now emit an enum trait impl fallback,
  using the enum display string when constructor-backed enum fields are not
  represented. In
  `.tmp/corpus-fastqc-exception-subset-after-enum-trait-impl-fix`, the focused
  exception subset remains 9/9 translated with 0 invalid-pattern files and
  generated cargo errors drop to 33. The `LocalizedFormats: Localizable` trait
  bound family is gone. Remaining top families are mismatched types,
  `ExceptionContext` `Default`/`Clone`, `Localizable` `as_any`, erased
  `Object[]`/varargs clone/display gaps, `Vec::size`, `Locale::US`,
  `arg_utils`, one string borrow mismatch, one unit-to-`Localizable` coercion,
  and one lifetime row.
- Constructor self-temporary seeding now reuses a self-dependent field
  initializer when that initializer can be lowered without reading the temporary
  `__java_self`. This avoids requiring `Default` for source structs such as
  `ExceptionContext` when the seed can be built through the same constructor
  boundary as the final assignment. In
  `.tmp/corpus-fastqc-exception-subset-after-self-temp-seed-fix`, the focused
  exception subset remains 9/9 translated with 0 invalid-pattern files and
  generated cargo errors drop to 30. The `ExceptionContext: Default` family
  drops from 5 rows to 2 rows. The remaining 2 rows are inherited `context`
  defaults in constructors whose Java semantics depend on implicit source
  superclass construction, e.g. `MaxCountExceededException` and
  `TooManyEvaluationsException`.
- Shared source varargs lowering now forwards explicit `Object...` vectors
  directly for owned public varargs slots, appends trailing forwarded varargs
  vectors for expanded calls such as `max, args`, and projects `Object...`
  elements through the erased-object carrier before packing. Inherited
  `getContext().addMessage(...)` trailing constructor calls now lower through
  flattened `__java_self.context` and use the same varargs packing path.
  Flattened constructor field initialization also clones a future-read
  parameter when a later trailing statement boxes the same value into erased
  varargs. In
  `.tmp/corpus-fastqc-exception-subset-after-trailing-field-future-read-clone`,
  the focused exception subset remains 9/9 translated with 0 invalid-pattern
  files and generated cargo errors drop from 30 to 26. The generated
  `MaxCountExceededException` no longer contains `__java_self.get_context()`,
  `args.as_slice()`, `vec![max, args]`, or a moved-value error for `max`.
- Java `keySet().size()` now retargets through the native collection length
  path, so rendered map key views use `.len() as i32` instead of leaking Java
  `.size()` onto a Rust `Vec`. JDK static constant support also covers
  `Locale.US` and emits the short-owner `Locale::US` surface when the canonical
  support owner is `JavaUtilLocale`. In
  `.tmp/corpus-fastqc-exception-subset-after-keyset-size-locale-us`, the
  focused exception subset remains 9/9 translated with 0 invalid-pattern files
  and generated cargo errors drop from 26 to 24.
- Slot-aware `keySet()` lowering now uses the target collection slot: a Java
  `Set<T>` return/local/assignment collects map keys into
  `std::collections::HashSet<_>`, while `keySet().toArray(...)` keeps the
  vector pipeline. In
  `.tmp/corpus-fastqc-exception-subset-after-keyset-set-slot`, the focused
  exception subset remains 9/9 translated with 0 invalid-pattern files and
  generated cargo errors drop from 24 to 23.
- Same-package utility receivers such as `ArgUtils.flatten(...)` now render as
  static support-owner calls when the simple receiver is unshadowed and has a
  utility-type shape. Shadowed locals still stay instance receivers. In
  `.tmp/corpus-fastqc-exception-subset-after-static-utils-owner`, the focused
  exception subset remains 9/9 translated with 0 invalid-pattern files and
  generated cargo errors drop from 23 to 22.
- Stable JDK `ObjectInputStream.readInt()` facts now produce a typed
  `Result<i32, JavaException>` facade method, so generated calls use
  `read_int()?` instead of a unit placeholder. In
  `.tmp/corpus-fastqc-exception-subset-after-readint-facade`, the focused
  exception subset remains 9/9 translated with 0 invalid-pattern files and
  generated cargo errors drop from 22 to 19. A typed `readObject()` facade is
  deferred: the first attempt reduced one error but introduced an invalid
  downcast when `(Object) in.readObject()` flowed into an `Object[]` element.
- Source interface collection storage and indexed access now use the shared
  trait-object carrier path. `List<Localizable>` elements render as
  `Vec<Box<dyn Localizable>>`, `add(pattern)` lowers through
  `pattern.clone_box()`, generated source traits include an `as_any`/`clone_box`
  object ABI, and `List<Localizable>.get(i)` clones the boxed trait object
  instead of boxing `&Box<dyn Localizable>`. In
  `.tmp/corpus-fastqc-exception-subset-after-indexed-localizable-box`, the
  focused exception subset remains 9/9 translated with 0 invalid-pattern files
  and generated cargo errors drop from 19 to 15. The old `Localizable.as_any`
  missing-method rows and `&Box<dyn Localizable>: Localizable` rows are gone.
  The remaining top families are `ExceptionContext` clone/default ownership,
  erased `Object`/varargs clone-display-serialization, deferred
  `ObjectInputStream.readObject()` carrier typing, one `String: Borrow<&String>`
  mismatch, one unit-to-`Localizable` coercion, and one lifetime row.
- Stable JDK object stream facts now cover `ObjectInputStream.readObject()`,
  `ObjectOutputStream.writeInt(int)`, and
  `ObjectOutputStream.writeObject(Object)` as fallible facade/handoff methods.
  External `Object` returns are projected to the same erased-object carrier used
  by slots, and erased-object values assigned into Java `Object` slots no longer
  downcast to the placeholder `Object` facade. In
  `.tmp/corpus-fastqc-exception-subset-after-object-stream-erased-identity`, the
  focused exception subset remains 9/9 translated with 0 invalid-pattern files
  and generated cargo errors drop from 14 to 12. The remaining top families are
  erased `Vec<Box<dyn Any>>` clone/display, `Box<dyn Any>` clone, typed
  deserialization casts from the erased carrier to `Throwable`/`String`/source
  interfaces, one `String: Borrow<&String>` mismatch, and one lifetime row.
- Nullable borrowed external arguments are now opt-in for explicit handoff APIs
  instead of applying to every nullable external parameter. This keeps
  `ObjectOutputStream.writeObject(...)` on the borrowed handoff path while
  returning nullable fluent/string/path arguments to owned `Option<T>` carriers,
  avoiding temporary `Option<&str>` values. `StringBuilder.append` and
  `StringBuffer.append` are modeled as definitely non-null borrowed string
  arguments. Focused evidence in
  `.tmp/corpus-fastqc-exception-subset-after-nullable-borrow-policy` remains
  9/9 translated with 0 invalid-pattern files and 6 underlying generated Rust
  errors.
- Enhanced-for iteration over `Map.keySet()` now gives the loop variable a
  borrowed key shape, and native `Map.get` avoids adding a second borrow when
  the key is already a borrowed loop binding. Focused evidence in
  `.tmp/corpus-fastqc-exception-subset-after-keyset-mapget-borrow-fix` remains
  9/9 translated with 0 invalid-pattern files; generated cargo errors drop from
  6 underlying errors to 5. The former `String: Borrow<&String>` row is gone.
  Remaining families are typed deserialization recovery from erased
  `ObjectInputStream.readObject()` to `Throwable`/`String`/source interfaces,
  non-cloneable erased `Vec<Box<dyn Any>>` storage recovery, and one borrowed
  source-field lifetime row.
- Erased-object sequence clone recovery now recognizes nested sequence shapes
  such as `List<Object[]>` and `Object[][]`, and `for`-loop body lowering uses
  per-body-statement read counts instead of inheriting the enclosing loop's
  whole-body counts. Focused evidence in
  `.tmp/corpus-fastqc-exception-subset-after-for-body-erased-sequence-move`
  remains 9/9 translated with 0 invalid-pattern files; generated cargo errors
  drop from 6 reported rows to 5 reported rows, with 4 underlying Rust errors.
  `ExceptionContext.deSerializeMessages` now emits
  `self.msg_arguments.push(args);` instead of cloning
  `Vec<Box<dyn Any>>`. Remaining families are typed deserialization recovery
  from erased `ObjectInputStream.readObject()` to `Throwable`/`String`/source
  interfaces and one borrowed source-field lifetime row.
- Typed object-stream serialization/deserialization recovery now covers the
  remaining 9-file FastQC exception subset. `ObjectOutputStream.writeObject`
  preserves generic source argument shapes while wrapping non-null values in
  `Some(...)`, indexed erased-object values use `java_clone_erased_object_ref`
  instead of moving out of arrays/vectors, source fields store owned Java object
  state instead of borrowed constructor parameters, optional owned field getters
  return borrowed optional views with `.as_ref()`, and `readObject()` typed
  runtime downcasts use a deserialization-specific mismatch message rather than
  the generic invalid-cast marker. Focused evidence in
  `.tmp/corpus-fastqc-exception-subset-after-deserialization-message-rewrite` is
  clean: 9/9 translated, 0 invalid-pattern files, `cargo check` ok, and 0
  generated Rust errors.
- Shared-support promotion now treats Rust lifetimes such as `Formatter<'_>` as
  lifetimes rather than character literals while scanning brace depth. This
  prevents promoted trait impls from being truncated before the outer closing
  brace; the regression is
  `shared_support_merger_preserves_display_impl_with_formatter_lifetime`.
- Abstract Java classes that declare an interface but do not provide a complete
  concrete implementation no longer emit placeholder-filled Rust trait impls.
  In `.tmp/corpus-fastqc-abstract-qcmodule-after-abstract-trait-skip2`, the
  focused FastQC `AbstractQCModule` subset translates 8/8 files and drops from
  10 compile-error placeholders to 0 invalid-pattern rows.
- Lowercase JDK static-owner aliases now recover through canonical JDK facade
  facts without restoring broad simple-name static fallback. `math`,
  `double`, `float`, `integer`, and `system` can resolve to `Math`, `Double`,
  `Float`, `Integer`, and `System` only when a known static method/arity proves
  the owner. The generated support builder now emits matching static JDK
  surfaces such as `Math::pow`, `Math::sqrt`, `Double::is_infinite`, and
  `System::arraycopy`, while avoiding duplicate weak instance stubs for those
  owners. Verified with
  `lowercase_jdk_static_owner_aliases_recover_canonical_static_calls`,
  `call_plan_recovers_normalized_static_utility_owner_receivers`,
  `jdk_numeric_and_decimal_format_facades_keep_precise_return_shapes`,
  `cargo fmt --check`, `cargo check --quiet`,
  `cargo test --test idiom_gaps --quiet` with 351 passing tests, and
  `cargo build --release --quiet --bin new-java2rust`.
- Primitive array allocation lengths now cast compound expressions to `usize`
  as a whole expression. This fixes FastQC-style `new float[data.length -
  offset]`, which previously rendered as mixed `i32 - usize` inside the Rust
  `vec![default; len]` repeat count. Verified with
  `primitive_array_allocation_casts_compound_length_as_whole_expression`,
  `documents_primitive_array_allocation_gap`, `cargo fmt --check`,
  `cargo check --quiet`, `cargo test --test idiom_gaps --quiet` with 352
  passing tests, `cargo build --release --quiet --bin new-java2rust`, and
  focused FastQC statistics rerun
  `.tmp/corpus-fastqc-statistics-after-array-length-cast`.
- JDK `File` instance facade support now covers `getName`, `getParent`,
  `getAbsoluteFile`, `isDirectory`, and `mkdir`, and the focused wrapper's
  hardcoded shared support block carries the same methods. Verified with
  `jdk_file_common_instance_facades_compile`, `cargo check --quiet`, and
  focused FastQC AbstractQCModule rerun
  `.tmp/corpus-fastqc-abstract-qcmodule-after-file-facades-r3`; the previous
  missing `File.get_*`/`File.mkdir` rows are gone from that subset.
- Java collection `get(...)[index]` now unwraps the outer collection lookup
  before applying Java array indexing. This covers FastQC-style
  `group.get(0)[1]` for `ArrayList<int[]>`/`ArrayList<String[]>` and avoids
  direct indexing into `Option<&Vec<_>>`. Verified with
  `collection_get_result_unwraps_before_array_indexing`,
  `erased_object_sequence_push_moves_when_no_later_read`, `cargo fmt --check`,
  `cargo check --quiet`, `cargo test --test idiom_gaps --quiet` with 354
  passing tests, `cargo build --release --quiet --bin new-java2rust`, and
  focused FastQC AbstractQCModule rerun
  `.tmp/corpus-fastqc-abstract-qcmodule-after-collection-get-index`.
- Java `String.getBytes()` and `String.indexOf(...)` now lower through native
  string plans instead of fallback Rust method names. `getBytes()` emits a
  Java-byte `Vec<i8>`, and `indexOf` uses generated runtime helpers with
  Java's `-1` not-found result. Verified with
  `java_string_get_bytes_and_index_of_use_native_helpers`,
  `cargo fmt --check`, `cargo check --quiet`,
  `cargo test --test idiom_gaps --quiet` with 355 passing tests,
  `cargo build --release --quiet --bin new-java2rust`, and focused FastQC
  AbstractQCModule rerun
  `.tmp/corpus-fastqc-abstract-qcmodule-after-string-facades-r2`. The focused
  wrapper's hardcoded string helper block is synchronized with the translator
  helper block.
- Receiverless Java `getClass().getResourceAsStream(...)` now lowers through
  the current source type's `JavaClass` facade instead of an unresolved free
  `get_class()` call. JDK AWT image/graphics support now covers
  `BufferedImage.TYPE_INT_RGB`, `BufferedImage.createGraphics() -> Graphics`,
  `Graphics.dispose()`, and `JPanel.print(Graphics)` argument shape without
  erasing the local graphics slot. The focused wrapper records `BufferedImage`,
  `Graphics`, and core `InputStream` support as shared support types to avoid
  duplicate module re-exports. Verified with
  `receiverless_get_class_resource_stream_uses_current_receiver`,
  `buffered_image_graphics_facades_are_typed`,
  `buffered_image_graphics_passed_to_jpanel_print_stays_typed`,
  `buffered_image_graphics_from_source_panel_getter_stays_typed`,
  `buffered_image_graphics_in_fallible_panel_workflow_stays_typed`,
  `cargo fmt --check`, `cargo check --quiet`,
  `cargo test --test idiom_gaps --quiet` with 360 passing tests,
  `cargo test --test batch_translation --quiet`, parser/type-coercion/
  idiom-sweep tests, `bash -n scripts/translate-focused-package.sh`,
  `cargo build --release --quiet --bin new-java2rust`, and focused FastQC
  AbstractQCModule rerun
  `.tmp/corpus-fastqc-abstract-qcmodule-after-getclass-awt-facades-r7`.
- JDK `InputStream.read(byte[])` argument planning now uses a shared mutable
  borrowed byte-slice slot, and generated support exposes concrete
  `InputStream::read`/`close` methods in both the translator support block and
  the focused wrapper. Conventional external `VERSION` static constants are
  also typed as string constants, avoiding invalid string-slot downcasts for
  imported owners such as `FastQCApplication.VERSION`. Verified with
  `resource_input_stream_read_and_close_have_typed_facade_support`,
  `nullable_receiver_external_argument_facts_shape_borrowed_stream_read_args`,
  `platform_mutable_external_argument_marks_local_binding_mutable`,
  `conventional_external_version_constant_append_does_not_downcast`,
  `cargo fmt --check`, `cargo check --quiet`,
  `cargo test --test idiom_gaps --quiet` with 361 passing tests,
  `cargo build --release --quiet --bin new-java2rust`, and focused FastQC
  AbstractQCModule rerun
  `.tmp/corpus-fastqc-abstract-qcmodule-after-inputstream-resource-facade-r3`.
- Non-static inner class outer-field capture now has an explicit generated
  representation point. Inner classes with resolved `OuterThis` capture facts
  receive a synthetic `__outer: Rc<RefCell<Outer>>` field, proven bare outer
  field reads lower through `self.__outer.borrow().field`, and that carrier is
  allowed to derive `Clone`. Verified with
  `non_static_inner_class_outer_fields_lower_through_outer_capture_handle`,
  `cargo test --test capture_facts --quiet`, `cargo fmt --check`,
  `cargo check --quiet`, `cargo test --test idiom_gaps --quiet` with 362
  passing tests, `cargo build --release --quiet --bin new-java2rust`, and
  focused FastQC AbstractQCModule rerun
  `.tmp/corpus-fastqc-abstract-qcmodule-after-inner-outer-capture-r2`.
- Source structs with dyn-trait container fields can derive `Clone` when a
  generated trait-object clone adapter is available. This removes the focused
  `HTMLReportArchive` clone failure from the AbstractQCModule subset.
- Local Java array declarators now preserve dimensions written between the type
  and variable name while ignoring `[]` markers inside generic arguments. The
  FastQC-style `byte [] array = new byte[128]` case now emits `Vec<i8>`.
- Java `String(byte[], int, int)` construction lowers through
  `java_string_from_byte_range(&bytes, offset, len)`, and the focused wrapper
  imports the same helper.
- JDK IO byte-range argument facts now cover common input/output stream
  receivers. `OutputStream.write(byte[], int, int)` lowers reused byte buffers
  as borrowed slices, and JDK `read`/`write` overload naming keeps one-argument
  calls as `read`/`write` while suffixing multi-argument overloads as
  `read_3`/`write_3`. The current no-op `read_3` support uses a shared borrowed
  slice; true mutating IO semantics should become explicit `FacadeAbiFact`
  rows with `&mut [i8]` and pre-borrow temporaries for values such as
  `buffer.length`.
- Omitted context receiver methods used directly in display/string sinks now
  get a fallback `String` support return hint keyed by receiver type, method,
  and arity. The hint is skipped when JDK/external-pack facts already provide a
  concrete return type, so display use does not override precise facts such as
  `TableModel.getValueAt(...) -> Object`.
- Java logging `Level` canonicalization is now stable through focused-package
  shared support assembly. The merger adds missing builtin constants for a
  promoted root `Level` facade, removing the `Level::SEVERE` and
  `Level::WARNING` regressions from JAligner.
- `ClassLoader.getResourceAsStream(...)` and related `Class`/`ClassLoader`
  resource methods now publish option-returning JDK facade ABI facts. Chained
  facade receiver projection and optional assignment wrapping keep direct
  optional return values from being wrapped as `Some(Option<InputStream>)`.
  A narrow syntax-level fallback remains as migration debt until every chained
  facade assignment path can prove the optional shape directly from ABI facts.
- JDK `InputStream` subclass upcasts now use external conversion adapter facts
  and shared slot conversion, including optional target slots. Focused-package
  shared support also preserves builtin `impl From<SubStream> for InputStream`
  edges when `InputStream` was already pre-populated as shared support before
  the merger ran.

## Corpus Results

| Corpus | Output | Result |
|---|---|---|
| `picard-programgroups` | `.tmp/corpus-batch-current-picard-programgroups-after-index-slot` | clean: 11/11 translated, 0 invalid patterns, `cargo check` ok |
| `htsjdk-fastq` | `.tmp/corpus-batch-current-htsjdk-fastq-after-optional-copy-samtag` | clean: 8/8 translated, 0 invalid patterns, offline `cargo check` ok |
| `gatk-genomicsdb` | `.tmp/corpus-batch-current-gatk-genomicsdb` | 6/6 translated, 0 invalid patterns, `cargo check` fails with 3 generated Rust errors |
| `gatk-disq-spark-datasources` | `.tmp/corpus-batch-current-gatk-disq-spark-datasources` | 10/10 translated, 4 invalid-pattern files, `cargo check` fails with 21 generated Rust errors |
| `htsjdk-vcf` | `.tmp/corpus-batch-current-htsjdk-vcf-after-derived-trait-dedupe` | 32/32 translated, 14 invalid-pattern files, no unclosed delimiter, and no duplicate `PartialEq`/`Eq`/`Hash` impl conflicts. `cargo check` now reaches generated Rust and fails with 607 errors; top families are mismatched types, `HashSet<VCFHeaderLine>` trait bounds, enum/version facade methods, string indexing by `i32`, concrete `instanceof`, VCF constants, and remaining `super(...)` placeholders. |
| `trimmomatic` | `.tmp/corpus-batch-current-trimmomatic-after-ndarray-shim` | 68/68 translated, 18 invalid-pattern files, offline `cargo check` now reaches generated Rust and fails with 856 errors; top families include mismatched types, keyword identifier `in`, missing `File`/`Logger`/string facades, array index coercions, and concrete `None` assignment |
| `biojava-aaprop` | `.tmp/corpus-batch-current-biojava-aaprop-after-ndarray-shim` | 33/33 translated, 3 invalid-pattern files, offline `cargo check` now reaches generated Rust and fails with 740 errors; top families include mismatched types, switch scaffolding, module path/name assembly for amino-acid constants, `.put` calls, and string char APIs |
| `biojava-alignment` | `.tmp/corpus-batch-current-biojava-alignment-after-ndarray-shim` | 53/53 translated, 21 invalid-pattern files, offline `cargo check` now reaches generated Rust and fails with 188 errors; top families include trait/interface assembly, `super(...)`, downcasts, array creation placeholders, `instanceof` shape gaps, and concrete `None` assignment |
| `jaligner` | `.tmp/corpus-batch-current-jaligner-after-ndarray-shim` | 29/29 translated, 2 invalid-pattern files, offline `cargo check` now reaches generated Rust and fails with 629 errors; top families include mismatched types, integer/index coercions, missing Java string/file/logging facades, and switch scaffolding |
| `varscan` | `.tmp/corpus-batch-current-varscan-after-loop-owned-elements` | clean: 18/18 translated, 0 invalid patterns, offline `cargo check` ok |
| `jahmm` | `.tmp/corpus-batch-current-jahmm-after-ndarray-shim` | 102/102 translated, 9 invalid-pattern files, offline `cargo check` now reaches generated Rust and fails with 120 errors; top families include missing test/assertion facades, trait/interface assembly for `Opdf`, `super(...)`, array creation placeholders, and collection spelling |
| `fastqc` | `.tmp/corpus-batch-current-fastqc-after-object-stream-field-fixes` | 156/156 translated, 0 failed files, no translator panic. `cargo check` reaches generated Rust and fails with 2733 errors; invalid scan reports 20 invalid-pattern files: 43 `compile_error!` placeholders across 13 files, 38 Java cast downcasts across 13 files, 0 `.put` calls, and 1 concrete `None` assignment. The earlier exception-object-stream subset fixes carry into the full run; the remaining top compile families are mismatched types, optional array indexing/length, missing local/static facade owners (`sb`, `double`, `math`, `system`, `float`), GUI erased-object calls, and constant/facade gaps. |
| `fastqc` partial rerun | `.tmp/corpus-batch-current-fastqc-after-keyword-cast-literals` | translation and crate assembly completed: 156/156 translated, 0 failed files, no translator panic. Local scan still reports 30 invalid-pattern files with 58 `compile_error!` placeholders across 24 files, 40 Java cast downcasts across 14 files, 0 `.put` calls, and 1 concrete `None` assignment. Targeted scans show `in::read` and raw `as i32 <` blockers are gone. The wrapper was interrupted after crate assembly during/after cargo reporting, so there is no final `summary.tsv`; `compile.log` contains 80 reported errors headed by the Java block-comment condition `if /*!encode &&*/`, then duplicate re-export names. The block-comment condition, Java hex floating literals, and `0x8000000000000000L` long masks now have local/focused fixes after this partial rerun and need a fresh FastQC rerun for corpus proof. |
| `fastqc` fresh parser rerun | `.tmp/corpus-batch-current-fastqc-after-comment-hex-literals` | translation completed: 156/156 translated, 0 failed files, no translator panic. The wrapper hung before final summary/compile reporting, but translation artifacts are complete. Targeted generated-source scans show 0 `/*!` block-comment condition artifacts, 0 Java hex floating literal suffixes, 0 Java hex long suffix masks, 0 `in::read` static-owner renderings, and 0 raw `as i32 <` cast-comparison renderings. Invalid-pattern counts remain 30 files / 99 rows: 58 `compile_error!` placeholders across 24 files, 40 Java cast downcasts across 14 files, 0 `.put` calls. The largest placeholder family is now `super(...) requires inheritance lowering` with 17 occurrences before the stream-wrapper super slice. |
| `fastqc` statistics subset | `.tmp/corpus-fastqc-statistics-after-lowercase-jdk-static-support` | 2/2 translated, 0 failed files, 0 invalid-pattern files. Offline `cargo check` reaches generated Rust and fails with 7 compile-error rows, headed by mismatched types and `usize`/`i32` arithmetic. Targeted generated-source scans show `Math::sqrt`/`Math::pow` calls and emitted `pub struct Math` support with `pow`/`sqrt`; no lowercase `math.` rows remain in this subset. |
| `fastqc` statistics subset | `.tmp/corpus-fastqc-statistics-after-array-length-cast` | clean: 2/2 translated, 0 failed files, 0 invalid-pattern files, offline `cargo check` ok, and 0 top compile-error rows. This proves the lowercase JDK owner support and compound array-length cast fixes together clear the current FastQC statistics subset. |
| `fastqc` AbstractQCModule subset | `.tmp/corpus-fastqc-abstract-qcmodule-after-file-facades-r3` | 8/8 translated, 0 failed files, 0 invalid-pattern files, offline `cargo check` reaches generated Rust and fails with 198 reported errors. The JDK `File` instance facade rows are gone. The remaining top families are non-static inner-class outer-field capture (`sb`, `c_width`, `c_height`, `g`), optional `Vec` indexing through `Option<&Vec<_>>`, small JDK/GUI facade gaps (`String.getBytes`, `String.indexOf`, `BufferedImage.TYPE_INT_RGB`, `Graphics.dispose`, `getClass`), and source/facade owner assembly gaps. |
| `fastqc` AbstractQCModule subset | `.tmp/corpus-fastqc-abstract-qcmodule-after-string-facades-r2` | 8/8 translated, 0 failed files, 0 invalid-pattern files, offline `cargo check` reaches generated Rust and fails with 190 reported errors. The optional `Option<&Vec<_>>` indexing rows and the `String.get_bytes`/`String.index_of` rows are gone. The remaining top families are non-static inner-class outer-field capture (`sb`, `c_width`, `c_height`, `g`), mismatched types, GUI/graphics facade gaps (`Graphics.dispose`, `BufferedImage.TYPE_INT_RGB`, `getClass`), source/facade owner assembly gaps, and private source-field access. |
| `fastqc` AbstractQCModule subset | `.tmp/corpus-fastqc-abstract-qcmodule-after-getclass-awt-facades-r7` | 8/8 translated, 0 failed files, 0 invalid-pattern files, offline `cargo check` reaches generated Rust and fails with 186 reported errors. The previous free `get_class`, missing `BufferedImage::TYPE_INT_RGB`, erased `Graphics.dispose`, and invalid `Graphics` downcast rows are gone. New exposed rows include `InputStream.read`/`close`; the dominant remaining family is still non-static inner-class outer-field capture (`sb`, `c_width`, `c_height`, `g`). |
| `fastqc` AbstractQCModule subset | `.tmp/corpus-fastqc-abstract-qcmodule-after-inputstream-resource-facade-r3` | 8/8 translated, 0 failed files, 0 invalid-pattern files, offline `cargo check` reaches generated Rust and fails with 174 reported errors. The `InputStream.read`/`close` rows and `FastQCApplication.VERSION` invalid downcast are gone. The dominant remaining family is non-static inner-class outer-field capture (`sb`, `c_width`, `c_height`, `g`), followed by mismatched types, type annotations, void-display expressions, private `FastQCConfig` fields, and remaining static/source support gaps. |
| `fastqc` AbstractQCModule subset | `.tmp/corpus-fastqc-abstract-qcmodule-after-inner-outer-capture-r2` | 8/8 translated, 0 failed files, 0 invalid-pattern files, offline `cargo check` reaches generated Rust and fails with 154 reported errors. Bare unresolved non-static inner outer-field rows for `sb`, `c_width`, `c_height`, and `g` are gone; `SVGGenerator.SVGGraphics` now has an explicit `__outer` capture carrier. The remaining top families are mismatched `StringBuffer.append(...)` argument slots, optional outer `g` facade calls, type annotations, void-display expressions, private `FastQCConfig` fields, and remaining static/source support gaps. |
| `fastqc` AbstractQCModule subset | `.tmp/corpus-fastqc-abstract-qcmodule-after-display-append-r1` | 8/8 translated, 0 failed files, 0 invalid-pattern files, offline `cargo check` reaches generated Rust and fails with 63 reported errors. `StringBuilder`/`StringBuffer.append(...)` support now accepts displayable Java append overload values, so the outer-capture append mismatches mostly disappear. Remaining top families are nullable/static slot drift, source/facade methods returning `()`, optional outer `g` facade calls, private `FastQCConfig` fields, and remaining source/static support gaps. |
| `fastqc` AbstractQCModule subset | `.tmp/corpus-fastqc-abstract-qcmodule-after-nullable-constructor-slot-r1` | 8/8 translated, 0 failed files, 0 invalid-pattern files, offline `cargo check` reaches generated Rust and fails with 62 reported errors. The constructor field initializer no longer emits `line_separator: Some(System::get_property(...))`; it now preserves the nullable `Option<String>` facade result directly. Remaining top families are 23 mismatched types, 9 void-display rows, optional outer `g` facade calls, private `FastQCConfig` fields, missing source/facade support members, and static/source symbol gaps. |
| `fastqc` AbstractQCModule subset | `.tmp/corpus-fastqc-abstract-qcmodule-after-constructor-local-graphics-r4` | 8/8 translated, 0 failed files, 0 invalid-pattern files, offline `cargo check` reaches generated Rust and fails with 55 reported errors. The constructor-local `b` row is gone, `BufferedImage.getGraphics()` has focused-wrapper support, and `SVGGenerator.g` now stores `Option<Graphics>` with `__java_self.g = Some(b.get_graphics())` instead of erased `Any`. Remaining top families are 21 mismatched types, 10 void-display/display rows, 4 type-annotation rows, `ZipFile.as_ref`, private `FastQCConfig` fields, missing source/static support members, and constructor/inner-class `self` misuse rows. |
| `fastqc` AbstractQCModule subset | `.tmp/corpus-fastqc-abstract-qcmodule-after-gui-erased-display-r2` | 8/8 translated, 0 failed files, 0 invalid-pattern files, offline `cargo check` reaches generated Rust and fails with 39 reported errors. AWT/Swing GUI getters now have precise primitive/string/object return shapes, and erased `TableModel.getValueAt(...)` display uses lower through `java_erased_object_to_string(...)`; the invalid Java cast scanner reports 0 files. Remaining top families are 12 mismatched types, 4 type-annotation rows, 3 `ZipFile.as_ref` rows, 3 `()` display rows, 3 constructor/inner-class `self` misuse rows, private `FastQCConfig` fields, missing `SVGImageSaver` pattern constants, and path/facade support drift. |
| `fastqc` AbstractQCModule subset | `.tmp/corpus-fastqc-abstract-qcmodule-after-source-borrowed-getter-r1` | 8/8 translated, 0 failed files, 0 invalid-pattern files, offline `cargo check` reaches generated Rust and fails with 34 reported errors. Source receiver call lowering now prefers emitted `SignaturePlan` return ABI for primary source methods, while summary-only owners keep source-summary nullability first. This removes the previous `Some(report.xhtml_stream())`, `Some(report.data_document())`, and `let zip: ZipOutputStream = report.zip_file();` mismatches; generated code now uses `.cloned()` or `.clone()` with clone-audit comments. Remaining top families are 7 mismatched types, 4 type-annotation rows, 3 `ZipFile.as_ref` rows, 3 `()` display rows, 3 constructor/inner-class `self` misuse rows, private `FastQCConfig` fields, missing `SVGImageSaver` pattern constants, and path/facade support drift. |
| `fastqc` AbstractQCModule subset | `.tmp/corpus-fastqc-abstract-qcmodule-after-zipfile-facade-r1` | 8/8 translated, 0 failed files, 0 invalid-pattern files, offline `cargo check` reaches generated Rust and fails with 34 reported errors. Bare local receiver lookup now respects Java shadowing when a local and field share a name, so the previous `zip_file.as_ref().unwrap()` rows are gone. JDK `ZipFile.entries()`, `ZipFile.getInputStream(...)`, and `ZipFile.close()` now have typed fallible platform facts/support; generated code uses `zip_file.entries()?`, `zip_file.get_input_stream(...)?`, and `zip_file.close()?`. Remaining top families are 9 mismatched types, 4 type-annotation rows, 6 total `()` display/display-adjacent rows, 3 constructor/inner-class `self` misuse rows, private `FastQCConfig` fields, missing `SVGImageSaver` pattern constants, and source/facade support drift. |
| `fastqc` AbstractQCModule subset | `.tmp/corpus-fastqc-abstract-qcmodule-after-constructor-static-owner-r1` | 8/8 translated, 0 failed files, 0 invalid-pattern files, offline `cargo check` reaches generated Rust and fails with 32 reported errors. Constructor field assignments whose RHS reads another receiverless instance field now defer until `__java_self` exists, so the previous associated-constructor `self` rows are gone. Parser/planner static-owner cleanup preserves UpperCamel field-access receivers such as `Color.BLACK` and `Font.PLAIN` even when same-named fields `color`/`font` exist; JDK typed constants now lower as `Color::BLACK` and `Font::PLAIN`. Remaining top families are 9 mismatched types, 4 type-annotation rows, 6 total `()` display/display-adjacent rows, two `HTMLReportArchive` clone/optional field-recovery rows, private `FastQCConfig` fields, missing `SVGImageSaver` pattern constants, and source/facade support drift. |
| `fastqc` AbstractQCModule subset | `.tmp/corpus-fastqc-abstract-qcmodule-after-constructor-self-field-slot-r1` | 8/8 translated, 0 failed files, 0 invalid-pattern files, offline `cargo check` reaches generated Rust and fails with 31 reported errors. Deferred constructor field assignments now lower self-dependent RHS expressions with a temporary self receiver before replacing `self` with `__java_self`, so nested source-constructor arguments can recover owned fields as `__java_self.field.clone()` or optional fields as `__java_self.field.as_ref().expect(...).clone()` instead of cloning the whole partially built struct. The previous `HTMLReportArchive::new` whole-struct clone row is gone; the generated line now uses `__java_self.zip_file.clone()` for the still-unshaped JDK `FileOutputStream` constructor. Remaining top families are 9 mismatched types, 4 type-annotation rows, 6 total `()` display/display-adjacent rows, one `HTMLReportArchive` clone row from source argument passing, private `FastQCConfig` fields, missing `SVGImageSaver` pattern constants, and source/facade support drift. |
| `fastqc` AbstractQCModule subset | `.tmp/corpus-fastqc-abstract-qcmodule-after-fileoutputstream-overload-slots-r1` | 8/8 translated, 0 failed files, 0 invalid-pattern files, offline `cargo check` reaches generated Rust and fails with 31 reported errors. JDK constructor argument planning now uses actual argument/source types before old arity-only constructor facts, so `FileOutputStream(File)` recovers nullable `File` fields as `__java_self.zip_file.as_ref().expect(...).clone()` while `FileOutputStream(String)` remains a string/path slot. The remaining single `arguments to this function are incorrect` row is now the adjacent `ZipOutputStream`/`OutputStream` facade-inheritance slot family, not the `FileOutputStream(File/String)` overload slot. |
| `fastqc` AbstractQCModule subset | `.tmp/corpus-fastqc-abstract-qcmodule-after-zip-outputstream-adapter-r2` | 8/8 translated, 0 failed files, 0 invalid-pattern files, offline `cargo check` reaches generated Rust and fails with 30 reported errors. Java platform conversion adapter facts now cover `ZipOutputStream -> OutputStream`, support emits the matching `From` surface, and reused stream arguments clone before `.into()` rather than after it. The previous `SVGImageSaver.saveImage(..., zip)` OutputStream mismatch is gone; the remaining stream mismatch is `Base64OutputStream -> OutputStream`, which belongs with Commons Codec facade facts rather than core JDK. |
| `fastqc` AbstractQCModule subset | `.tmp/corpus-fastqc-abstract-qcmodule-after-iharder-base64-outputstream-adapter-r1` | 8/8 translated, 0 failed files, 0 invalid-pattern files, offline `cargo check` reaches generated Rust and fails with 29 reported errors. External-pack conversion facts now cover both Apache Commons Codec `Base64OutputStream -> OutputStream` and the actual FastQC iHarder shape `net.sourceforge.iharder.base64.Base64.OutputStream -> OutputStream`; generated `ImageToBase64` now emits `Base64OutputStream::new(os.clone()/* TODO(translation): validate added clone */).into()`. The remaining top families are 8 mismatched types, 4 type-annotation rows, 6 total `()` display/display-adjacent rows, private `FastQCConfig` fields, missing `SVGImageSaver` pattern constants, source/facade support drift, and source argument ownership rows. |
| `fastqc` AbstractQCModule subset | `.tmp/corpus-fastqc-abstract-qcmodule-after-public-field-visibility-r1` | 8/8 translated, 0 failed files, 0 invalid-pattern files, offline `cargo check` reaches generated Rust and fails with 27 reported errors. Java `public` instance fields now emit as public Rust struct fields, and borrowed receiver field reads clone owned values when needed. The previous `FastQCConfig.svg_output` and `FastQCConfig.line_separator` private-field errors are gone; generated `FastQCConfig` now has `pub line_separator: Option<String>` and `pub svg_output: bool`. Remaining top families are 8 mismatched types, 4 type-annotation rows, 6 total `()` display/display-adjacent rows, missing `SVGImageSaver` pattern constants, source/facade support drift, singleton/static field lowering, and source argument ownership rows. |
| `fastqc` AbstractQCModule subset | `.tmp/corpus-fastqc-abstract-qcmodule-after-static-final-pattern-r1` | 8/8 translated, 0 failed files, 0 invalid-pattern files, offline `cargo check` reaches generated Rust and fails with 25 reported errors. Source `static final Pattern` fields now emit associated default facade values instead of instance storage, and JDK regex facts provide `Pattern.matcher`/`Matcher` support. The previous missing `SVGImageSaver::LINE_PATTERN` and `SVGImageSaver::RECT_PATTERN` associated item errors are gone; generated `SVGImageSaver` now has `const LINE_PATTERN: Pattern = Pattern;` and `const RECT_PATTERN: Pattern = Pattern;`. Remaining top families are 8 mismatched types, 4 type-annotation rows, 6 total `()` display/display-adjacent rows, source/facade support drift, singleton/static field lowering, class-loader resource access, and source argument ownership rows. |
| `fastqc` AbstractQCModule subset | `.tmp/corpus-fastqc-abstract-qcmodule-after-late-static-signature-r1` | 8/8 translated, 0 failed files, 0 invalid-pattern files, offline `cargo check` reaches generated Rust and fails with 9 reported errors. Low-precedence Rust/JDK signature facts now cover the exposed `String`, IO, ZIP, XML writer, AWT image/RepaintManager, Base64 encoder, and ImageIO rows; late opaque support honors static facade metadata. The previous `ClassLoader`, `XMLOutputFactory` `()`, `Enumeration`/`ZipEntry` `()`, `BufferedInputStream.read(...)`, `Image.getWidth/Height`, RepaintManager generic/static, Base64 encoder, and ImageIO generic-return families are gone. Remaining families are abstract-interface method surface (`AbstractQCModule.getResultsPanel`), `JPanel -> Component` facade conversion, missing `Clone` for source report structs reused by value, byte-array local/String(byte[],int,int) typing, `SequenceFile.name()` returning `()`, and one output-buffer ownership row. |
| `fastqc` AbstractQCModule subset | `.tmp/corpus-fastqc-abstract-qcmodule-after-dyn-trait-clone-r1` | 8/8 translated, 0 failed files, 0 invalid-pattern files, offline `cargo check` reaches generated Rust and fails with 6 reported errors. Dyn-trait container `Clone` support removes the previous `HTMLReportArchive` clone failure. Remaining families are byte-array local/String range constructor typing, `SequenceFile.name()` returning `()`, and output-buffer ownership. |
| `fastqc` AbstractQCModule subset | `.tmp/corpus-fastqc-abstract-qcmodule-after-byte-array-string-r2` | 8/8 translated, 0 failed files, 0 invalid-pattern files, offline `cargo check` reaches generated Rust and fails with 3 reported errors. Local `byte [] array` declarations now emit `Vec<i8>`, generic argument arrays are not over-counted, and `new String(array, 0, nRead)` lowers through `java_string_from_byte_range(...)`. Remaining families are `SequenceFile.name()` returning `()` and output-buffer ownership. |
| `fastqc` AbstractQCModule subset | `.tmp/corpus-fastqc-abstract-qcmodule-after-io-borrow-slots-r1` | 8/8 translated, 0 failed files, 0 invalid-pattern files, offline `cargo check` reaches generated Rust and fails with 2 reported errors. JDK IO argument facts now borrow reused byte buffers for `write(byte[], int, int)`, so the output-buffer move row is gone. The remaining real compile error is `SequenceFile.name()` returning `()` because the primary-only subset omits the real `SequenceFile` interface; a broad full FastQC source-summary root proves the method returns `String`, but currently introduces duplicate/conflicting symbols into this focused generated crate. |
| `fastqc` AbstractQCModule subset | `.tmp/corpus-fastqc-abstract-qcmodule-after-display-context-return-r2` | clean: 8/8 translated, 0 failed files, 0 invalid-pattern files, offline `cargo check` ok, and 0 top compile-error rows. Display-context return hints upgrade the weak omitted `SequenceFile.name()` support stub to `String` without adding a corpus-specific method-name table and without overriding concrete external facts. |
| `fastqc` full corpus | `.tmp/corpus-batch-current-fastqc-after-display-context-return-r1` | 156/156 translated, 0 translation failures, 18 invalid-pattern files, and 2143 generated-Rust errors. This improves the previous full FastQC baseline from 20 invalid-pattern files and 2733 errors after the focused display-context return fix. Top compile families are optional `Vec`/array/`ndarray` indexing, weak GUI/AWT erased object returns, missing class/facade constants, array creation, switch scaffolding, and source object/constructor representation. Runtime remains dominated by signature propagation: `global_signature_ms=525467`, `signature_fixpoint_ms=516779`, `signature_propagation_sweep_ms=502527`, `total_elapsed_ms=924533`. |
| `fastqc` full corpus | `.tmp/corpus-batch-current-fastqc-after-fastq-samtag-r1` | 156/156 translated, 0 translation failures, 18 invalid-pattern files, and 1657 generated-Rust errors. This full rerun confirms FastQC no longer panics in signature solving after the source-summary/facade work. It was run before the parameter-declarator and optional receiver fixes below, so a refreshed full run is still needed. |
| `fastqc` full corpus | `.tmp/corpus-batch-current-fastqc-refresh-20260724-r1` | 156/156 translated, 0 translation failures, 28 invalid-pattern files, and 1427 generated-Rust errors. Timings: `total_elapsed_ms=1577967`, `translation_ms=1132882`, `global_analysis_ms=910028`, `signature_fixpoint_ms=659088`, `crate_assembly_ms=432084`, `cargo_check_ms=12135`. The latest top families are mismatched types, fallible `?` in non-`Result` methods, erased GUI/stream facades returning `Box<dyn Any>`, output-stream/Base64 shared-support identity drift, `File::new_1`, mutable receiver drift, missing source/scope fields, and remaining invalid downcast placeholders. A shared-support merger follow-up now prevents local `OutputStream = Base64OutputStream` aliases and source-owned nested Base64 impls from overriding shared JDK stream facades, while adding output-stream upcast synthesis alongside input-stream upcasts. |
| `fastqc` math util subset | `.tmp/corpus-fastqc-math-util-after-param-array-and-refined-write-r2` | 10/10 translated, 0 translation failures, 1 invalid-pattern file, and 351 generated-Rust errors. The old `cannot index into f64` family from `final double result[]`-style parameters is gone, and the stale `unwrap().as_mut().expect(...)` / `unwrap().as_ref().expect(...)` optional sequence receiver family is gone in the current release binary. Remaining families are missing source/facade constants and helper types (`FastMath`/`FastMathCalc` tables, `LocalizedFormats`, `ExpIntTable`, `LnMant`), reserved identifier/raw-name leakage around `in`/`as` in generated contexts, local mutable binding gaps such as `reduce_results`, and math/facade surface gaps such as `BigDecimal`/`BigInteger` constants. |
| `fastqc` math util subset | `.tmp/corpus-fastqc-math-util-after-internal-mut-borrow-local-r1` | 10/10 translated, 0 translation failures, 1 invalid-pattern file, and 347 generated-Rust errors. Internal mutable-borrow source-call propagation marks caller locals such as `reduce_results` and `r#as` as `mut` before `.as_mut_slice()`, removing the exposed `cannot borrow ... as mutable` rows. |
| `fastqc` math util subset | `.tmp/corpus-fastqc-math-util-after-while-true-loop-r1` | 10/10 translated, 0 translation failures, 1 invalid-pattern file, and 341 generated-Rust errors. Literal `while (true)` loops now emit Rust `loop`, removing the `rem_a`/`rem_b` possibly-uninitialized rows. Remaining top families are still source/facade constants and helper types (`LocalizedFormats`, `FastMath`/`FastMathCalc` tables, `ExpIntTable`, `ExpFracTable`, `LnMant`), reserved identifier/raw-name leakage, math facade constants, and generated-Rust type mismatches. |
| `fastqc` math util subset | `.tmp/corpus-fastqc-math-util-after-static-array-consts-r3` | 10/10 translated, 0 translation failures, 1 invalid-pattern file, and 196 generated-Rust errors. Static-final primitive expression constants and literal array constants now emit associated constants; `double[][]` const tables use nested static slices for reads and `.len()` for `.length`, while ordinary mutable 2D arrays remain ndarray-backed. The former missing `FastMath`/`FastMathCalc` literal table rows, `.nrows()` on static slices, `.as_slice()` on static slices, and missing `FastMathCalc::FACT` rows are gone. Remaining top families are mismatched types, omitted/context dependency types such as `LocalizedFormats`, reserved identifier leakage around `in`, helper table classes (`ExpIntTable`, `ExpFracTable`, `LnMant`), `Float.isNaN` facade casing, fallible calls in non-`Result` methods, and math facade constants for `BigDecimal`/`BigInteger`. |
| `fastqc` math util subset | `.tmp/corpus-fastqc-math-util-after-float-isnan-r1` | 10/10 translated, 0 translation failures, 1 invalid-pattern file, and 189 generated-Rust errors. `Float.isNaN(...)` now shares the native floating-point static-call plan with `Double.isNaN(...)`, so generated Rust uses primitive `.is_nan()` and the previous missing `Float::is_na_n` support-method family is gone. Remaining top families are mismatched types, omitted/context dependency types such as `LocalizedFormats`, reserved identifier leakage around `in`, helper table classes (`ExpIntTable`, `ExpFracTable`, `LnMant`), fallible calls in non-`Result` methods, receiverless same-class static calls such as `round(...)`, and math facade constants for `BigDecimal`/`BigInteger`. |
| `fastqc` math util subset | `.tmp/corpus-fastqc-math-util-after-static-self-overload-r3` | 10/10 translated, 0 translation failures, 1 invalid-pattern file, and 187 generated-Rust errors. Receiverless same-class static overload calls now use current source signatures and partial known argument types to retarget to `Self::<overload>(...)`; exact primitive matches outrank widening matches when unknown arguments such as missing static constants cannot fully type the call. The previous free `round(...)` rows are gone; generated `Precision` calls now use `Self::round_double_int_int(...)` and `Self::round_float_int_int(...)`. Remaining top families are mismatched types, omitted/context dependency types such as `LocalizedFormats`, reserved identifier leakage around `in`, helper table classes (`ExpIntTable`, `ExpFracTable`, `LnMant`), fallible calls in non-`Result` methods, and math facade constants for `BigDecimal`/`BigInteger`. |
| `fastqc` math util subset | `.tmp/corpus-fastqc-math-util-after-wrapper-hashcode-r1` | 10/10 translated, 0 translation failures, 1 invalid-pattern file, and 159 generated-Rust errors. Typed JDK numeric facade facts/support now cover `BigDecimal` rounding constants, `BigInteger.ZERO`/`ONE`, `BigDecimal.setScale(...).doubleValue()`, boxed primitive constructor erasure, and boxed primitive `hashCode()`. The previous missing `BigDecimal::ROUND_*`, missing `BigInteger::ZERO`/`ONE`, `f32::new`/`f64::new`, `set_scale(...) -> ()`, and primitive `.hash_code()` rows are gone. Remaining top families are mismatched types, omitted/context dependency types such as `LocalizedFormats`, reserved identifier leakage around `in`, helper table classes (`ExpIntTable`, `ExpFracTable`, `LnMant`), fallible calls in non-`Result` methods, and remaining source/facade constants such as `MathUtils::TWO_PI` and `Precision::SGN_MASK_FLOAT`. |
| `fastqc` math util subset | `.tmp/corpus-fastqc-math-util-after-cross-source-consts-r1` | 10/10 translated, 0 translation failures, 1 invalid-pattern file, and 155 generated-Rust errors. Source `static final` constants can reference other source constants by owner, and Java hex int sign-bit literals preserve Java two's-complement `int` semantics. The previous missing `MathUtils::TWO_PI` and `Precision::SGN_MASK_FLOAT` rows are gone. Remaining top families are mismatched types, omitted/context dependency types such as `LocalizedFormats`, reserved identifier leakage around `in`, helper table classes (`ExpIntTable`, `ExpFracTable`, `LnMant`), fallible calls in non-`Result` methods, and scalar/type coercion rows. |
| `fastqc` math util subset | `.tmp/corpus-fastqc-math-util-after-nested-static-table-alias-r1` | 10/10 translated, 0 translation failures, 1 invalid-pattern file, and 146 generated-Rust errors. Nested static source helper classes now get simple-name aliases for flattened Rust type names, and `static final` primitive/String array fields without inline initializers emit conservative empty-slice associated constants for static-block-populated table reads. The previous missing `ExpIntTable`, `ExpFracTable`, and `LnMant` type rows are gone. Remaining top families are 56 mismatched types, 42 missing `LocalizedFormats`, 15 missing `r#in`, 10 missing `fast_math_calc`, 5 fallible calls in non-`Result` methods, plus smaller moved-value, ndarray/static-slice indexing, strict-math, keyword, and scalar coercion rows. |
| `fastqc` math util subset | `.tmp/corpus-fastqc-math-util-after-localized-formats-pack-r1` | 10/10 translated, 0 translation failures, 0 invalid-pattern files, and 107 generated-Rust errors. Apache Commons Math `LocalizedFormats` constants are now typed external-pack enum-variant fields, and enum variants are emitted from the used static-constant support path rather than duplicated by broad external field surfaces. The previous 42 missing `LocalizedFormats` rows and the two invalid Java cast downcast rows are gone. Remaining top families are 58 mismatched types, 15 missing `r#in`, 10 missing `fast_math_calc`, 5 fallible calls in non-`Result` methods, plus smaller moved-value, ndarray/static-slice indexing, mutable receiver, strict-math, keyword, and scalar coercion rows. |
| `fastqc` math util subset | `.tmp/corpus-fastqc-math-util-after-keyword-param-r1` | 10/10 translated, 0 translation failures, 0 invalid-pattern files, and 91 generated-Rust errors. Rust function parameter rendering now raw-ident escapes names after applying the existing mutable-parameter convention, so FastMathCalc emits `mut r#in` consistently with body references. The previous 15 missing `r#in` rows and invalid `mut in` parse row are gone. Remaining top families are 58 mismatched types, 10 missing `fast_math_calc`, 5 fallible calls in non-`Result` methods, plus smaller moved-value, ndarray/static-slice indexing, mutable receiver, `strict_math`, keyword, and scalar coercion rows. |
| `fastqc` math util subset | `.tmp/corpus-fastqc-math-util-after-static-owner-overload-r1` | 10/10 translated, 0 translation failures, 0 invalid-pattern files, and 85 generated-Rust errors. Same-source static-owner aliases are seeded from source type declarations and fallback overload names refine `StaticOwner` plans without degrading already-specific names. The previous unresolved lowercase `fast_math_calc` owner rows are gone, and 6 of the 10 `FastMathCalc` overload-name rows now use emitted suffixed overloads. Remaining top families are 58 mismatched types, 4 `FastMathCalc::printarray` table-constant overload rows, 5 fallible calls in non-`Result` methods, plus smaller moved-value, ndarray/static-slice indexing, mutable receiver, `strict_math`, keyword, and scalar coercion rows. |
| `fastqc` math util subset | `.tmp/corpus-fastqc-math-util-after-static-table-index-r1` | 10/10 translated, 0 translation failures, 0 invalid-pattern files, and 81 generated-Rust errors. Nested static table aliases/source field facts now guide `FastMathCalc.printarray` overload selection for 1D table constants, static associated array constants keep nested slice indexing instead of ndarray tuple indexing, and complex bitwise/shift index expressions cast the whole expression to `usize`. Remaining top families are 59 mismatched types, 5 fallible calls in non-`Result` methods, moved `hi_prec`, slice-to-`Vec` recovery for `LN_MANT`, `LN_MANT` 2D slice-vs-`Array2` printarray shape, mutable receiver, `strict_math`, scalar shift/coercion, and display/keyword support rows. |
| `fastqc` math util subset | `.tmp/corpus-fastqc-math-util-after-sequence-to-vec-r1` | 10/10 translated, 0 translation failures, 0 invalid-pattern files, and 80 generated-Rust errors. Shared ownership coercion now treats borrowed Java sequences recovered into owned Java sequences as `.to_vec()` instead of generic `.clone()`, so `LnMant::LN_MANT[index]` assigned to `Vec<f64>` compiles through the right Rust recovery shape. Remaining top families are 57 mismatched types, 5 fallible calls in non-`Result` methods, moved `hi_prec`/`x2`, `LN_MANT` 2D slice-vs-`Array2` printarray shape, source getters returning static slices as owned arrays, mutable receiver, `strict_math`, scalar shift/coercion, display support, duplicate `JavaPathArg`, and raw keyword support rows. |
| `fastqc` math util subset | `.tmp/corpus-fastqc-math-util-after-widen-compound-cast-r1` | 10/10 translated, 0 translation failures, 0 invalid-pattern files, and 81 generated-Rust errors. Primitive widening casts now parenthesize compound operands, removing the previous `i64 >> f64` shift-distance row in `reducePayneHanek`. The same Payne-Hanek expression then exposed the next boundary: additive/multiplicative integer subexpressions still needed whole-expression widening. |
| `fastqc` math util subset | `.tmp/corpus-fastqc-math-util-after-whole-widen-cast-r1` | 10/10 translated, 0 translation failures, 0 invalid-pattern files, and 80 generated-Rust errors. The cast parenthesizer now recognizes only balanced outer parentheses that wrap the whole rendered expression, so the Payne-Hanek integer sum lowers as `((((prod2_a & 4095i64) << 40) + (prod2_b >> 24)) as f64) / ...` instead of casting only the right child. The `i64 >> f64` and `f64 + i64` rows are gone; one `f64 * i64` scalar promotion row remains. |
| `fastqc` math util subset | `.tmp/corpus-fastqc-math-util-after-optional-array-numeric-promotion-r1` | 10/10 translated, 0 translation failures, 0 invalid-pattern files, and 79 generated-Rust errors. Optional array/index expression type inference now unwraps `Optional<Array<T>>` before deriving the indexed element type, so `hiPrec[0] * HEX_40000000` lowers as `hi_prec.as_ref().expect("Java null receiver")[0 as usize] * (Self::HEX_40000000 as f64)` instead of a late whole-expression assignment cast. The previous `f64 * i64` scalar promotion row is gone. Remaining top families are 57 mismatched types, 5 fallible calls in non-`Result` methods, 2 moved `hi_prec` rows, plus static slice/`Array2` shape drift, mutable receiver, missing `strict_math`, moved `x2`, type annotations, display support, duplicate `JavaPathArg`, and raw keyword support. |
| `fastqc` math util subset | `.tmp/corpus-fastqc-math-util-after-nullable-out-param-reborrow-r1` | 10/10 translated, 0 translation failures, 0 invalid-pattern files, and 72 generated-Rust errors. Fresh local arrays passed to nullable out-parameters now stay owned `Vec<T>` storage and wrap call arguments as `Some(values.as_mut_slice())`; nullable mutable out-parameters passed onward and reused now reborrow with `as_deref_mut()`. The previous `Option<&mut Vec<f64>>` to `Option<&mut [f64]>` mismatches and moved `hi_prec` rows are gone. Remaining top families are 52 mismatched types, 5 fallible calls in non-`Result` methods, static slice/`Array2` shape drift, mutable receiver, missing `strict_math`, moved `x2`, type annotations, display support, duplicate `JavaPathArg`, and raw keyword support. |
| `fastqc` math util subset | `.tmp/corpus-fastqc-math-util-after-numeric-equality-promotion-r1` | 10/10 translated, 0 translation failures, 0 invalid-pattern files, and 45 generated-Rust errors. Primitive numeric equality/inequality now goes through the shared numeric promotion planner, so raw comparisons such as `double == 0`, `double != 0`, `double == long`, and `float == double` lower with explicit casts before equality rendering. Mismatched-type rows drop from 52 to 25. Remaining top families are 25 mismatched types, 5 fallible calls in non-`Result` methods, static slice/`Array2` shape drift, mutable receiver, missing `strict_math`, moved `x2`, type annotations, display support, duplicate `JavaPathArg`, and raw keyword/support rows. |
| `fastqc` math util subset | `.tmp/corpus-fastqc-math-util-after-biginteger-method-facts-r1` | 10/10 translated, 0 translation failures, 0 invalid-pattern files, and 38 generated-Rust errors. Builtin numeric facade facts now type immutable `BigInteger.multiply(...)`, `pow(...)`, `shiftRight(...)`, and `testBit(...)`, so generated support returns `BigInteger`/`bool` instead of weak `()`. The previous BigInteger unit-return rows are gone. Remaining top families are 18 mismatched types, 5 fallible calls in non-`Result` methods, static slice/`Array2` shape drift, mutable receiver, missing `strict_math`, moved `x2`, type annotations, display support, duplicate `JavaPathArg`, and raw keyword/support rows. |
| `fastqc` math util subset | `.tmp/corpus-fastqc-math-util-after-chained-array-assign-r1` | 10/10 translated, 0 translation failures, 0 invalid-pattern files, and 41 generated-Rust errors. Plain array assignment expressions in value position now emit assignment blocks that return the assigned value, so chained writes such as `ys[0] = ys[1] = 0.0` no longer produce unit-to-`f64` mismatches. The latest count exposes later move rows for read-only source array parameters such as `xs` and `facts`, which still lower as owned `Option<Vec<T>>` instead of borrowed optional slices. Remaining top families are 15 mismatched types, 5 fallible calls in non-`Result` methods, 3 moved `xs` rows, 3 moved `facts` rows, static slice/`Array2` shape drift, mutable receiver, missing `strict_math`, moved `x2`, type annotations, display support, duplicate `JavaPathArg`, and raw keyword/support rows. |
| `fastqc` math util subset | `.tmp/corpus-fastqc-math-util-after-source-array-borrow-propagation-r1` | 10/10 translated, 0 translation failures, 0 invalid-pattern files, and 40 generated-Rust errors. Internal source-call propagation now stops caller-owned array storage from overriding callee body evidence that a parameter is borrowed, so the previous owned `Option<Vec<T>>` shape is gone for the minimized nullable read-only array case. This exposed read-only alias rows where caller mutable bindings still forced `Option<&mut Vec<f64>>` inputs in `split_mult`/`quad_mult`. |
| `fastqc` math util subset | `.tmp/corpus-fastqc-math-util-after-readonly-alias-borrow-propagation-r1` | 10/10 translated, 0 translation failures, 0 invalid-pattern files, and 37 generated-Rust errors. Caller mutable-binding requirements no longer override callee read-only parameters, so `split_mult` and `quad_mult` now take read-only inputs as `Option<&Vec<f64>>` while keeping output slots mutable. The previous `cannot borrow xs as mutable more than once` / `cannot borrow ys as mutable more than once` families are gone. Remaining top families are 16 mismatched types, 5 fallible calls in non-`Result` methods, 2 read/write overlap rows on `result`, static slice/`Array2` shape drift, mutable receiver, missing `strict_math`, type annotations, display support, duplicate `JavaPathArg`, and raw keyword/support rows. |
| `fastqc` math util subset | `.tmp/corpus-fastqc-math-util-after-same-call-readwrite-snapshot-r1` | 10/10 translated, 0 translation failures, 0 invalid-pattern files, and 35 generated-Rust errors. Shared-read arguments that alias a later mutable output argument in the same call now snapshot the read side with an audited clone, so `split_add(Some(&result.clone()...), ..., Some(&mut result))` avoids Rust `E0502` while preserving Java pre-call read behavior. The two `cannot borrow result as mutable because it is also borrowed as immutable` rows are gone. Remaining top families are 16 mismatched types, 5 fallible calls in non-`Result` methods, static slice/`Array2` shape drift, mutable receiver, missing `strict_math`, type annotations, display support, duplicate `JavaPathArg`, and raw keyword/support rows. |
| `fastqc` math util subset | `.tmp/corpus-fastqc-math-util-after-strictmath-static-facade-r1` | 10/10 translated, 0 translation failures, 0 invalid-pattern files, and 34 generated-Rust errors. JDK static-owner canonicalization and support now cover `StrictMath.IEEEremainder(double,double)`, so generated `FastMath::i_e_e_eremainder` calls `StrictMath::i_e_e_eremainder(...)` instead of unresolved `strict_math.i_e_e_eremainder(...)`. Remaining top families are 16 mismatched types, 5 fallible calls in non-`Result` methods, static slice/`Array2` shape drift, mutable receiver, type annotations, display support, duplicate `JavaPathArg`, and raw keyword/support rows. |
| `fastqc` math util subset | `.tmp/corpus-fastqc-math-util-after-javapatharg-shared-strip-r2` | 10/10 translated, 0 translation failures, 0 invalid-pattern files, and 33 generated-Rust errors. Focused crate assembly now records `JavaPathArg` as shared support and strips module-local impls for shared support traits, so the previous duplicate `JavaPathArg` root re-export and follow-on conflicting impl rows are gone. Remaining top families are 16 mismatched types, 5 fallible calls in non-`Result` methods, static slice/`Array2` shape drift, mutable receiver, type annotations, display support, and raw keyword/support rows. |
| `fastqc` math util subset | `.tmp/corpus-fastqc-math-util-after-static-clone-and-slice-owned-recovery-r1` | 10/10 translated, 0 translation failures, 0 invalid-pattern files, and 25 generated-Rust errors. Static-final 2D table constants now feed `ndarray::Array2` slots through `Array2::from_rows(...)`, static row reads flowing into owned Java arrays recover with `.to_vec()`, and `FastMathLiteralArrays.loadLnMant()` returns `ndarray::Array2::from_rows(Self::LN_MANT)` instead of cloning the static nested slice. Borrowed/nullable Java array parameters now normalize toward slice slots, and optional mutable arrays reborrow through `as_deref()`/`as_deref_mut()` rather than moving through `Option::map(...)`. Remaining top families are 10 mismatched types, 5 fallible calls in non-`Result` methods, iterator `&f64` values passed to `f64` helpers, `i64 -> i32` shift/coercion rows, `Vec<Box<dyn Any>>` display, one trait-object type spelling row, mutable receiver drift, and an `if` expression missing an `else`. |
| `fastqc` math util subset | `.tmp/corpus-fastqc-math-util-after-primitive-enhanced-for-copied-r1` | 10/10 translated, 0 translation failures, 0 invalid-pattern files, and 23 generated-Rust errors. Enhanced-for loops over primitive sequence elements now use `.iter().copied()` and an owned primitive loop-variable shape, so the previous `Self::format(d)` rows with `d: &f64` are gone without adding a FastQC or `format` special case. Remaining top families are 8 mismatched types, 5 fallible calls in non-`Result` methods, one trait-object type spelling row, mutable receiver drift, `Vec<Box<dyn Any>>` display, and an `if` expression missing an `else`. |
| `fastqc` math util subset | `.tmp/corpus-fastqc-math-util-after-overload-exception-key-r1` | 10/10 translated, 0 translation failures, 0 invalid-pattern files, and 20 generated-Rust errors. Source method effect facts now publish emitted overload-name aliases, and exception planning keys methods by emitted Rust identity with a Java-name fallback. The static `ResizableDoubleArray.copy(ResizableDoubleArray, ResizableDoubleArray)` helper now emits `-> Result<(), JavaException>` and its internal `check_not_null` calls compile through the helper shell. Remaining top families are 8 mismatched types, 4 fallible calls in non-`Result` methods, one trait-object type spelling row, mutable receiver drift, `Vec<Box<dyn Any>>` display, and an `if` expression missing an `else`. |
| `fastqc` math util subset | `.tmp/corpus-fastqc-math-util-after-explicit-cast-parentheses-r1` | 10/10 translated, 0 translation failures, 0 invalid-pattern files, and 17 generated-Rust errors. `RustExpr::Cast` now parenthesizes compound operands, so explicit Java primitive casts around shifts/bitwise expressions cast the whole lowered expression. The previous three `i64 -> i32` rows from `FastMath` bit extraction are gone. Remaining top families are 5 mismatched types, 4 fallible calls in non-`Result` methods, one trait-object type spelling row, mutable receiver drift, `Vec<Box<dyn Any>>` display, and an `if` expression missing an `else`. |
| `fastqc` math util subset | `.tmp/corpus-fastqc-math-util-after-abstract-method-placeholders-r1` | 10/10 translated, 0 translation failures, 0 invalid-pattern files, and 15 generated-Rust errors. Abstract source methods without bodies now emit typed diverging placeholders, so `ContinuedFraction.getA/getB` no longer compile as non-void functions with implicit `()`. The previous two `expected f64, found ()` rows are gone. Remaining top families are 4 fallible calls in non-`Result` methods, 3 mismatched types, one trait-object type spelling row, mutable receiver drift, `Vec<Box<dyn Any>>` display, and an `if` expression missing an `else`. |
| `fastqc` math util subset | `.tmp/corpus-fastqc-math-util-after-trait-forwarding-slice-recovery-r1` | 10/10 translated, 0 translation failures, 0 invalid-pattern files, and 14 generated-Rust errors. Trait implementation forwarding now adapts borrowed slice trait slots back into owned Java array implementation slots with `.to_vec()`, so `DoubleArray for ResizableDoubleArray` forwards `add_elements(self, values.to_vec())` instead of passing `&[f64]` to a `Vec<f64>` parameter. Remaining top families are 4 fallible calls in non-`Result` methods, 2 mismatched types, one trait-object type spelling row, mutable receiver drift, `Vec<Box<dyn Any>>` display, and an `if` expression missing an `else`. |
| `fastqc` math util subset | `.tmp/corpus-fastqc-math-util-after-synchronized-block-body-r1` | 10/10 translated, 0 translation failures, 1 invalid-pattern file, and 16 generated-Rust errors. Parser block collection now preserves `synchronized (...) { ... }` bodies as ordinary blocks, so the `ResizableDoubleArray.equals(...)` guard is no longer the tail expression and the missing-`else` row is gone. This exposes previously hidden synchronized-body work: Java cast/downcast handling, `Arrays.equals(...)` static facade lowering, optional array `.len()`/arraycopy access inside synchronized copy blocks, and mutable destination handling in static copy helpers. |
| `fastqc` math util subset | `.tmp/corpus-fastqc-math-util-after-arrays-equals-r1` | 10/10 translated, 0 translation failures, 0 invalid-pattern files, and 15 generated-Rust errors. Rejected `instanceof` guards now refine later exact casts to borrowed `downcast_ref::<T>()` views, and JDK `Arrays.equals(...)` lowers to Rust equality instead of an unresolved lowercase `arrays` receiver. The invalid Java cast scan is clean again and the previous `cannot find value arrays` row is gone. Remaining top families are fallible calls in non-`Result` methods, optional array `.len()` on copied fields, Localizable enum-to-interface slot drift, trait-object alias/spelling, mutable receiver drift, and erased-object vector display. |
| `fastqc` math util subset | `.tmp/corpus-fastqc-math-util-after-source-mut-local-abi-infra-r1` | 10/10 translated, 0 translation failures, 0 invalid-pattern files, and 14 generated-Rust errors. Public parameters with field/array-element mutations solve as mutable-borrow slots, and resolved source mutable-borrow parameters now propagate only the caller binding mutability needed for local out values such as `let mut result` in `ResizableDoubleArray.copy()`. Remaining top families are fallible calls in non-`Result` methods, `System.arraycopy`/optional-field read-write ordering, Localizable enum-to-interface slot drift, trait-object alias/spelling, mutable receiver drift, and erased-object vector display. |
| `jaligner` | `.tmp/corpus-sweep-20260724-jaligner-after-level-constants-r3` | 29/29 translated, 0 invalid-pattern files, generated-Rust errors restored to 14 after fixing the shared-support `Level` constant regression. The `Level::SEVERE`/`Level::WARNING` rows are gone. |
| `jaligner` | `.tmp/corpus-sweep-20260724-jaligner-after-optional-resource-stream-r1` | 29/29 translated, 0 invalid-pattern files, generated-Rust errors dropped from 14 to 13. The `InputStream in = null; in = ...getResourceAsStream(...)` case now lowers as a direct optional assignment instead of `Some(Option<InputStream>)`. Remaining top families are `FileInputStream -> InputStream` facade upcast conversion, statement-side `Collections.sort(...)` generic return, `NamedInputStream.get_name()` returning `()`, `Format`/`Pair` interface shape, temporary borrowed string lifetimes, and moved `StringBuffer.append(...)` arguments. |
| `jaligner` | `.tmp/corpus-sweep-20260724-jaligner-after-inputstream-upcast-r3` | 29/29 translated, 0 invalid-pattern files, generated-Rust errors dropped from 13 to 12. The `FileInputStream::new(...)` into `Option<InputStream>` row is gone: `MatrixLoader` now emits `Some(FileInputStream::new(...).into())`, and shared support emits `impl From<FileInputStream> for InputStream`. Remaining top families are `FormatFactory` nullable singleton clone, `NamedInputStream.get_name()` returning `()`, statement-side `Collections.sort(...)` generic return, `Pair -> Format` interface conversion, `SequenceParser` line optionality drift, temporary borrowed string lifetimes, and moved `StringBuffer.append(...)` arguments. |
| `jaligner` | `.tmp/corpus-sweep-20260724-jaligner-after-collections-sort-r4` | 29/29 translated, 0 invalid-pattern files, generated-Rust errors dropped from 12 to 11. Statement-side `Collections.sort(...)` no longer falls through to generic `JavaUtilCollections::sort`; `MatrixLoader` now emits `let mut matrices: Vec<String>` and a comparator-backed `sort_by` block. The previous `cannot borrow matrices as mutable` row is gone. Remaining top families are `FormatFactory` nullable singleton clone, `NamedInputStream.get_name()` returning `()`, `Pair -> Format` interface conversion, `SequenceParser` line optionality drift, temporary borrowed string lifetimes, moved `StringBuffer.append(...)` arguments, and one type-inference row. |
| `jaligner` | `.tmp/corpus-sweep-20260724-jaligner-after-name-getter-r1` | 29/29 translated, 0 invalid-pattern files, generated-Rust errors were 12 after fixing omitted-context support return precision for `NamedInputStream.get_name()`. The getter now emits `-> String`, removing the `Matrix::new(nis.get_name(), ...)` unit mismatch and exposing nullable `BufferedReader.readLine()` flow as the next borrow/slot issue. |
| `jaligner` | `.tmp/corpus-sweep-20260724-jaligner-after-assignment-null-condition-r1` | 29/29 translated, 0 invalid-pattern files, generated-Rust errors dropped from 11 to 8 after `BufferedReader.readLine()` became a `MaybeNull` platform fact and compound assignment-null conditions started assigning then testing `line.is_some()` without moving the optional local. Remaining top families are `FormatFactory` nullable singleton clone, `Pair -> Format` interface conversion, temporary borrowed string lifetimes in append/substring paths, and moved `StringBuffer.append(...)` arguments. |
| `jaligner` | `.tmp/corpus-sweep-20260724-jaligner-after-source-context-upcast-r1` | 29/29 translated, 0 invalid-pattern files, generated-Rust errors dropped from 8 to 5 after emitted source-context superclass conversion and slot upcast support. The three `Pair -> Format` rows are gone; generated tester modules use `Pair::new().into()` and `Pair` emits `impl From<Pair> for Format`. Remaining top families are `FormatFactory` nullable singleton clone, temporary borrowed string lifetimes in append/substring paths, and moved `StringBuffer.append(...)` arguments. |
| `jaligner` | `.tmp/corpus-sweep-20260724-jaligner-after-upcast-field-visibility-r1` | 29/29 translated, 0 invalid-pattern files, and `cargo_check=ok` with 0 generated-Rust errors. `StringBuilder`/`StringBuffer.append(...)` generic facade calls now borrow reusable non-primitive arguments while ternary/value-producing arguments stay owned, static singleton source classes with cloneable `HashMap` fields derive `Clone`, and superclass fields used by emitted cross-module source upcast impls are `pub(crate)` instead of inaccessible private fields. |
| `fastqc` full corpus | `.tmp/corpus-batch-current-fastqc-after-focused-clean-r1` | 156/156 translated with 0 translator failures. Offline generated-crate `cargo check` fails with 1464 reported errors and 28 invalid-pattern files: 16 compile-error placeholders across 10 files and 44 invalid Java-cast downcasts across 23 files. Top generated-Rust families are mismatched types, fallible `?` in non-`Result` methods, erased `Box<dyn Any>` facade calls, unresolved scope/switch scaffolding names, missing facade upcasts, unresolved ImageIO support, `File::new_1`, mutable receiver drift, and `Color` alias constants. Timings: total 1,651,558 ms, translation 1,193,860 ms, global signature fixpoint 699,558 ms, crate assembly 443,516 ms, cargo check 13,188 ms. The `Color` constant row is now targeted by shared-support alias/target associated-constant normalization, and the inherited `ImageIO.write(...)` row is targeted by seeding support/source-summary imports while lowering inherited method bodies. |
| `jhlabs` | `.tmp/corpus-batch-current-jhlabs` | long-running worker still active during this audit |
| `jsoup` | `.tmp/corpus-batch-current-jsoup` | long-running worker still active during this audit |

## Remaining Idioms And Bugs

| Priority | Family | Evidence | Next fixture/action |
|---:|---|---|---|
| 1 | FastQC generated-Rust cleanup | Duplicate-owner minimizations for `BasicStatsTest` and `NormalDistribution` no longer panic, `.put` calls are gone, generic-erased source-super and closed-enum-to-interface coercion are fixed locally, nested interface names no longer block parsing, and the fresh parser rerun proves the `in::read`, raw `as i32 <`, block-comment condition, hex-float, and long-mask parse blockers are gone in generated FastQC sources. Stream-wrapper no-state super construction is fixed for Base64 and public MultiMemberGZIPInputStream constructors in focused checks; private parent-linking MultiMemberGZIPInputStream constructors now emit a precise object-graph mutation diagnostic instead of the broad inheritance-lowering placeholder; FastQC CLI `String.join` varargs no longer emit a placeholder in focused translation; direct source-super trailing method-result calls are preserved in the FastQC exception subset; the focused exception subset now has 0 broad `super(...)` placeholders after external-exception trailing and varargs source-super matching; the `Serializable` marker `instanceof`, current-self exception-supertype, and `HashMap<String,Object>` insert rows now leave the 9-file exception subset with 0 invalid-pattern files. Focused wrapper source-type assembly now preserves source `ExceptionContext`/`Localizable`/`ExceptionContextProvider` while keeping required support impls, direct `StringBuilder.append` is modeled, enum constant labels remain stable, enum constants passed to source interface slots no longer use optional `.as_ref()` recovery, Java enum/source-interface impl fallback is emitted, self-temporary constructor seeds avoid unnecessary `ExceptionContext: Default` requirements when a constructor initializer is available, inherited context trailing calls use `__java_self.context`, source varargs forwarding/spread uses the shared erased-object varargs plan, `keySet().size()` lowers to native length, `keySet()` in Set slots lowers to `HashSet`, `Locale.US` emits a usable short-owner support surface, same-package `ArgUtils.flatten(...)` lowers to a static support owner, `ObjectInputStream.readInt()` has a typed JDK facade, source interface collection add/get paths now use boxed dyn trait carriers, JDK object stream read/write methods now use typed fallible erased-object handoff facades without invalid `Object` downcasts, nullable borrowed external arguments are opt-in, `Map.keySet()` loop keys no longer double-borrow into `Map.get`, indexed erased-object serialization avoids moves, source fields use owned storage, optional owned field getters borrow with `.as_ref()`, typed `readObject()` downcasts no longer trip the invalid-cast scanner, lowercase JDK static-owner aliases now recover to canonical static calls with matching generated support in the FastQC statistics subset, compound array allocation lengths now cast as whole expressions, focused AbstractQCModule getClass/AWT/InputStream/outer-capture rows are gone, display-capable `StringBuilder`/`StringBuffer.append(...)` support drops the focused subset to 63 errors, nullable constructor slot cleanup drops it to 62 errors, constructor-local `BufferedImage.getGraphics()` field assignment cleanup drops it to 55 errors, GUI getter/erased `TableModel.getValueAt(...)` display cleanup drops it to 39 errors, source borrowed getter local recovery drops it to 34 errors, local-shadowed `ZipFile` receiver/facade support closes the `ZipFile.as_ref` family, constructor receiverless field dependency plus UpperCamel static-owner cleanup drop the focused subset to 32 errors, constructor self-field slot recovery drops it to 31 errors, actual-argument-aware JDK constructor slot planning fixes the exposed `FileOutputStream(File/String)` overload shape, JDK `ZipOutputStream -> OutputStream` conversion drops the subset to 30 errors, iHarder `Base64.OutputStream -> OutputStream` conversion drops it to 29 errors, public source field visibility drops it to 27 errors, static-final Pattern associated values drop it to 25 errors, low-precedence Rust/JDK signature facts drop it to 9 errors, dyn-trait container clone plus byte-array/String range plus IO byte-slice facts drop it to 2 errors in `.tmp/corpus-fastqc-abstract-qcmodule-after-io-borrow-slots-r1`, and display-context return hints make that focused subset clean. Latest full FastQC evidence is `.tmp/corpus-batch-current-fastqc-refresh-20260724-r1`: 156/156 translated, 28 invalid-pattern files, 1427 generated-Rust errors. The latest full run exposed output-stream/Base64 shared-support identity drift; the local merger fix now protects shared `OutputStream` from local `Base64OutputStream` aliases and synthesizes `ZipOutputStream -> OutputStream` support. Apache Commons Codec and iHarder Base64 output-stream constructors now have external-pack constructor facts, nested qualified constructor parsing preserves `Base64.OutputStream`, and constructor lowering can apply the existing external `Into` adapter instead of emitting erased downcasts. The no-later-read Base64 constructor assignment now moves its argument directly, while later-read cases still keep audited clones. Focused FastQC proof `.tmp/corpus-fastqc-abstract-qcmodule-after-base64-qualified-new-r2`: 8/8 translated, 0 invalid-pattern files, 9 generated-Rust errors, with `ImageToBase64` emitting `Base64OutputStream::new(os.clone()/* TODO(translation): validate added clone */).into()`. | Fixture and fix the newly exposed focused family: indexed owned `String` reads from `Vec<String>` moving out of the vector. Route indexed element recovery through the shared slot policy so owned slots clone/recover and borrowed slots borrow without local string special cases. |
| 2 | Shared support enum assembly | Fixed locally and corpus-proven in `.tmp/corpus-batch-current-htsjdk-vcf-after-derived-trait-dedupe`: promoted `Display` impls with `Formatter<'_>` keep balanced braces, and manual `PartialEq`/`Eq`/`Hash` impls are not promoted when the enum declaration already derives those traits. | Continue with VCF's next generated-Rust families: enum/facade constants, concrete `instanceof`, string/index coercions, and `HashSet` trait bounds |
| 3 | Spark/DISQ interface bridging | Spark/DISQ has missing `Reference*SparkSource.getBases` impls and borrowed/owned delegate mismatches | Add smaller interface bridge fixture for `getBases` |
| 4 | GenomicsDB overload/name conflicts | GenomicsDB has duplicate `query` definitions and one mismatched type | Minimize overload naming plus facade handoff shape |
| 5 | Generated-Rust errors in ndarray corpuses | Local `ndarray` shim now removes the dependency-cache gate for focused generated crates; BioJava AAPROP, BioJava alignment, JAligner, JAHMM, and Trimmomatic all reach real generated-Rust diagnostics. VarScan is now clean after statement-wrapper return recovery, raw map return precision, statement-side super-boundary no-op propagation, indexed JDK `File` constructor argument cloning, and mutable owned enhanced-for `String` variables | Continue grouping the remaining non-VarScan generated-Rust errors into fixtures, especially array creation, switch scaffolding, Java string/file/logging facades, non-emitter raw keyword leaks, and trait/interface assembly |
| 6 | Switch/state-machine lowering | VCF, BioJava, JAligner show high switch scaffold counts in idiom sweeps | Strengthen switch fixtures for generated scaffold forms |
| 7 | Java collection/map surface gaps | BioJava and JAHMM show collection/map API variants | Add map module-leak and Java spelling variant fixtures |

FastQC priority update: the `SequenceFile.name()` omitted-context row described
above is fixed in `.tmp/corpus-fastqc-abstract-qcmodule-after-display-context-return-r2`.
The next action for FastQC is a full rerun to prove the focused fixes carry
beyond the AbstractQCModule subset, then fixture the next exposed families.

External typed-return update: the minimized HTSJDK
`TypedFeatureReaderHeaderCast` case no longer lowers `(VCFHeader)
reader.getHeader()` through `Option<Box<dyn Any>>`. Returned-call usage now
preserves arity plus resolved external owner metadata for signature solving,
and fact-proven typed erased-facade returns render as named facade values in
support methods. Focused regressions
`typed_feature_reader_header_cast_uses_external_pack_return_fact` and
`object_return_from_typed_external_getter_erases_once_at_boundary` pass. A
broad `cargo test --test idiom_gaps --quiet` rerun after that slice reached
468 passed / 30 failed, removing the former typed feature-reader failure from
the red list.

Source-summary typed-return companion update: source-summary method return
shapes now preserve concrete dispatch for non-`Object` erased-facade return
types, matching the support renderer. The previously red
`source_summary_method_surface_can_be_added_after_external_return_selects_type`
fixture now emits direct `sam_header().unwrap()` without an invalid
`downcast::<SAMFileHeader>()`.

External typed receiver update: fact-backed read-only external receiver
bindings for non-`Object` erased-facade types now solve as shared-borrow
concrete native facade parameters. The VCF typed getter fixtures now expose
`&VCFHeader` instead of `&Box<dyn Any>` while keeping typed collection and
dictionary returns from the external pack facts. Focused regressions
`external_pack_typed_string_list_getter_supports_indexed_sample_access` and
`external_pack_typed_set_and_dictionary_getters_shape_facade_returns` pass.

Current idiom-suite baseline update: a broad
`cargo test --test idiom_gaps --quiet` run on 2026-07-25 reached 474 passed /
24 failed in 528.03s. Focused fixes after that run removed three of those red
items locally: `source_static_final_facade_fields_emit_associated_defaults_not_storage`,
`emitted_type_extending_source_summary_super_gets_inherited_method_surface`,
and `source_summary_super_calls_lower_to_boundary_defaults_not_raw_super_receiver`.
The fixes are shared-path changes: Rust inherent facade support now dedupes
same-name/same-arity static and instance placeholders, and inherited
source-summary bodyless methods lower to typed defaults while redundant support
placeholders are suppressed. The broad suite still needs a fresh rerun to
publish the next exact count.

Focused assertion-drift update: three more 2026-07-25 red-list items pass in
focused reruns without broad-suite recount:
`bare_final_instance_field_read_does_not_become_static_constant`,
`class_literal_get_class_loader_uses_platform_facade`, and
`constructor_nested_arg_reads_assigned_field_without_cloning_self`. The
constructor case is a useful borrow-system marker: external/facade borrow facts
did not apply because the value is an internal source optional field read during
constructor lowering; the generated `.as_ref().cloned().expect(...)` path is
the shared optional-element conversion spelling and still avoids cloning
`__java_self`.

Focused follow-up update: eight additional red-list items now pass in focused
reruns without broad-suite recount:
`documents_primitive_array_allocation_gap`,
`documents_proven_non_null_member_field_gap`,
`documents_string_array_literal_argument_gap`,
`exception_constructor_message_argument_is_stringified`,
`file_output_stream_constructor_selects_file_and_string_overload_slots`,
`java_string_substring_and_hash_code_use_runtime_helpers`,
`java_list_iterator_cursor_uses_platform_facade`, and
`java_exception_logging_surface_compiles_without_system_err_value`. The first
six were stale assertions around newer slot-conversion/support spelling. The
iterator case was a real generated-Rust mutability issue and is now driven by a
typed `FacadeAbiFact` for `Iterator.next()`/`JavaIterator.next()` plus an
owner-aware local mutability scan. The logging case now uses the same nullable
JDK throwable string-return fact for `getLocalizedMessage()`/`get_localized_message`
as `getMessage()`.

## Next Batch

1. Convert the exposed full FastQC families from
   `.tmp/corpus-batch-current-fastqc-after-display-context-return-r1` into
   fixtures and fixes. Prefer Rust signature/ABI facts over new special-case
   lowering whenever a Rust signature proves the same slot shape.
2. Minimize the VCF shared-support enum assembly failure.
3. Add a Spark/DISQ interface bridge fixture around `getBases`.
4. Convert shared errors from the newly unblocked ndarray corpus reruns into
   fixtures, especially `super(...)`, array creation, switch scaffolding,
   Java string/file/logging facades, non-emitter raw keyword leaks, and trait/interface
   assembly.
5. For JAligner, prefer statement-side or mutator-role ABI facts for
   `Collections.sort(...) -> ()`, then handle `NamedInputStream.get_name()`,
   `Pair -> Format`, `SequenceParser` nullable line flow, temporary borrowed
   string lifetimes, and moved `StringBuffer.append(...)` arguments.
