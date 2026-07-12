# Control Flow

## Conditionals

Java `if` lowers directly to Rust `if`. Ternary expressions lower to Rust `if` expressions when both branches produce compatible Rust types.

## Loops

- `while` lowers to `while`.
- `do/while` lowers to `loop` with a trailing condition break.
- Classic `for` lowers to initializer statements plus `while` or `loop`, preserving update execution after `continue`.
- Enhanced `for` lowers through carrier-specific iteration rules.

## Break And Continue

Unlabeled `break` and `continue` lower directly. Labeled Java flow requires Rust loop labels. Continue in a classic `for` must still execute the update expression; this may require a structural loop rewrite.

### Classic `for` Rewrite

The normalized Java IR for a classic `for` is:

```text
init;
loop label {
    if condition is false {
        break;
    }
    body;
    update;
}
```

Plain `continue` inside the body jumps to a generated update block, then returns to the condition check. If Rust's native `continue` would skip the update expression, lower the loop as:

```text
'loop_label: loop {
    if condition is false {
        break 'loop_label;
    }
    'body_result: loop {
        body statements may break 'body_result for continue;
        break 'body_result;
    }
    update;
}
```

For labeled `continue outer` targeting an outer classic `for`, the inner site must branch to the outer loop's update path, not directly to the outer condition. This rewrite is mandatory for examples such as `LabeledContinueForUpdate.java`.

## Switch

Expression-like `switch` over primitives, strings, or enums should lower to `match` when fallthrough is absent. Fallthrough requires grouped arms or an explicit statement-level rewrite. Unsupported fallthrough must be diagnosed rather than silently dropped.

### Statement `switch` With Fallthrough

The first supported statement-switch lowering prioritizes correctness over prettiness:

1. Evaluate the selector exactly once into a generated local.
2. Create a generated `matched` boolean initialized to `false`.
3. Enter a labeled one-shot loop representing the switch body.
4. For each case group:
   - If no previous group matched, compare the selector against the group's labels.
   - Set `matched = true` when a label matches.
   - Execute the group's statements when `matched` is true.
5. Lower Java `break` inside the switch body to `break 'generated_switch_label`.
6. End the one-shot loop with an unconditional break so non-breaking fallthrough leaves the switch after the last case.

The generated shape is:

```rust
{
    let __switch_value = selector;
    let mut __switch_matched = false;
    'java_switch: loop {
        if !__switch_matched && (__switch_value == label_a) {
            __switch_matched = true;
        }
        if __switch_matched {
            // case statements; Java break becomes break 'java_switch
        }
        break 'java_switch;
    }
}
```

This lowering lives in `lower`. The parser only records `Stmt::Switch`; `resolve` and `types` own any later selector/type facts needed to improve the output to `match`.

## Definite Assignment

Java definite-assignment guarantees are stronger than Rust initialization rules in some places and weaker in nullable references. The resolver should produce definite-assignment facts before Rust IR emission so declarations can be initialized exactly once when possible.
