# 0079 — The gate read nested test summaries as suite results

Status: **fixed** — suite counts are parsed from the terminal outer-harness
result, and green targets no longer retain empty failure-name files.

## Root cause

`scripts/gate.sh` selected the first line beginning with `test result`. A test
that compiles or runs a generated crate can include that nested crate's test
summary in its captured panic output. On a failing outer test, the nested line
can therefore precede the real harness summary, yielding impossible counts and
causing `--record` to delete a non-empty failure-name oracle.

Both integration-target and library parsing now select the last matching line,
which is the outer harness's terminal summary. A source-level regression test
pins both parser sites. Re-recording removes comment-only failure files for
green suites, restores the populated `idiom_gaps` failure-name baseline, and
reveals the named red populations that the same defect had hidden in nine
other integration targets. The library target remains green at 1,126 tests.

## Verification

- `idiom_sweep_script::gate_uses_the_terminal_outer_test_result_not_a_nested_result`
- `bash -n scripts/gate.sh`
- `scripts/gate.sh --record`
- a subsequent gate comparison against the recorded baseline
