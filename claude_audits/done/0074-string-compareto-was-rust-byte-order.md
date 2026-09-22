# 0074 — `String.compareTo` used Rust UTF-8 ordering and discarded magnitude

Status: **fixed** — translated `String.compareTo` now follows Java's UTF-16
lexicographic contract and returns the first code-unit or length difference.

## Root cause

The runtime helper called Rust `str::cmp` and mapped its result to `-1`, `0`, or
`1`. That differs from Java in two independent ways:

1. Java compares UTF-16 code units, while Rust string ordering compares UTF-8
   bytes. A supplementary character such as U+10000 sorts before U+E000 in
   Java (`0xD800 < 0xE000`) but after it under UTF-8 byte ordering.
2. `String.compareTo` returns the first code-unit difference, or the UTF-16
   length difference for a prefix. It does not normalize every result to a
   sign. For example, `"a".compareTo("c") == -2`.

The helper now walks `encode_utf16()` iterators and implements that contract
directly.

## Verification

`tests/differential/StringCompareToUtf16.java` is a Java/Rust oracle covering
ASCII magnitude, prefix length, and both directions of the supplementary/BMP
ordering boundary. The focused differential test passes.

