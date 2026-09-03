# 0055 — The syntax-error detector is `match { _ => {} }`, and `translate_status` is the literal `"ok"`

Status: **fixed** — moved to `done/` after the parser began reporting
tree-sitter `ERROR`/missing nodes, single-file translation began returning a
non-zero exit status on `Severity::Error`, and batch `translate_status` became a
derived severity signal instead of a constant.
Severity: **high**, **live**
Kind: three no-op reporting surfaces stacked on one path. Two are single lines.
Sibling of [0054](0054-switch-arms-are-silently-deleted.md) — 0054 is what leaks
through the hole this one leaves open.

Codex follow-up 2026-08-24: fixed Findings A-C. `collect_syntax_diagnostics`
now walks all tree-sitter children and emits `java_syntax_error` /
`java_syntax_missing` with spans. The translate CLI exits non-zero if external
fact diagnostics or translation diagnostics contain `Severity::Error`.
`translate-batch` keeps producing artifacts for the batch, but each status row
now records `ok`, `diagnostics`, or `errors` from the file's diagnostic bag.
Pinned by parser-level negative tests plus binary-level translate and
translate-batch tests.

Measured on `b391645`, `target/release/new-java2rust`, `javac` 19.0.2.

## Symptom

The translator never reports that its Java input failed to parse. Input that
`javac` rejects outright is silently translated to Rust that compiles and is
wrong, with exit code 0 and empty stderr. In batch mode every file is recorded
as `ok` regardless of what happened.

## Minimal repro

```java
public class E {
    static int m1(int a, int b) { return a + b ~; }        // javac: ';' expected
    static int m3(int a, int b) { int x = a ? b; return x; } // javac: ':' expected
    static int m5(int a, int b) { return a +* b; }          // javac: illegal start of expression
}
```

```
$ javac -d /tmp/jc E.java
E.java:2: error: ';' expected
E.java:3: error: : expected
E.java:4: error: illegal start of expression
3 errors

$ ./target/release/new-java2rust E.java > e.rs 2> e.err
$ echo "exit=$?  stderr bytes=$(wc -c < e.err)"
exit=0  stderr bytes=0
```

```rust
pub fn m1(a: i32, b: i32) -> i32 { return (a).wrapping_add(b); }   // the `~` vanished
pub fn m3(a: i32, b: i32) -> i32 { let x: i32 = a; return x; }     // `? b` vanished
pub fn m5(a: i32, b: i32) -> i32 { return (a).wrapping_mul(b); }   // `+` became `*`
```

Three ill-formed programs, three confident wrong answers, no diagnostic.

## Finding A — `collect_syntax_diagnostics` matches nothing

`src/parser.rs:2875`, the only function in the pipeline whose name says it finds
syntax errors, called once at `src/parser.rs:19` from `parse_java_source`:

```rust
fn collect_syntax_diagnostics(node: tree_sitter::Node, diagnostics: &mut DiagnosticBag) {
    match node.kind() {
        _ => {}
    }

    for child in named_children(node) {
        collect_syntax_diagnostics(child, diagnostics);
    }
}
```

It walks the entire tree and pushes nothing. `src/parser.rs` contains **zero**
occurrences of `has_error`, `is_error`, `is_missing`, or `"ERROR"`:

```
$ grep -c 'has_error\|is_error\|is_missing\|"ERROR"' src/parser.rs
0
```

so tree-sitter's `Node::has_error()` — one call on the root, which is exactly
what this function is positioned to make — is never used. The `diagnostics`
parameter escapes an unused-variable warning only because it is forwarded to the
recursive call, and `match x { _ => {} }` is not a rustc lint (it is clippy's
`match_single_binding`, and clippy does not run: `scripts/gate.sh` discovers
`tests/*.rs` targets and there is no clippy step).

The consequence is that tree-sitter's error *recovery* — which is good, and
which is the reason the corrupted files above still yield a whole class — is
treated as success. Recovery deletes the tokens it cannot place; nothing counts
the deletions.

Not every recovery is silent: an `ERROR` node landing at statement or argument
position does reach the catch-all arm of `parser.rs:1317 parse_stmt_node`
(`_ => Stmt::Unsupported("unsupported statement node: ERROR")`) or of
`parser.rs:1959 parse_expr_node` (`_ => unsupported_expr_node(…)`) and becomes a
`compile_error!`. A merge
conflict marker inside a method body, for instance, is loud. The point is that
whether an unparseable file is loud or silent is decided by where tree-sitter's
recovery happens to put the `ERROR` node, not by any policy.

## Finding B — the CLI exits 0 even when the bag holds `Severity::Error`

`src/main.rs:167`:

```rust
print_diagnostics(&fact_diagnostics);
print_diagnostics(&translation.diagnostics);
println!("{}", translation.rust);
ExitCode::SUCCESS
```

`DiagnosticBag::has_errors()` (`src/diagnostics.rs:69 DiagnosticBag::has_errors`)
exists and is correct.
Its only production caller is `src/resolver_benchmark.rs` (`:593`, `:1049`,
`:1981`); everything else is `tests/`. Neither the `Translate` nor the
`TranslateBatch` arm of `main` consults it.

Verified with a file that is not Java at all:

```
$ printf 'this is not java at all\n<<<<<<< HEAD\nfoo bar baz\n' > NotJava.java
$ ./target/release/new-java2rust NotJava.java > nj.rs
Error parse_no_class: no top-level class found
$ echo "exit=$?  bytes=$(wc -c < nj.rs)"
exit=0  bytes=51600
```

A `Severity::Error` diagnostic is raised, printed, and then 51 KB of prelude is
written to stdout under a success status.

This is not hypothetical downstream: `scripts/translate-focused-package.sh:589`
initialises `local status="ok"` and only ever changes it at `:603` from the
process exit code (`failed`, or `timeout` on 124). Because the exit code is
`ExitCode::SUCCESS` for every input the binary does not crash on, that script's
`translate_status` column is a constant too — arrived at independently of
Finding C, by the same missing signal. Its `diagnostics` column is
`wc -l < "$log_path"`, a line count of stderr, so severity is erased there as
well.

## Finding C — `translate_status` is a constant

`src/main.rs:652` declares the batch status header:

```rust
"source\trust_path\ttranslate_status\tdiagnostics\tinvalid_patterns\tunsupported_members\tstubbed_constants\n"
```

and `src/main.rs:677` fills it:

```rust
status.push_str(&format!(
    "{}\t{}\tok\t{}\t{}\t{}\t{}\n",     //        ^^ hardcoded
    translation.path, rust_path.display(),
    translation.diagnostics.len(), invalid_patterns, unsupported_members, stubbed_constants
));
```

`grep -rn translate_status src/` returns exactly two hits, both above. There is
no code path that writes anything other than `ok`. The column is consumed by
`scripts/compare-focused-batch-artifacts.py:63,78,88`, which compares it between
runs — a comparison of a constant against itself. `tests/batch_translation.rs:598`
asserts the exact status line including `\tok\t`, so the constant is pinned by a
test.

The adjacent `diagnostics` column carries `translation.diagnostics.len()`, a
count with no severity, so a run with 200 `Unsupported` notes and a run with 200
`Error`s are indistinguishable in the artifact.

## Finding D — "parse_failures" in the gate means *Rust* parse failures

`scripts/focused-parse-failures.sh:write_parse_failures` greps a **cargo** log
for `error: expected`, `unclosed delimiter`, `unterminated`, etc. — i.e. it
counts places where the *generated Rust* is not parseable, and gates
`cargo_type_errors_valid` on that count being zero. There is no Java-side
counterpart anywhere in `scripts/`. So the one metric named after parsing
measures the output, and the input's parseability is measured by nothing at all.

This is why a reader scanning the gate can reasonably conclude parse health is
covered.

## Why nothing noticed

- Findings A and C are each a single line that reads as if it does the thing its
  name says. `match node.kind() { _ => {} }` is the shape of a dispatcher whose
  arms were never written, and it compiles clean.
- The suite's parser oracle is `collect_program_fallbacks`, which counts
  `Expr::Unsupported` / `Stmt::Unsupported` markers in the IR. Silent recovery
  produces none, so a file that lost half its tokens is indistinguishable from a
  clean one. Same blind spot as
  [0018](0018-the-compile-error-metric-cannot-see-silent-failures.md).
- Every test constructs its Java inline, so every test input is well-formed by
  construction. Nothing in `tests/` feeds the parser a file it should reject —
  there is no negative test for the parser at all.
- `tests/batch_translation.rs:598` pins `ok`, which converts the missing
  computation into an asserted invariant.

## Direction that removes the possibility

1. **Make the tree's error state part of the parse result, not an optional
   check.** `parse_java_source` already returns `ParsedProgram { program,
   diagnostics }`. Replace the body of `collect_syntax_diagnostics` with a real
   walk that pushes one `Severity::Error` per `node.is_error() || node.is_missing()`,
   carrying `span(node)` — the `Span` machinery is already there and already
   unused for this. One `root.has_error()` check would catch the file-level case
   for free.
2. **Make "translated a file with errors" unrepresentable rather than
   discouraged.** The strongest version is a return type: have the translate
   entry point yield `Result<Translation, DiagnosticBag>` — or a
   `Translation { rust: Option<String>, .. }` — so a caller cannot reach the
   Rust text of a program the parser refused. Short of that, `main` must gate
   on `has_errors()` and return a non-zero `ExitCode`, and batch mode must write
   the file with a non-`ok` status.
3. **Compute `translate_status`, or delete the column.** A field that is written
   but never derived is the same defect class as the no-op
   `--no-builtin-external-packs` flag recorded in
   [0050](done/0050-builtin-pack-facts-and-name-aliases-rebuilt-per-call.md):
   an artifact that reads as a signal and carries none. It should be a function
   of the bag (`ok` / `diagnostics` / `errors`), and the severity breakdown
   should replace the bare `diagnostics.len()` count.
4. **Add a negative parser test.** `tests/parser_gaps.rs` has no input that the
   parser is *required* to reject. Two cases — a file with a stray token inside
   a method, and a file that is not Java — asserting `parsed.diagnostics.has_errors()`
   would have caught all of A, B and the silent half of 0054.

Step 1 is roughly ten lines and turns 0054's whole class of failure from
"different answer" into "reported error" for any input that is actually
malformed. It does not fix 0054 itself — those inputs parse cleanly — but it
closes the door this audit is about.

---

## Re-verified 2026-09-02 — all three findings fixed

Findings **A and B**: the repro exits 1 with three `java_syntax_error`
diagnostics, `NotJava.java` exits 1, four further malformed shapes are caught,
and 60 valid corpus files produce **zero false positives**.

Finding **C** was fixed in form only and has now been fixed properly.
`translate_status` was derived instead of the literal `"ok"`, but batch mode
pooled every *primary* file's parse diagnostics into a batch-wide bag and
appended that bag to each file. Every file carried every other file's parse
errors, so the column had flipped from constant-`ok` to effectively
constant-`errors` — still not a measurement. Measured on a three-file batch with
one unparseable member:

| file | before | after |
|---|---|---|
| `Good.java` | `errors`, 1 diagnostic | `ok`, 0 |
| `Also.java` | `errors`, 1 diagnostic | `ok`, 0 |
| `Bad.java` | `errors`, **2** diagnostics | `errors`, 1 |

The bad file was counted twice because its diagnostics reach it directly through
`parsed.diagnostics` as well.

The fix is a deletion, not a filter: a primary file's parse diagnostics are
simply not put in the shared bag. The bag holds context and summary diagnostics,
which genuinely are shared by every file — the invariant the single-file path
always kept. The two paths now agree by construction rather than by parallel
maintenance.

Regression test: `translate_batch_keeps_parse_diagnostics_with_the_file_that_caused_them`
in `tests/batch_translation.rs`, which asserts both the isolation and the
no-double-count.

Suite unchanged against HEAD (`batch_translation` 20/2 → 21/2; the two failures
are pre-existing and unrelated, in the offline-cargo and external-pack wrapper
tests).
