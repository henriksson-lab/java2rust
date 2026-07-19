#!/usr/bin/env bash
set -u
set -o pipefail

usage() {
  cat >&2 <<'USAGE'
usage: scripts/run-gatk-resolver-benchmarks.sh [options]

Options:
  --gatk-root <dir>        GATK Java source root. Default:
                           /home/mahogny/github/claude/java2rust/testdata/gatk/src/main/java
  --out <dir>              Output directory. Default: .tmp/gatk-resolver-benchmarks
  --jobs <n>               Focused envelope benchmarks to run in parallel. Default: nproc or 4.
  --skip-full              Run only focused package envelopes.
  --only-full              Run only the full GATK benchmark.
  --help                   Show this help.

Output:
  <out>/summary.tsv        One row per scenario with extracted counters.
  <out>/<scenario>.txt     Raw resolver-bench report for each scenario.
  <out>/<scenario>.time    Wall-clock timing from /usr/bin/time when available.
USAGE
}

gatk_root="/home/mahogny/github/claude/java2rust/testdata/gatk/src/main/java"
out_dir=".tmp/gatk-resolver-benchmarks"
jobs="${JOBS:-}"
run_full=1
run_focused=1

while [[ $# -gt 0 ]]; do
  case "$1" in
    --gatk-root)
      [[ $# -ge 2 ]] || { echo "missing value for --gatk-root" >&2; exit 2; }
      gatk_root="$2"
      shift 2
      ;;
    --out)
      [[ $# -ge 2 ]] || { echo "missing value for --out" >&2; exit 2; }
      out_dir="$2"
      shift 2
      ;;
    --jobs)
      [[ $# -ge 2 ]] || { echo "missing value for --jobs" >&2; exit 2; }
      jobs="$2"
      shift 2
      ;;
    --skip-full)
      run_full=0
      shift
      ;;
    --only-full)
      run_focused=0
      shift
      ;;
    --help|-h)
      usage
      exit 0
      ;;
    *)
      echo "unknown argument: $1" >&2
      usage
      exit 2
      ;;
  esac
done

if [[ ! -d "$gatk_root" ]]; then
  echo "GATK Java source root does not exist: $gatk_root" >&2
  exit 1
fi

if [[ -z "$jobs" ]]; then
  if command -v nproc >/dev/null 2>&1; then
    jobs="$(nproc)"
  else
    jobs=4
  fi
fi

case "$jobs" in
  ''|*[!0-9]*)
    echo "--jobs must be a positive integer: $jobs" >&2
    exit 2
    ;;
esac
if [[ "$jobs" -lt 1 ]]; then
  echo "--jobs must be at least 1" >&2
  exit 2
fi

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
binary="$repo_root/target/debug/new-java2rust"
mkdir -p "$out_dir"

if [[ ! -x "$binary" ]]; then
  cargo build --quiet --bin new-java2rust
fi

summary_path="$out_dir/summary.tsv"
summary_fragments_dir="$out_dir/summary-fragments"
mkdir -p "$summary_fragments_dir"
find "$summary_fragments_dir" -type f -name '*.tsv' -delete
printf "scenario\tstatus\twall_seconds\tfiles_seen\tparser_diagnostics\tresolver_diagnostics\tsemantic_diagnostics\tobject_representation_diagnostics\tunexplained_method_calls\tunexplained_constructors\tunexplained_field_accesses\tsource_dependency_methods\tsource_dependency_constructors\tsource_dependency_fields\telapsed_ms\tfiles_per_second_x1000\treport\n" > "$summary_path"

counter() {
  local key="$1"
  local report="$2"
  awk -F '\t' -v key="$key" '$1 == key { print $2; found = 1; exit } END { if (!found) print "" }' "$report"
}

wall_seconds() {
  local time_path="$1"
  if [[ -s "$time_path" ]]; then
    awk -F '\t' '$1 == "wall_seconds" { print $2; found = 1; exit } END { if (!found) print "" }' "$time_path"
  else
    printf ""
  fi
}

append_summary() {
  local destination="$1"
  shift
  local scenario="$1"
  local status="$2"
  local report="$3"
  local time_path="$4"
  printf "%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\n" \
    "$scenario" \
    "$status" \
    "$(wall_seconds "$time_path")" \
    "$(counter files_seen "$report")" \
    "$(counter parser_diagnostics "$report")" \
    "$(counter resolver_diagnostics "$report")" \
    "$(counter semantic_diagnostics "$report")" \
    "$(counter object_representation_diagnostics "$report")" \
    "$(counter unexplained_method_calls "$report")" \
    "$(counter unexplained_constructors "$report")" \
    "$(counter unexplained_field_accesses "$report")" \
    "$(counter source_dependency_resolved_method_calls "$report")" \
    "$(counter source_dependency_resolved_constructors "$report")" \
    "$(counter source_dependency_resolved_field_accesses "$report")" \
    "$(counter elapsed_ms "$report")" \
    "$(counter files_per_second_x1000 "$report")" \
    "$report" >> "$destination"
}

run_benchmark() {
  local scenario="$1"
  local summary_fragment="$2"
  shift
  shift
  local report="$out_dir/$scenario.txt"
  local time_path="$out_dir/$scenario.time"
  local status="ok"
  if command -v /usr/bin/time >/dev/null 2>&1; then
    if ! /usr/bin/time -f "wall_seconds\t%e" -o "$time_path" "$binary" resolver-bench "$@" > "$report" 2>&1; then
      status="failed"
    fi
  else
    if ! "$binary" resolver-bench "$@" > "$report" 2>&1; then
      status="failed"
    fi
    : > "$time_path"
  fi
  append_summary "$summary_fragment" "$scenario" "$status" "$report" "$time_path"
}

focused_scenarios=(
  "focused_genomicsdb|$gatk_root/org/broadinstitute/hellbender/tools/genomicsdb"
  "focused_spark_sv_evidence|--source-root|source-summary:$gatk_root/org/broadinstitute/hellbender/exceptions|--source-root|source-summary:$gatk_root/org/broadinstitute/hellbender/tools/spark/sv|$gatk_root/org/broadinstitute/hellbender/tools/spark/sv/evidence"
  "focused_variantutils|--source-root|source-summary:$gatk_root/org/broadinstitute/hellbender/engine|--source-root|source-summary:$gatk_root/org/broadinstitute/hellbender/utils|--source-root|source-summary:$gatk_root/org/broadinstitute/hellbender/cmdline/argumentcollections|--source-root|source-summary:$gatk_root/org/broadinstitute/hellbender/tools/walkers/annotator|--source-root|source-summary:$gatk_root/org/broadinstitute/hellbender/tools/walkers/haplotypecaller|--source-root|source-summary:$gatk_root/org/broadinstitute/hellbender/tools/walkers/genotyper|--source-root|source-summary:$gatk_root/org/broadinstitute/hellbender/exceptions|--source-root|source-summary:$gatk_root/org/broadinstitute/hellbender/tools/genomicsdb|$gatk_root/org/broadinstitute/hellbender/tools/walkers/variantutils"
)

if [[ "$run_full" -eq 1 ]]; then
  run_benchmark "full_gatk" "$summary_fragments_dir/full_gatk.tsv" "$gatk_root"
fi

if [[ "$run_focused" -eq 1 ]]; then
  active_jobs=0
  for scenario_spec in "${focused_scenarios[@]}"; do
    IFS='|' read -r -a parts <<< "$scenario_spec"
    scenario="${parts[0]}"
    args=("${parts[@]:1}")
    (
      run_benchmark "$scenario" "$summary_fragments_dir/$scenario.tsv" "${args[@]}"
    ) &
    active_jobs="$((active_jobs + 1))"
    if [[ "$active_jobs" -ge "$jobs" ]]; then
      wait -n
      active_jobs="$((active_jobs - 1))"
    fi
  done
  while [[ "$active_jobs" -gt 0 ]]; do
    wait -n
    active_jobs="$((active_jobs - 1))"
  done
fi

for fragment_path in "$summary_fragments_dir"/*.tsv; do
  [[ -e "$fragment_path" ]] || continue
  cat "$fragment_path" >> "$summary_path"
done

printf "wrote %s\n" "$summary_path"
