# 0020 — Text block content is not processed; incidental indentation is kept

Status: **validated** (compiles cleanly, wrong string value)
Severity: medium — silent wrong value
Kind: literal semantics

## Symptom

A Java text block (`"""…"""`, JLS 3.10.6) is emitted with its raw source content:
the leading line terminator and the incidental leading whitespace are not
stripped.

```java
public class P4 {
    public String run() {
        String s = """
        hello
        world""";
        return s;
    }
}
```

| | value |
|---|---|
| Java | `"hello\nworld"` |
| emitted Rust | `"\n        hello\n        world"` |

Emitted:

```rust
let s: String = "\n        hello\n        world".to_string();
```

Compiles cleanly. Any code that compares, hashes, parses, or writes this string
is wrong.

## Root cause

The text block is lexed and escaped correctly — quoting and newlines survive —
but the JLS content-derivation step is skipped. Java derives a text block's
value by:

1. removing the line terminator immediately after the opening `"""`;
2. computing the **minimum indentation** across all non-blank content lines
   *and* the closing delimiter line, then stripping that many leading white
   space characters from every line;
3. stripping trailing white space from each line;
4. processing escape sequences, including the text-block-only `\<line-terminator>`
   (line continuation) and `\s` (retained space).

Only step 4's ordinary escapes appear to be applied.

## Fix

Implement the derivation in the literal path, in JLS order. The subtlety worth
calling out: the **closing delimiter's own indentation participates** in the
minimum. This is what lets an author control the strip depth:

```java
String a = """
    hello
    world""";        // closing on content line -> min indent 4 -> "hello\nworld"

String b = """
    hello
    world
    """;             // closing on its own line, indent 4 -> "hello\nworld\n"

String c = """
        hello
    world""";        // min indent 4 -> "    hello\nworld"
```

Blank lines do not contribute to the minimum, but the closing delimiter line
does.

## Regression tests worth adding

The three cases above, plus:

```java
"""
  a\
  b"""            // line continuation -> "a  b"  (no newline)
"""
  a\s
  b"""            // \s retains one space -> "a \nb"
```

and a negative: an ordinary `"…"` literal must be unaffected.

## Note on priority

Text blocks are Java 15+. Whether this matters depends on the corpora — the
current ones (FastQC, jaligner, biojava, htsjdk, picard, GATK) are mostly older
Java, so real-world exposure today is probably low. Worth fixing when the
literal path is touched for another reason rather than on its own; but it is a
silent wrong value, so it should not be closed as "won't fix" while text blocks
parse at all.
