# 0054 — Switch arms the parser does not recognise are deleted, not reported

Status: **fixed** — moved to `done/` after switch rules, comma labels, shared
yield labels, and unsupported switch-expression block/throw arms were preserved
instead of silently deleted.
Severity: **critical**, **live**
Kind: silent wrong answer on **valid, javac-clean, mainstream Java**. The output
compiles, the exit code is 0, stderr is empty, and `count-invalid-patterns`
returns 0.

Codex follow-up 2026-08-24: fixed the three parser roots. Statement switch
parsing now consumes `switch_rule` children; arrow statement rules receive an
implicit non-fallthrough break and can optimize to assignment matches. Switch
labels are plural, so `case 1, 2` survives in statement and expression switches.
Switch-expression colon groups now carry pending labels forward and preserve
unsupported block/throw arms as `Expr::Unsupported` instead of returning `None`.
Pinned by parser IR tests and `tests/differential/SwitchArms.java`, whose Java
and generated Rust outputs both produce `11,99,22,22,22,22,22,22`.

Measured on `b391645`, `target/release/new-java2rust`, `javac`/`java` 19.0.2,
`rustc --edition 2021`.

## Symptom

Five ordinary `switch` spellings translate to Rust that silently returns the
wrong value. Java oracle vs. the compiled generated Rust, same program:

| method | Java | Rust | what was deleted |
|---|---|---|---|
| `arrowStmt(1)` / `(2)` | `11` / `99` | **`-1`** / **`-1`** | the *entire* switch body |
| `commaExpr(1)` / `(2)` | `22` / `22` | `22` / **`99`** | the label `2` in `case 1, 2 ->` |
| `commaStmt(1)` / `(2)` | `22` / `22` | `22` / **`99`** | the label `2` in `case 1, 2:` |
| `sharedExpr(1)` / `(2)` | `22` / `22` | **`99`** / `22` | the whole `case 1:` arm |
| `blockExpr(1)` / `(2)` | `8` / `99` | **`99`** / `99` | the whole `case 1: { … yield … }` arm |

Eight of ten answers are wrong. Nothing anywhere says so.

## Minimal repro

```java
public class SwitchArms {
    static int arrowStmt(int x) {                       // arrow-form switch STATEMENT
        int r = -1;
        switch (x) { case 1 -> r = 11; default -> r = 99; }
        return r;
    }
    static int commaExpr(int x) {                       // comma labels, arrow expression
        return switch (x) { case 1, 2 -> 22; default -> 99; };
    }
    static int commaStmt(int x) {                       // comma labels, colon statement
        int r = -1;
        switch (x) { case 1, 2: r = 22; break; default: r = 99; }
        return r;
    }
    static int sharedExpr(int x) {                      // shared labels, colon expression
        return switch (x) { case 1: case 2: yield 22; default: yield 99; };
    }
    static int blockExpr(int x) {                       // colon expression, yield inside a block
        return switch (x) { case 1: { int t = 4; yield t * 2; } default: yield 99; };
    }
    public static void main(String[] a) {
        System.out.println("arrowStmt  1=" + arrowStmt(1)  + " 2=" + arrowStmt(2));
        System.out.println("commaExpr  1=" + commaExpr(1)  + " 2=" + commaExpr(2));
        System.out.println("commaStmt  1=" + commaStmt(1)  + " 2=" + commaStmt(2));
        System.out.println("sharedExpr 1=" + sharedExpr(1) + " 2=" + sharedExpr(2));
        System.out.println("blockExpr  1=" + blockExpr(1)  + " 2=" + blockExpr(2));
    }
}
```

## Verified evidence

```
$ javac -d /tmp/jc SwitchArms.java && java -cp /tmp/jc SwitchArms
arrowStmt  1=11 2=99
commaExpr  1=22 2=22
commaStmt  1=22 2=22
sharedExpr 1=22 2=22
blockExpr  1=8 2=99

$ ./target/release/new-java2rust SwitchArms.java > sa.rs 2> sa.err
$ echo "exit=$?  stderr bytes=$(wc -c < sa.err)"
exit=0  stderr bytes=0
$ ./target/release/new-java2rust count-invalid-patterns sa.rs
0

$ printf '\nfn main() { SwitchArms::main(Vec::new()); }\n' >> sa.rs
$ rustc --edition 2021 -o sa_bin sa.rs        # 0 errors, 3 unrelated warnings
$ ./sa_bin
arrowStmt  1=-1 2=-1
commaExpr  1=22 2=99
commaStmt  1=22 2=99
sharedExpr 1=99 2=22
blockExpr  1=99 2=99
```

The generated Rust:

```rust
pub fn arrow_stmt(x: i32) -> i32 {
    let r: i32 = -1;
    { let __java_switch_value = x; let mut __java_switch_matched = false; 'java_switch: loop {
    break 'java_switch ();
} };                                        // <-- zero arms. The switch is gone.
    return r;
}

pub fn comma_expr(x: i32) -> i32  { return match x { 1 => 22, _ => 99 }; }   // label 2 gone
pub fn comma_stmt(x: i32) -> i32  { let mut r: i32 = -1; r = match x { 1 => 22, _ => 99 }; return r; }
pub fn shared_expr(x: i32) -> i32 { return match x { 2 => 22, _ => 99 }; }   // arm `case 1:` gone
pub fn block_expr(x: i32) -> i32  { return match x { _ => 99 }; }            // arm `case 1:` gone
```

A `case 1: throw new IllegalStateException("boom");` arm in a switch *expression*
is deleted the same way — the Java throws, the Rust returns the `default` value.

## Root cause — three deletions in one file, all of the same shape

The grammar (`tree-sitter-java-0.23.5/grammar.js:488`) says a switch body is
**either** a run of colon-groups **or** a run of arrow-rules, and that one label
node can carry several expressions:

```js
switch_block: $ => seq('{', choice(repeat($.switch_block_statement_group),
                                   repeat($.switch_rule)), '}'),
switch_block_statement_group: $ => prec.left(seq(repeat1(seq($.switch_label, ':')),
                                                 repeat($.statement))),
switch_rule: $ => seq($.switch_label, '->',
                      choice($.expression_statement, $.throw_statement, $.block)),
switch_label: $ => choice(seq('case', choice($.pattern, commaSep1($.expression)),
                              optional($.guard)), 'default'),
```

`switch_expression` is the *same* node for statements and expressions — the
grammar comment at line 556 says so — but the translator parses it with **two
different functions, 600 lines apart, that disagree about which children exist.**

### A. `switch_rule` is unknown to the statement parser — `parser.rs:1610 parse_switch_block`

```rust
fn parse_switch_block(node, source, diagnostics) -> Vec<SwitchCase> {
    named_children(node).into_iter()
        .filter_map(|child| match child.kind() {
            "switch_block_statement_group" => Some(parse_switch_group(child, source, diagnostics)),
            _ => None,                       // <-- every `switch_rule` lands here
        })
        .collect()
}
```

An arrow-form switch statement is *all* `switch_rule` children, so this returns
an **empty `Vec`** and `Stmt::Switch { cases: vec![] }` reaches lowering, which
faithfully emits a loop with no arms. The expression sibling
(`parser.rs:2223 parse_switch_expr_block`) handles **both** kinds:

```rust
"switch_rule" => Some(parse_switch_expr_rule(child, source)),
"switch_block_statement_group" => parse_switch_expr_yield_group(child, source),
```

One of the two transcriptions of the grammar is complete. Nothing makes them
agree.

### B. Only the first label of a `switch_label` survives — `parser.rs:1648 parse_switch_label`

```rust
fn parse_switch_label(node, source) -> SwitchLabel {
    first_named_child(node)                       // commaSep1 -> N children, 1 taken
        .map(|label| SwitchLabel::Expr(parse_expr_node(label, source)))
        .unwrap_or(SwitchLabel::Default)          // and "no child" silently means `default`
}
```

The return type is the defect: `SwitchLabel`, singular, for a node the grammar
defines as `commaSep1($.expression)`. Both parse paths share it, so `case 1, 2, 3`
loses two labels in **all four** switch spellings. Note also `optional($.guard)`
— a Java 21 `when` clause is dropped by the same `first_named_child`.

The IR and the emitter are innocent: `java_ir.rs:222 SwitchCase.labels` and
`java_ir.rs:332 SwitchExprCase.labels` are both `Vec<SwitchLabel>`, and
`lower.rs:11267 patterns.join(" | ")` renders `1 | 2 => …` correctly when the
labels are there. `case 1: case 2:` in a *statement* switch does produce
`1 | 2 => 22`. Only the parser loses them.

### C. A colon group with no direct `yield` is dropped — `parser.rs:2265 parse_switch_expr_yield_group`

```rust
fn parse_switch_expr_yield_group(node, source) -> Option<SwitchExprCase> {
    let mut labels = Vec::new();
    let mut value = None;
    for child in named_children(node) {
        match child.kind() {
            "switch_label"    => labels.push(parse_switch_label(child, source)),
            "statement"       => { if … stmt.kind() == "yield_statement" { value = … } }
            "yield_statement" => value = parse_yield_statement_value(child, source),
            _ => {}                                    // block / throw / anything else
        }
    }
    value.map(|value| SwitchExprCase { labels, value })   // <-- None discards the labels
}
```

`filter_map` at `parser.rs:2228` then removes the `None`, so the arm *and its
labels* vanish from `Expr::Switch::cases`. Three valid Java shapes hit it:

- **shared labels** — `case 1:` becomes a group carrying a label and no statements, so `value` stays `None` and the label `1` is destroyed;
- `case 1: { … yield x; }` — the `yield` is inside a `block` child, matched by `_ => {}`;
- `case 1: throw …;` — `throw_statement` is matched by `_ => {}`.

The first of these is the sharpest, because **the translator already knows how to
handle it — in the other lowering.** `lower.rs:11313 plan_switch_assignment`
carries a `pending_labels` accumulator precisely so that a label group with no
statements merges into the next one:

```rust
for (index, case) in cases.iter().enumerate() {
    pending_labels.extend(plan_switch_case(case).labels);
    if case.statements.is_empty() { continue; }        // fall through, keep the labels
    …
}
```

That is why `case 1: case 2: r = 22; break;` correctly renders `1 | 2 => 22` in a
switch *statement*. The switch-expression lowering
(`lower.rs:11227 lower_switch_expr`) has no such accumulator — and could
not use one, because `parse_switch_expr_yield_group` already threw the empty
group away before lowering ran. The fallthrough rule is implemented once, in the
place where the parser happens not to have destroyed its input.

The **immediately preceding function** handles the identical failure condition
the opposite way. `parser.rs:2234 parse_switch_expr_rule` returns
`SwitchExprCase`, not `Option`, and fills the missing value with

```rust
value.unwrap_or_else(|| unsupported_expr_node(node, "switch expression rule without value"))
```

which reaches the output as `compile_error!(…)` — loud, and the maintainer's
preferred outcome. Two sibling functions, one grammar, one failure condition,
opposite policies. The one that returns `Option` is the one that lies.

## Why nothing noticed

- **The suite's own "did we give up?" oracle cannot see a deletion.**
  `tests/parser_gaps.rs:652 parses_arrow_switch_expression_without_fallbacks` and
  `:680 parses_yield_switch_expression_without_fallbacks` both assert
  `collect_program_fallbacks(&parsed.program).is_empty()`. That helper looks for
  `Expr::Unsupported` / `Stmt::Unsupported` markers. A deleted arm leaves **no
  marker**, so it passes the fallback check trivially. This is
  [0018](0018-the-compile-error-metric-cannot-see-silent-failures.md)
  reproduced inside the parser suite: the metric measures the loud failure mode
  and is blind to the quiet one.
- **Those same two tests assert `cases.len() == 2` — and never assert on the
  labels.** A case whose label list lost an entry still counts as one case.
- **The corpus has no instance of the broken spellings.** Across all **221**
  `.java` files in `examples/` and `tests/`, `case … ->` appears **zero** times
  and comma-separated labels appear **zero** times. In the whole repository
  (excluding `target/`) `case … -> ` occurs three times: `tests/parser_gaps.rs:658`
  and `tests/idiom_gaps.rs:29149`, inline Java in the two tests above, plus
  `claude_audits/repro/P2.java` — all three single-label switch *expressions*,
  the one arrow shape that works. `tests/differential/ControlFlow.java` covers
  only single-label colon statements.
- **`count-invalid-patterns` returns 0** on the wrong output, because the wrong
  output contains no invalid pattern — it is well-formed Rust that computes a
  different function.
- **rustc catches it only by accident.** `case 1: int t = 7; yield t;` drops the
  declaration and keeps `1 => t`, which is `E0425`. Every other shape compiles.

## Direction that makes the silent case impossible

The three defects share one representable state: *"I walked a child I did not
recognise and produced a smaller answer."* Remove the ability to represent it.

1. **`parse_switch_label` must return `Vec<SwitchLabel>`.** The grammar node is
   `commaSep1($.expression)`; a function returning one label for a node that
   holds N cannot be right. Have it map *all* named children, and refuse (one
   `SwitchLabel::Unsupported`, or a diagnostic) on a `pattern` or `guard` child
   rather than silently reading the first. `unwrap_or(SwitchLabel::Default)`
   should not exist — `default` is the *absence of the `case` keyword*, which is
   a different question from "the node had no named children".
2. **Delete `parse_switch_block`; make the statement path call
   `parse_switch_expr_block`'s dispatcher.** One `switch_expression` node, one
   child dispatcher. The grammar has exactly two child kinds and both must be
   handled; today the completeness of that handling depends on which of two
   functions you happen to reach.
3. **`parse_switch_expr_yield_group` must not return `Option`.** Give it the
   same signature as its sibling `parse_switch_expr_rule` — return
   `SwitchExprCase` and fill a missing value with
   `unsupported_expr_node(node, "switch expression group without yield")`. Then
   `parse_switch_expr_block` needs no `filter_map`, the labels survive, and the
   three unsupported shapes surface as `compile_error!` instead of a plausible
   number.
4. **Structural guard for the whole family**: a parser that consumed a named
   child without producing IR and without recording a diagnostic is a bug in
   every case, not just switches. The same `filter_map`/`?`-on-a-child shape is
   at `parser.rs:1028` (a field declarator with no `name` disappears) and
   `parser.rs:1115 parse_params_node` (a `formal_parameter` with no `name`
   disappears, silently changing the method's arity). A child-visitor that
   asserts "every named child was either consumed or diagnosed" would make all
   of these unrepresentable rather than found one at a time.

Steps 1 and 3 are local and mechanical; step 2 removes the duplicate
transcription of the grammar that made A possible.

## Regression fixtures the fix needs

`SwitchArms.java` above belongs in `tests/differential/` — it is a five-line-per-
case oracle for exactly the shapes nothing covers today. Add to
`tests/parser_gaps.rs`, alongside the two existing tests:

- an arrow-form switch **statement** asserting `cases.len() == 2`;
- `case 1, 2, 3 ->` and `case 1, 2, 3:` asserting `labels.len() == 3`;
- `case 1: case 2: yield` asserting one case with two labels;
- `case 1: { yield x; }` and `case 1: throw …` asserting an `Expr::Unsupported`
  is present rather than the arm being absent.

The last two are the ones that matter most: they pin *refusal* as the required
behaviour, so a future "improvement" cannot quietly go back to deleting.
