#!/usr/bin/env bash
set -u
set -o pipefail

usage() {
  cat >&2 <<'USAGE'
usage: scripts/run-focused-batch-stability-gate.sh --primary <java-dir> --out <out-dir> [--jobs <n>] [translate-focused-package options]

Runs the focused package gate twice:

  <out-dir>/serial    with --jobs 1
  <out-dir>/parallel  with --jobs <n>, default 10

Then compares normalized batch manifests, status rows, invalid-pattern tables,
top compile errors, and key summary fields.

Options are forwarded to scripts/translate-focused-package.sh except --out and
--jobs, which are controlled by this script.
USAGE
}

primary=""
out_dir=""
jobs="${JOBS:-10}"
forward_args=()

while [[ $# -gt 0 ]]; do
  case "$1" in
    --primary)
      [[ $# -ge 2 ]] || { echo "missing value for --primary" >&2; exit 2; }
      primary="$2"
      forward_args+=("--primary" "$2")
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
    --help|-h)
      usage
      exit 0
      ;;
    *)
      forward_args+=("$1")
      shift
      ;;
  esac
done

if [[ -z "$primary" || -z "$out_dir" ]]; then
  usage
  exit 2
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
serial_dir="$out_dir/serial"
parallel_dir="$out_dir/parallel"
report_path="$out_dir/stability-report.tsv"

mkdir -p "$out_dir"

serial_status=0
parallel_status=0
"$repo_root/scripts/translate-focused-package.sh" "${forward_args[@]}" --out "$serial_dir" --jobs 1 \
  > "$out_dir/serial.stdout" 2> "$out_dir/serial.stderr" || serial_status="$?"
"$repo_root/scripts/translate-focused-package.sh" "${forward_args[@]}" --out "$parallel_dir" --jobs "$jobs" \
  > "$out_dir/parallel.stdout" 2> "$out_dir/parallel.stderr" || parallel_status="$?"

compare_status=0
python3 "$repo_root/scripts/compare-focused-batch-artifacts.py" \
  --serial-dir "$serial_dir" \
  --parallel-dir "$parallel_dir" \
  --out "$report_path" > "$out_dir/compare.stdout" 2> "$out_dir/compare.stderr" || compare_status="$?"

{
  printf "serial_exit\t%s\n" "$serial_status"
  printf "parallel_exit\t%s\n" "$parallel_status"
  printf "compare_exit\t%s\n" "$compare_status"
} >> "$report_path"

cat "$report_path"

if [[ "$serial_status" -ne 0 || "$parallel_status" -ne 0 || "$compare_status" -ne 0 ]]; then
  exit 1
fi
