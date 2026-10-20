# java-string-similarity feasibility candidate

Candidate: [upstream v2.0.0](https://github.com/tdebatty/java-string-similarity/releases/tag/v2.0.0),
commit `1372d0b4549f79b9a3d80f5c678cf8e3a787a856` (annotated tag object
`de556acd35948bb7386ac3a5c7b077a8dc4b73d3`). MIT licensed.

This is a candidate pending the baseline and generated-rewrite audit, not a
claim of translator support or a committed incremental fix campaign.
Existing corpus records cover JAligner, Jahmm, VarScan, Commons CSV,
Trimmomatic, BioJava alignment, HTSJDK VCF and JTS, among others; no previous
java-string-similarity translation was found in the corpus scripts/docs.

## Proposed complete scope

All **27 files in src/main/java**, including five source interfaces, both
character-cost interfaces, all algorithms, four examples, and experimental
Sift4 with its method-local mutable Offset class. No GUI or native libraries.
The POM declares only jcip-annotations1.0 as a non-test dependency, and
JUnit4.12 for tests. Annotation metadata must not become runtime stub bodies.

Preparation and runner:

```sh
bash scripts/prepare-java-string-similarity-2.0.0.sh
NEW_JAVA2RUST_BIN="$PWD/.tmp/new-java2rust-v152-retained-deque-contracts" \
  bash scripts/run-java-string-similarity-2.0.0.sh
```

The runner requires the exact commit and clean checkout, validates the full
27-file census, and uses the production translation/assembly/check pipeline.
No source staging or generated-code corrections are deliverables.

## Feasibility gate to perform after the JGraphT checkpoint

Establish translator/parse/compile/placeholder/timing baseline first. Inspect
representative generated bodies and trace entire contracts for:

- Damerau/LongestCommonSubsequence/OptimalStringAlignment: independently
  allocated two-dimensional DP rows, integer indexing and swapping work rows.
- WeightedLevenshtein: retained constructor-injected substitution and optional
  insertion/deletion interfaces, virtual dispatch and exception/null behavior.
- ShingleBased/Cosine/Jaccard/QGram: inherited constructors and methods,
  unmodifiable profile maps, boxed nullable lookups, entry and key iteration,
  regex replacement, and UTF-16 substring/index semantics.
- RatcliffObershelp: recursive source static calls, list merging and string
  ownership across recursive frames.
- Sift4: local Offset type identity, mutable values aliased through LinkedList,
  and removal while scanning by index.
- Examples: anonymous callback capture/lifetime and source interface dispatch.

For each, derive the smallest correct hypothetical Rust rewrite, classify it
as local emission, cross-function contract or representation redesign, and
check whether existing mechanisms can express it. Do not start a piecemeal
fix campaign before this decision. If redesign dominates, compare another
candidate rather than silently excluding difficult files.

## Acceptance and oracle plan

All27 must translate without translator/parse failures or invalid placeholders
and assemble into a cargo-check-clean crate. Java oracles must cover each
algorithm plus null/empty/equal strings, early cutoffs, asymmetrical costs,
supplementary Unicode and surrogate-boundary operations, repeated whitespace,
profile immutability and callback dispatch. Use exact discrete results and
explicit floating tolerances (including NaN/infinity classification), not
bitwise floating equality or weakened semantics. Run upstream tests on the
untouched source as additional evidence. Generated Rust is never repaired by
hand; fixes belong to general translator/runtime mechanisms with focused
regressions and a final full-scope behavior gate.

## Pre-baseline architectural warning

Source inspection found a correctness issue that compiler error counts cannot
detect: `string_char_at_option_projection` in `src/coercion.rs` emits
`.chars().nth(index)`, while `java_string_substring_result` in `src/stdlib.rs`
uses `.chars().count()/skip()/take()`. String search helpers use Rust byte
offsets. `slot_render.rs` maps Java char to Rust char; Rust char cannot hold an
isolated surrogate. These are three incompatible index domains (scalars,
bytes, and Java UTF-16 units), not a missing cast at one call site.

For example, Java Levenshtein distance between one supplementary character
and an empty string is2. ShingleBased with k1 can produce two separate keys
containing isolated surrogates. A correct hypothetical rewrite must store
UTF-16-preserving strings/char values, use one index domain in length,
charAt, substring and searches, and preserve it through profile-map keys,
callback char arguments, array elements and source method signatures. Lossy
UTF-16 decoding or an ASCII-only oracle would not solve this. Classify as a
representation/framework redesign unless an existing lossless representation
can be reused across all these boundaries. Baseline/generated audit is still
required before the selection decision; do not begin isolated compile fixes.

## V152 baseline and selection decision (2026-10-20)

Frozen translator SHA256
`7692d388eb4fc976b3bb0f526a301474dffc84a8b57c558c77051167336b1e81`.
Root `.tmp/java-string-similarity-2.0.0-v152-baseline`, runner log
`.tmp/java-string-similarity-v152-baseline-run.log`. Terminal1:
27files, translator failures0, generated parse failures2, provisional compiler
diagnostics47, **cargo_type_errors_valid0**. Do not treat47 as the complete
type-error baseline: keyword `match` in an enhanced-for binder prevents parsing.
Registry markers9/5files. Total26.228s (resolver1.123, translation5.314,
assembly16.443, check2.265). No competing build/test.

Generated-code audit confirms the following whole-path rewrites:

1. RatcliffObershelp emits `for match` but references `r#match`. Reuse the
   shared identifier renderer for loop declarations as for expressions.
   Local emission fix; the recursive list composition is expressible.
2. ShingleBased produces `HashMap<String,i32>` but declares a retained semantic
   map return, ending in an allocation guard. Plan the original allocation,
   immutable wrapper and all inherited call/iteration consumers as one retained
   map contract. Never copy at the return or drop unmodifiable policy.
   Cross-function contract; existing retained-map mechanisms are relevant.
3. Sift4 emits `JavaNativeCollection<Offset>` without declaring Offset; its
   element loads become erased Any and then field access fails. Lift the local
   class to a declaration with method-scoped identity before source indexing,
   resolution and representation planning. Store/load the same shared Offset
   handle and mutate its trans field, without cloning object payloads. This is
   a source-normalization/framework change, not a local type spelling repair.
4. WeightedLevenshtein has boxed callback fields, clone_box delegation, and
   anonymous implementations missing required trait companions. Correct
   rewriting must preserve callback identity/state across constructor storage,
   delegation, retained field loads, virtual cost calls and anonymous-class
   companions. Existing adapters can help, but merely deriving Clone is not a
   semantic proof. DP row buffers themselves can be expressed by existing
   array machinery; use swaps/moves only with proven nonescaping references.
5. The emitted WeightedLevenshtein and Damerau bodies use byte `.len()` with
   scalar `.chars().nth()`; ShingleBased stores substring results as String map
   keys. The Unicode redesign described above is exercised, not hypothetical.
   Rewriting charAt alone cannot fix the signatures, keys and substrings.

**Do not select this corpus for the present smaller-corpus campaign.** Despite
its modest provisional diagnostic count, complete support requires a
lossless UTF-16/char representation across core APIs and source normalization
for local classes. These are useful future projects, but larger than the
intended bounded step after JGraphT. Retain this baseline/audit, do not exclude
Unicode or Sift4 to manufacture completion. Compare JAMA's seven-file numeric
production core next, including its full Matrix read/print API and live arrays.
