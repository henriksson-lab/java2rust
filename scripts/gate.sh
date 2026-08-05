#!/usr/bin/env bash
# Run every test target and diff against the checked-in baseline.
#
# This exists because a hand-maintained list of suites is the same hazard as a
# hand-maintained list of marker strings: it silently stops covering things.
# A gate that iterated 13 named suites missed `output_markers` and reported a
# batch green while that suite was red — see FROMCLAUDE.md. Targets are
# discovered from `tests/*.rs`, so adding a suite cannot leave it ungated.
#
# Usage:
#   scripts/gate.sh                 # every target except the slow one
#   scripts/gate.sh --all           # including idiom_gaps (~10 min)
#   scripts/gate.sh --record        # rewrite the full baseline, including slow targets
#
# Everything here runs --release. That matters less than it looks for the slow
# target: `idiom_gaps` spends most of its wall clock in ~290 *nested* `cargo`
# runs over generated crates, so the outer profile is not the bottleneck. If that
# suite needs to get faster, the nested invocations are where to look.
#
# Exit status is non-zero if any target moved from its baseline.

set -uo pipefail
cd "$(dirname "$0")/.."

BASELINE=tests/baselines/suite-results.tsv
SLOW_TARGETS="idiom_gaps"
RECORD=0
INCLUDE_SLOW=0

for arg in "$@"; do
  case "$arg" in
    --record) RECORD=1 ;;
    --all) INCLUDE_SLOW=1 ;;
    *) echo "unknown flag: $arg" >&2; exit 2 ;;
  esac
done

if [ "$RECORD" -eq 1 ]; then
  INCLUDE_SLOW=1
fi

# The differential harness prefers target/release/new-java2rust when it exists,
# so a stale binary would silently test the previous translator.
echo "building release binary and test targets..." >&2
cargo build --release --tests >/dev/null 2>&1 || {
  echo "build failed" >&2
  exit 1
}

targets=$(find tests -maxdepth 1 -name '*.rs' -printf '%f\n' | sed 's/\.rs$//' | sort)

results=$(mktemp)
for target in $targets; do
  if [ "$INCLUDE_SLOW" -eq 0 ] && echo "$SLOW_TARGETS" | grep -qw "$target"; then
    continue
  fi
  line=$(cargo test --release --test "$target" 2>&1 | grep '^test result' | head -1)
  passed=$(echo "$line" | sed -n 's/.*result: [a-zA-Z]*\. \([0-9]*\) passed.*/\1/p')
  failed=$(echo "$line" | sed -n 's/.*passed; \([0-9]*\) failed.*/\1/p')
  printf '%s\t%s\t%s\n' "$target" "${passed:-?}" "${failed:-?}" >> "$results"
done

lib_line=$(cargo test --release --lib 2>&1 | grep '^test result' | head -1)
lib_passed=$(echo "$lib_line" | sed -n 's/.*result: [a-zA-Z]*\. \([0-9]*\) passed.*/\1/p')
lib_failed=$(echo "$lib_line" | sed -n 's/.*passed; \([0-9]*\) failed.*/\1/p')
printf '%s\t%s\t%s\n' "--lib" "${lib_passed:-?}" "${lib_failed:-?}" >> "$results"

if [ "$RECORD" -eq 1 ]; then
  mkdir -p "$(dirname "$BASELINE")"
  {
    echo "# target	passed	failed"
    echo "# Recorded by scripts/gate.sh --record. Diff, do not eyeball."
    cat "$results"
  } > "$BASELINE"
  echo "recorded $(wc -l < "$results") targets to $BASELINE"
  rm -f "$results"
  exit 0
fi

if [ ! -f "$BASELINE" ]; then
  echo "no baseline at $BASELINE; run: scripts/gate.sh --record" >&2
  cat "$results"
  rm -f "$results"
  exit 2
fi

status=0
if [ "$INCLUDE_SLOW" -eq 0 ]; then
  echo "NOTE: fast gate — skipping:$(for t in $SLOW_TARGETS; do printf ' %s' "$t"; done). Use --all to include." >&2
fi
printf '%-24s %-14s %-14s %s\n' TARGET BASELINE NOW DELTA
while IFS=$'\t' read -r target passed failed; do
  base=$(grep -P "^\Q$target\E\t" "$BASELINE" || true)
  if [ -z "$base" ]; then
    printf '%-24s %-14s %-14s %s\n' "$target" "-" "$passed/$failed" "<<< NEW TARGET, not in baseline"
    status=1
    continue
  fi
  bp=$(echo "$base" | cut -f2)
  bf=$(echo "$base" | cut -f3)
  delta=""
  if [ "$passed" != "$bp" ] || [ "$failed" != "$bf" ]; then
    delta="<<< CHANGED"
    status=1
  fi
  printf '%-24s %-14s %-14s %s\n' "$target" "$bp/$bf" "$passed/$failed" "$delta"
done < "$results"

# A target in the baseline that no longer runs is also a gate failure.
while IFS=$'\t' read -r target _ _; do
  case "$target" in \#*) continue ;; esac
  if ! grep -qP "^\Q$target\E\t" "$results"; then
    if [ "$INCLUDE_SLOW" -eq 0 ] && echo "$SLOW_TARGETS" | grep -qw "$target"; then
      printf '%-24s %-14s %-14s %s\n' "$target" "-" "skipped" "(slow; use --all)"
      continue
    fi
    printf '%-24s %-14s %-14s %s\n' "$target" "-" "MISSING" "<<< target disappeared"
    status=1
  fi
done < "$BASELINE"

rm -f "$results"
exit "$status"
