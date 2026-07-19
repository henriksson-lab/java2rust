#!/usr/bin/env bash
set -u
set -o pipefail

usage() {
  cat >&2 <<'USAGE'
usage: scripts/run-focused-stage-gates.sh --primary <java-dir> --out <out-dir> [options]

Options:
  --stage <name>                    Stage to run: parser-resolver-semantic,
                                    representation, signature, compile, idiom, or all.
                                    Default: all.
  --source-root <mode:path>         Add a resolver-bench source root.
  --source-root-package <mode:path:package.prefix>
                                    Add a package-filtered resolver-bench source root.
  --facts <path>                    Add external fact overrides.
  --external-pack-manifest <path>   Add external pack facts.
  --builtin-external-pack <name>    Add a built-in external pack.
  --no-builtin-external-packs       Disable built-in external packs.
  --jobs <n>                        Worker count for compile gate. Default: nproc or 4.
  --help                            Show this help.

Output:
  <out>/stage-summary.tsv           One row per requested stage.
  <out>/resolver-bench.txt          Shared parser/resolver/semantic/
                                    representation/signature report.
  <out>/compile/summary.tsv         Generated-crate compile summary, when
                                    compile or idiom stages run.
USAGE
}

primary=""
out_dir=""
stage="all"
source_roots=()
source_root_packages=()
facts_paths=()
manifest_paths=()
builtin_packs=()
enable_builtin_packs=1
jobs="${JOBS:-}"

while [[ $# -gt 0 ]]; do
  case "$1" in
    --primary)
      [[ $# -ge 2 ]] || { echo "missing value for --primary" >&2; exit 2; }
      primary="$2"
      shift 2
      ;;
    --out)
      [[ $# -ge 2 ]] || { echo "missing value for --out" >&2; exit 2; }
      out_dir="$2"
      shift 2
      ;;
    --stage)
      [[ $# -ge 2 ]] || { echo "missing value for --stage" >&2; exit 2; }
      stage="$2"
      shift 2
      ;;
    --source-root)
      [[ $# -ge 2 ]] || { echo "missing value for --source-root" >&2; exit 2; }
      source_roots+=("$2")
      shift 2
      ;;
    --source-root-package)
      [[ $# -ge 2 ]] || { echo "missing value for --source-root-package" >&2; exit 2; }
      source_root_packages+=("$2")
      shift 2
      ;;
    --facts)
      [[ $# -ge 2 ]] || { echo "missing value for --facts" >&2; exit 2; }
      facts_paths+=("$2")
      shift 2
      ;;
    --external-pack-manifest)
      [[ $# -ge 2 ]] || { echo "missing value for --external-pack-manifest" >&2; exit 2; }
      manifest_paths+=("$2")
      shift 2
      ;;
    --builtin-external-pack)
      [[ $# -ge 2 ]] || { echo "missing value for --builtin-external-pack" >&2; exit 2; }
      builtin_packs+=("$2")
      shift 2
      ;;
    --no-builtin-external-packs)
      enable_builtin_packs=0
      shift
      ;;
    --jobs)
      [[ $# -ge 2 ]] || { echo "missing value for --jobs" >&2; exit 2; }
      jobs="$2"
      shift 2
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

if [[ -z "$primary" || -z "$out_dir" ]]; then
  usage
  exit 2
fi

if [[ ! -d "$primary" ]]; then
  echo "primary Java directory does not exist: $primary" >&2
  exit 1
fi

case "$stage" in
  parser-resolver-semantic|representation|signature|compile|idiom|all) ;;
  *)
    echo "unknown stage: $stage" >&2
    usage
    exit 2
    ;;
esac

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

summary_path="$out_dir/stage-summary.tsv"
resolver_report="$out_dir/resolver-bench.txt"
compile_out="$out_dir/compile"
printf "stage\tstatus\tmetric\tvalue\tlimit\treport\n" > "$summary_path"

counter() {
  local key="$1"
  local report="$2"
  awk -F '\t' -v key="$key" '$1 == key { print $2; found = 1; exit } END { if (!found) print 0 }' "$report"
}

append_stage() {
  local stage_name="$1"
  local status="$2"
  local metric="$3"
  local value="$4"
  local limit="$5"
  local report="$6"
  printf "%s\t%s\t%s\t%s\t%s\t%s\n" "$stage_name" "$status" "$metric" "$value" "$limit" "$report" >> "$summary_path"
}

resolver_args=("resolver-bench")
if [[ "$enable_builtin_packs" -eq 0 ]]; then
  resolver_args+=("--no-builtin-external-packs")
fi
for source_root in "${source_roots[@]}"; do
  resolver_args+=("--source-root" "$source_root")
done
for source_root_package in "${source_root_packages[@]}"; do
  resolver_args+=("--source-root-package" "$source_root_package")
done
for facts_path in "${facts_paths[@]}"; do
  resolver_args+=("--facts" "$facts_path")
done
for manifest_path in "${manifest_paths[@]}"; do
  resolver_args+=("--external-pack-manifest" "$manifest_path")
done
for pack_name in "${builtin_packs[@]}"; do
  resolver_args+=("--builtin-external-pack" "$pack_name")
done
resolver_args+=("$primary")

resolver_ran=0
ensure_resolver_report() {
  if [[ "$resolver_ran" -eq 0 ]]; then
    if "$binary" "${resolver_args[@]}" > "$resolver_report" 2>&1; then
      resolver_ran=1
    else
      resolver_ran=1
      return 1
    fi
  fi
}

gate_zero_counter() {
  local stage_name="$1"
  local metric="$2"
  local report="$3"
  local value
  value="$(counter "$metric" "$report")"
  if [[ "$value" == "0" ]]; then
    append_stage "$stage_name" "ok" "$metric" "$value" "0" "$report"
    return 0
  fi
  append_stage "$stage_name" "failed" "$metric" "$value" "0" "$report"
  return 1
}

run_parser_resolver_semantic_gate() {
  local status=0
  if ! ensure_resolver_report; then
    append_stage "parser-resolver-semantic" "failed" "resolver_bench_exit" "1" "0" "$resolver_report"
    return 1
  fi
  gate_zero_counter "parser-resolver-semantic" "parser_diagnostics" "$resolver_report" || status=1
  gate_zero_counter "parser-resolver-semantic" "resolver_diagnostics" "$resolver_report" || status=1
  gate_zero_counter "parser-resolver-semantic" "semantic_diagnostics" "$resolver_report" || status=1
  return "$status"
}

run_representation_gate() {
  if ! ensure_resolver_report; then
    append_stage "representation" "failed" "resolver_bench_exit" "1" "0" "$resolver_report"
    return 1
  fi
  gate_zero_counter "representation" "object_representation_diagnostics" "$resolver_report"
}

run_signature_gate() {
  if ! ensure_resolver_report; then
    append_stage "signature" "failed" "resolver_bench_exit" "1" "0" "$resolver_report"
    return 1
  fi
  local conflicts
  conflicts="$(counter "signature_join_conflicts" "$resolver_report")"
  append_stage "signature" "info" "signature_join_conflicts" "$conflicts" "triage" "$resolver_report"
  gate_zero_counter "signature" "signature_fixpoint_diagnostics" "$resolver_report"
}

compile_ran=0
compile_status=0
ensure_compile_report() {
  if [[ "$compile_ran" -eq 0 ]]; then
    compile_ran=1
    compile_args=("--primary" "$primary" "--out" "$compile_out" "--jobs" "$jobs")
    if [[ "$enable_builtin_packs" -eq 0 ]]; then
      compile_args+=("--no-builtin-external-packs")
    fi
    for source_root in "${source_roots[@]}"; do
      compile_args+=("--source-root" "$source_root")
    done
    for source_root_package in "${source_root_packages[@]}"; do
      compile_args+=("--source-root-package" "$source_root_package")
    done
    for facts_path in "${facts_paths[@]}"; do
      compile_args+=("--facts" "$facts_path")
    done
    for manifest_path in "${manifest_paths[@]}"; do
      compile_args+=("--external-pack-manifest" "$manifest_path")
    done
    for pack_name in "${builtin_packs[@]}"; do
      compile_args+=("--builtin-external-pack" "$pack_name")
    done
    if "$repo_root/scripts/translate-focused-package.sh" "${compile_args[@]}" > "$out_dir/compile-gate.stdout" 2> "$out_dir/compile-gate.stderr"; then
      compile_status=0
    else
      compile_status=1
    fi
  fi
  return "$compile_status"
}

summary_value() {
  local key="$1"
  local report="$2"
  awk -F '\t' -v key="$key" '$1 == key { print $2; found = 1; exit } END { if (!found) print 0 }' "$report"
}

run_compile_gate() {
  if ensure_compile_report; then
    append_stage "compile" "ok" "cargo_check_errors" "0" "0" "$compile_out/summary.tsv"
    return 0
  fi
  local errors
  errors="$(summary_value "cargo_check_errors" "$compile_out/summary.tsv")"
  append_stage "compile" "failed" "cargo_check_errors" "$errors" "0" "$compile_out/summary.tsv"
  return 1
}

run_idiom_gate() {
  if ensure_compile_report; then
    append_stage "idiom" "ok" "invalid_pattern_files" "0" "0" "$compile_out/summary.tsv"
    return 0
  fi
  local invalid_files
  invalid_files="$(summary_value "invalid_pattern_files" "$compile_out/summary.tsv")"
  if [[ "$invalid_files" == "0" ]]; then
    append_stage "idiom" "ok" "invalid_pattern_files" "$invalid_files" "0" "$compile_out/summary.tsv"
    return 0
  fi
  append_stage "idiom" "failed" "invalid_pattern_files" "$invalid_files" "0" "$compile_out/summary.tsv"
  return 1
}

exit_status=0
case "$stage" in
  parser-resolver-semantic)
    run_parser_resolver_semantic_gate || exit_status=1
    ;;
  representation)
    run_representation_gate || exit_status=1
    ;;
  signature)
    run_signature_gate || exit_status=1
    ;;
  compile)
    run_compile_gate || exit_status=1
    ;;
  idiom)
    run_idiom_gate || exit_status=1
    ;;
  all)
    run_parser_resolver_semantic_gate || exit_status=1
    run_representation_gate || exit_status=1
    run_signature_gate || exit_status=1
    run_compile_gate || exit_status=1
    run_idiom_gate || exit_status=1
    ;;
esac

cat "$summary_path"
exit "$exit_status"
