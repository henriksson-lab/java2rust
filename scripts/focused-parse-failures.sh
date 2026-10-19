#!/usr/bin/env bash
set -u
set -o pipefail

compiler_diagnostic_error_headers() {
  local compile_log="$1"
  # Select headers, not quoted source text or Cargo/rustc summary footers.
  awk '
    {
      line = $0
      gsub(/\033\[[0-9;]*m/, "", line)
      if (line ~ /^error(\[[A-Z0-9]+\])?:/ &&
          line !~ /^error: (could not compile |aborting due to )/) print line
    }
  ' "$compile_log"
}

compiler_diagnostic_error_count() {
  compiler_diagnostic_error_headers "$1" | awk 'END { print NR + 0 }'
}

write_parse_failures() {
  local compile_log="$1"
  local out_path="$2"
  {
    printf "error\tcount\n"
    compiler_diagnostic_error_headers "$compile_log" \
      | grep -E '^error: (expected|unknown start of token|mismatched closing delimiter|unexpected closing delimiter|this file contains an unclosed delimiter|suffixes on string literals are invalid|unterminated|character literal may only contain one codepoint|invalid suffix)' \
      | sed 's/[[:space:]]\+/ /g' \
      | sort \
      | uniq -c \
      | sort -nr \
      | awk '{ count = $1; $1 = ""; sub(/^ /, ""); gsub(/\t/, " "); printf "%s\t%s\n", $0, count }'
  } > "$out_path"
}

parse_failure_count_from_report() {
  local path="$1"
  awk -F '\t' 'NR > 1 { count += $2 } END { print count + 0 }' "$path"
}

cargo_type_errors_valid_from_parse_failure_count() {
  local parse_failure_count="$1"
  if [[ "$parse_failure_count" -gt 0 ]]; then
    printf "0\n"
  else
    printf "1\n"
  fi
}

focused_parse_failures_main() {
  local compile_log=""
  local out_path=""
  while [[ $# -gt 0 ]]; do
    case "$1" in
      --compile-log)
        [[ $# -ge 2 ]] || { echo "missing value for --compile-log" >&2; return 2; }
        compile_log="$2"
        shift 2
        ;;
      --out)
        [[ $# -ge 2 ]] || { echo "missing value for --out" >&2; return 2; }
        out_path="$2"
        shift 2
        ;;
      --help)
        echo "usage: scripts/focused-parse-failures.sh --compile-log <path> --out <path>"
        return 0
        ;;
      *)
        echo "unknown argument: $1" >&2
        return 2
        ;;
    esac
  done
  [[ -n "$compile_log" ]] || { echo "missing --compile-log" >&2; return 2; }
  [[ -n "$out_path" ]] || { echo "missing --out" >&2; return 2; }
  write_parse_failures "$compile_log" "$out_path"
  local parse_failure_count
  parse_failure_count="$(parse_failure_count_from_report "$out_path")"
  printf "parse_failures\t%s\n" "$parse_failure_count"
  printf "cargo_type_errors_valid\t%s\n" "$(cargo_type_errors_valid_from_parse_failure_count "$parse_failure_count")"
}

if [[ "${BASH_SOURCE[0]}" == "$0" ]]; then
  focused_parse_failures_main "$@"
fi
