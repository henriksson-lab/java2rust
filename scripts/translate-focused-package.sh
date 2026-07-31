#!/usr/bin/env bash
set -u
set -o pipefail

usage() {
  cat >&2 <<'USAGE'
usage: scripts/translate-focused-package.sh --primary <java-dir> --out <out-dir> [options]

Options:
  --source-root <mode:path>           Add a resolver-bench source root preflight input.
  --source-root-package <mode:path:package.prefix>
                                      Add a package-filtered resolver-bench source root preflight input.
  --facts <path>                      Add external fact overrides to translation and preflight.
  --external-pack-manifest <path>     Add external pack facts to translation and preflight.
  --builtin-external-pack <name>      Add a built-in external pack to resolver preflight and translation.
  --no-builtin-external-packs         Disable built-in external packs in resolver preflight.
  --skip-resolver-bench               Do not run resolver-bench before translation.
  --jobs <n>                          Parallelism for batch emission or legacy per-file workers. Default: 10.
  --no-batch-translate                Use legacy per-file worker translation.
  --translate-timeout-sec <n>         Mark a single Java file translation failed after n seconds.
  --verbose-cargo                     Run cargo check without --quiet.
  --offline-cargo                     Run cargo check with --offline.
  --help                              Show this help.

Output:
  <out-dir>/generated-crate/Cargo.toml
  <out-dir>/generated-crate/src/generated/*.rs
  <out-dir>/status.tsv
  <out-dir>/batch-manifest.tsv
  <out-dir>/compile.log
  <out-dir>/parse-failures.tsv
  <out-dir>/invalid-patterns.tsv
  <out-dir>/idiom-sweep.md
  <out-dir>/idiom-sweep.tsv
  <out-dir>/top-compile-errors.tsv
  <out-dir>/slowest-phases.tsv
  <out-dir>/slowest-workers.tsv
  <out-dir>/phase-events.tsv
  <out-dir>/worker-events.tsv
  <out-dir>/resolver-bench.txt, when resolver preflight runs
USAGE
}

primary=""
out_dir=""
source_roots=()
source_root_packages=()
facts_paths=()
manifest_paths=()
builtin_packs=()
enable_builtin_packs=1
run_resolver_bench=1
jobs="${JOBS:-10}"
translate_timeout_sec="${TRANSLATE_TIMEOUT_SEC:-}"
verbose_cargo=0
offline_cargo=0
batch_translate="${BATCH_TRANSLATE:-1}"

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
    --skip-resolver-bench)
      run_resolver_bench=0
      shift
      ;;
    --jobs)
      [[ $# -ge 2 ]] || { echo "missing value for --jobs" >&2; exit 2; }
      jobs="$2"
      shift 2
      ;;
    --no-batch-translate)
      batch_translate=0
      shift
      ;;
    --translate-timeout-sec)
      [[ $# -ge 2 ]] || { echo "missing value for --translate-timeout-sec" >&2; exit 2; }
      translate_timeout_sec="$2"
      shift 2
      ;;
    --verbose-cargo)
      verbose_cargo=1
      shift
      ;;
    --offline-cargo)
      offline_cargo=1
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

if [[ -z "$primary" || -z "$out_dir" ]]; then
  usage
  exit 2
fi

if [[ ! -d "$primary" ]]; then
  echo "primary Java directory does not exist: $primary" >&2
  exit 1
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
case "$translate_timeout_sec" in
  ''|*[!0-9]*)
    if [[ -n "$translate_timeout_sec" ]]; then
      echo "--translate-timeout-sec must be a positive integer: $translate_timeout_sec" >&2
      exit 2
    fi
    ;;
esac
if [[ -n "$translate_timeout_sec" && "$translate_timeout_sec" -lt 1 ]]; then
  echo "--translate-timeout-sec must be at least 1" >&2
  exit 2
fi

now_ms() {
  date +%s%3N
}

elapsed_ms_since() {
  local start_ms="$1"
  local end_ms
  end_ms="$(now_ms)"
  printf '%s' "$((end_ms - start_ms))"
}

write_ndarray_shim() {
  local shim_dir="$1"
  mkdir -p "$shim_dir/src"
  cat > "$shim_dir/Cargo.toml" <<'TOML'
[package]
name = "ndarray"
version = "0.0.0"
edition = "2021"

[lib]
path = "src/lib.rs"
TOML
  cat > "$shim_dir/src/lib.rs" <<'RUST'
use std::ops::{Index, IndexMut};

#[derive(Clone, Debug, PartialEq)]
pub struct Array2<T> {
    rows: usize,
    cols: usize,
    data: Vec<T>,
}

impl<T: Clone> Array2<T> {
    pub fn from_elem(shape: (usize, usize), value: T) -> Self {
        let (rows, cols) = shape;
        Self {
            rows,
            cols,
            data: vec![value; rows * cols],
        }
    }

    pub fn from_rows(rows: &[&[T]]) -> Self {
        let row_count = rows.len();
        let cols = rows.first().map_or(0, |row| row.len());
        let mut data = Vec::with_capacity(row_count * cols);
        for row in rows {
            data.extend_from_slice(row);
        }
        Self {
            rows: row_count,
            cols,
            data,
        }
    }
}

impl<T> Array2<T> {
    pub fn nrows(&self) -> usize {
        self.rows
    }

    pub fn ncols(&self) -> usize {
        self.cols
    }

    pub fn row(&self, row: usize) -> ArrayView1<'_, T> {
        let start = row * self.cols;
        ArrayView1 {
            data: &self.data[start..start + self.cols],
        }
    }

    pub fn row_mut(&mut self, row: usize) -> RowMut<'_, T> {
        let start = row * self.cols;
        RowMut {
            data: &mut self.data[start..start + self.cols],
        }
    }

    pub fn iter(&self) -> Rows<'_, T> {
        Rows {
            matrix: self,
            row: 0,
        }
    }
}

impl<T> Index<[usize; 2]> for Array2<T> {
    type Output = T;

    fn index(&self, index: [usize; 2]) -> &Self::Output {
        &self.data[index[0] * self.cols + index[1]]
    }
}

impl<T> IndexMut<[usize; 2]> for Array2<T> {
    fn index_mut(&mut self, index: [usize; 2]) -> &mut Self::Output {
        &mut self.data[index[0] * self.cols + index[1]]
    }
}

#[derive(Clone, Copy, Debug)]
pub struct ArrayView1<'a, T> {
    data: &'a [T],
}

impl<'a, T> ArrayView1<'a, T> {
    pub fn from(data: &'a [T]) -> Self {
        Self { data }
    }

    pub fn to_vec(&self) -> Vec<T>
    where
        T: Clone,
    {
        self.data.to_vec()
    }
}

impl<'a, T> From<&'a Vec<T>> for ArrayView1<'a, T> {
    fn from(data: &'a Vec<T>) -> Self {
        Self {
            data: data.as_slice(),
        }
    }
}

impl<'a, T> From<&'a [T]> for ArrayView1<'a, T> {
    fn from(data: &'a [T]) -> Self {
        Self { data }
    }
}

pub struct RowMut<'a, T> {
    data: &'a mut [T],
}

impl<'a, T: Clone> RowMut<'a, T> {
    pub fn assign(&mut self, source: &ArrayView1<'_, T>) {
        self.data.clone_from_slice(source.data);
    }
}

pub struct Rows<'a, T> {
    matrix: &'a Array2<T>,
    row: usize,
}

impl<'a, T> Iterator for Rows<'a, T> {
    type Item = &'a [T];

    fn next(&mut self) -> Option<Self::Item> {
        if self.row >= self.matrix.rows {
            return None;
        }
        let start = self.row * self.matrix.cols;
        self.row += 1;
        Some(&self.matrix.data[start..start + self.matrix.cols])
    }
}

pub fn arr2<T: Clone, const R: usize, const C: usize>(rows: &[[T; C]; R]) -> Array2<T> {
    let mut data = Vec::with_capacity(R * C);
    for row in rows {
        data.extend_from_slice(row);
    }
    Array2 {
        rows: R,
        cols: C,
        data,
    }
}
RUST
}

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
source "$repo_root/scripts/focused-parse-failures.sh"
if [[ -n "${NEW_JAVA2RUST_BIN:-}" ]]; then
  binary="$NEW_JAVA2RUST_BIN"
  build_profile_args=()
else
  binary="$repo_root/target/release/new-java2rust"
  build_profile_args=("--release")
fi
crate_dir="$out_dir/generated-crate"
generated_dir="$crate_dir/src/generated"
status_path="$out_dir/status.tsv"
batch_manifest_path="$out_dir/batch-manifest.tsv"
analysis_profile_path="$out_dir/analysis-profile.tsv"
compile_log="$out_dir/compile.log"
invalid_patterns_path="$out_dir/invalid-patterns.tsv"
parse_failures_path="$out_dir/parse-failures.tsv"
top_compile_errors="$out_dir/top-compile-errors.tsv"
resolver_log="$out_dir/resolver-bench.txt"
translate_log_dir="$out_dir/translation-logs"
idiom_sweep_tsv="$out_dir/idiom-sweep.tsv"
idiom_sweep_md="$out_dir/idiom-sweep.md"
phase_events_path="$out_dir/phase-events.tsv"
worker_events_path="$out_dir/worker-events.tsv"
slowest_phases_path="$out_dir/slowest-phases.tsv"
slowest_workers_path="$out_dir/slowest-workers.tsv"
script_start_ms="$(now_ms)"

mkdir -p "$generated_dir" "$translate_log_dir"
rm -f "$status_path" "$batch_manifest_path" "$analysis_profile_path" "$compile_log" "$invalid_patterns_path" "$parse_failures_path" "$top_compile_errors" "$resolver_log" "$idiom_sweep_tsv" "$idiom_sweep_md" "$phase_events_path" "$worker_events_path" "$slowest_phases_path" "$slowest_workers_path"
find "$generated_dir" -type f -name '*.rs' -delete
printf "phase\tstatus\telapsed_ms\tdetail\n" > "$phase_events_path"
printf "event\tpid\tindex\tsource\telapsed_ms\tstatus\trust_path\n" > "$worker_events_path"

if [[ ! -x "$binary" ]]; then
  if [[ -n "${NEW_JAVA2RUST_BIN:-}" ]]; then
    echo "NEW_JAVA2RUST_BIN is not executable: $binary" >&2
    exit 1
  fi
  cargo build --quiet "${build_profile_args[@]}" --bin new-java2rust
fi

external_dependency_lines=""
if [[ "${#manifest_paths[@]}" -gt 0 ]]; then
  if ! external_dependency_lines="$(python3 "$repo_root/scripts/extract-external-pack-cargo-deps.py" "${manifest_paths[@]}")"; then
    echo "failed to extract Cargo dependencies from external-pack manifests" >&2
    exit 1
  fi
fi

external_exception_conversion_impls=""
if [[ "${#manifest_paths[@]}" -gt 0 ]]; then
  if ! external_exception_conversion_impls="$(python3 "$repo_root/scripts/extract-external-pack-exception-conversions.py" "${manifest_paths[@]}")"; then
    echo "failed to extract JavaException conversions from external-pack manifests" >&2
    exit 1
  fi
fi

cat > "$crate_dir/Cargo.toml" <<'TOML'
[package]
name = "new-java2rust-focused-output"
version = "0.0.0"
edition = "2021"

[dependencies]
TOML
if [[ -n "$external_dependency_lines" ]]; then
  printf '%s\n' "$external_dependency_lines" >> "$crate_dir/Cargo.toml"
fi

cat > "$crate_dir/README.md" <<'README'
# Focused Generated Rust Check

This crate is generated by `scripts/translate-focused-package.sh`.

Each Java input file is emitted into one Rust module. Translation receives the
focused package as source context so source method signatures and overload
names can be shared across sibling modules.
README

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

resolver_status="skipped"
resolver_elapsed_ms=0
if [[ "$run_resolver_bench" -eq 1 ]]; then
  resolver_start_ms="$(now_ms)"
  if "$binary" "${resolver_args[@]}" > "$resolver_log" 2>&1; then
    resolver_status="ok"
  else
    resolver_status="failed"
  fi
  resolver_elapsed_ms="$(elapsed_ms_since "$resolver_start_ms")"
fi
printf "resolver_bench\t%s\t%s\t%s\n" "$resolver_status" "$resolver_elapsed_ms" "$resolver_log" >> "$phase_events_path"

printf "source\trust_path\ttranslate_status\tdiagnostics\tinvalid_patterns\tunsupported_members\n" > "$status_path"
mapfile -t java_files < <(find "$primary" -type f -name '*.java' | sort)

if [[ "${#java_files[@]}" -eq 0 ]]; then
  echo "no Java files found under: $primary" >&2
  exit 1
fi

count_concrete_none_assignments() {
  local path="$1"
  grep -R -E '= None;' "$path" \
    | grep -v -E 'let mut (r#)?[A-Za-z_][A-Za-z0-9_]*: Option<.*> = None;' \
    | wc -l \
    | tr -d ' '
}

count_concrete_none_assignment_files() {
  local path="$1"
  grep -R -l -E '= None;' "$path" \
    | while IFS= read -r candidate; do
        if grep -E '= None;' "$candidate" \
          | grep -q -v -E 'let mut (r#)?[A-Za-z_][A-Za-z0-9_]*: Option<.*> = None;'; then
          printf '%s\n' "$candidate"
        fi
      done \
    | wc -l \
    | tr -d ' '
}

idiom_signal_count() {
  local path="$1"
  local signal="$2"
  awk -F '\t' -v signal="$signal" 'NR > 1 && $1 == signal { found = 1; print $2 } END { if (!found) print 0 }' "$path"
}

status_column_sum() {
  local path="$1"
  local column="$2"
  awk -F '\t' -v column="$column" '
    NR == 1 {
      for (i = 1; i <= NF; i++) {
        if ($i == column) {
          target = i
        }
      }
      next
    }
    target > 0 {
      sum += $target
    }
    END {
      print sum + 0
    }
  ' "$path"
}

translate_one() {
  local index="$1"
  local java_file="$2"
  local binary="$3"
  local out_dir="$4"
  local generated_dir="$5"
  local log_dir="$6"
  local event_path="$7"
  local timeout_sec="$8"
  shift 8
  local translate_args=("$@")
  local stem
  stem="$(printf '%05d_%s' "$index" "$(basename "${java_file%.java}")")"
  stem="$(printf '%s' "$stem" | tr -c 'A-Za-z0-9_' '_')"
  local rust_path="$generated_dir/$stem.rs"
  local log_path="$log_dir/$stem.stderr"
  local tmp_path="$out_dir/$stem.rs.tmp"
  local status="ok"
  local start_ms elapsed_ms exit_code
  start_ms="$(now_ms)"
  printf "translate_start\t%s\t%s\t%s\t0\trunning\t%s\n" "$BASHPID" "$index" "$java_file" "$rust_path" >> "$event_path"
  if [[ -n "$timeout_sec" ]]; then
    timeout "$timeout_sec" "$binary" "${translate_args[@]}" "$java_file" > "$tmp_path" 2> "$log_path"
    exit_code="$?"
  else
    "$binary" "${translate_args[@]}" "$java_file" > "$tmp_path" 2> "$log_path"
    exit_code="$?"
  fi
  if [[ "$exit_code" -eq 0 ]]; then
    mv "$tmp_path" "$rust_path"
  else
    status="failed"
    if [[ "$exit_code" -eq 124 ]]; then
      status="timeout"
    fi
    mv "$tmp_path" "$rust_path" 2>/dev/null || touch "$rust_path"
  fi
  elapsed_ms="$(elapsed_ms_since "$start_ms")"
  printf "translate_end\t%s\t%s\t%s\t%s\t%s\t%s\n" "$BASHPID" "$index" "$java_file" "$elapsed_ms" "$status" "$rust_path" >> "$event_path"
  local diagnostics invalid_patterns unsupported_members
  diagnostics="$(wc -l < "$log_path" | tr -d ' ')"
  unsupported_members="$(grep -E '(^|[[:space:]])unsupported_member:' "$log_path" | wc -l | tr -d ' ')"
  invalid_patterns="$(
    {
      grep -E 'compile_error!|downcast::<.*>\(\)\.expect\("invalid Java cast"\)|\.add_all\(|\.put\(|unsupported Java lambda|unsupported Java method reference|unsupported Java array creation' "$rust_path" || true
      grep -E '= None;' "$rust_path" \
        | grep -v -E 'let mut [A-Za-z_][A-Za-z0-9_]*: Option<.*> = None;' || true
    } | wc -l | tr -d ' '
  )"
  printf "%s\t%s\t%s\t%s\t%s\t%s\n" "$java_file" "$rust_path" "$status" "$diagnostics" "$invalid_patterns" "$unsupported_members"
}

export -f translate_one
export -f now_ms
export -f elapsed_ms_since

translate_args=()
if [[ "$enable_builtin_packs" -eq 0 ]]; then
  translate_args+=("--no-builtin-external-packs")
fi
translate_args+=("--context-dir" "$primary")
translate_args+=("--context-dir-emitted")
for source_root in "${source_roots[@]}"; do
  case "$source_root" in
    source-summary:*|source_summary:*|summary:*)
      translate_args+=("--context-source-summary-dir" "${source_root#*:}")
      ;;
  esac
done
for source_root_package in "${source_root_packages[@]}"; do
  rest="${source_root_package#*:}"
  context_path="${rest%:*}"
  context_package="${rest##*:}"
  translate_args+=("--context-dir-package" "$context_path:$context_package")
done
for facts_path in "${facts_paths[@]}"; do
  translate_args+=("--facts" "$facts_path")
done
for manifest_path in "${manifest_paths[@]}"; do
  translate_args+=("--external-pack-manifest" "$manifest_path")
done
for pack_name in "${builtin_packs[@]}"; do
  translate_args+=("--builtin-external-pack" "$pack_name")
done

status_fragments_dir="$out_dir/status-fragments"
mkdir -p "$status_fragments_dir"
find "$status_fragments_dir" -type f -name '*.tsv' -delete

active_jobs=0
translation_start_ms="$(now_ms)"
if [[ "$batch_translate" -eq 1 ]]; then
  batch_args=("translate-batch" "--primary" "$primary" "--out-dir" "$out_dir" "--generated-dir" "$generated_dir" "--jobs" "$jobs" "--phase-events" "$phase_events_path" "--worker-events" "$worker_events_path" "--context-dir-emitted")
  if [[ "$enable_builtin_packs" -eq 0 ]]; then
    batch_args+=("--no-builtin-external-packs")
  fi
  for source_root in "${source_roots[@]}"; do
    case "$source_root" in
      source-summary:*|source_summary:*|summary:*)
        batch_args+=("--context-source-summary-dir" "${source_root#*:}")
        ;;
    esac
  done
  for source_root_package in "${source_root_packages[@]}"; do
    rest="${source_root_package#*:}"
    context_path="${rest%:*}"
    context_package="${rest##*:}"
    batch_args+=("--context-dir-package" "$context_path:$context_package")
  done
  for facts_path in "${facts_paths[@]}"; do
    batch_args+=("--facts" "$facts_path")
  done
  for manifest_path in "${manifest_paths[@]}"; do
    batch_args+=("--external-pack-manifest" "$manifest_path")
  done
  for pack_name in "${builtin_packs[@]}"; do
    batch_args+=("--builtin-external-pack" "$pack_name")
  done
  batch_stdout="$translate_log_dir/batch.stdout"
  batch_stderr="$translate_log_dir/batch.stderr"
  printf "batch_translate_start\t%s\t0\t%s\t0\trunning\t%s\n" "$BASHPID" "$primary" "$generated_dir" >> "$worker_events_path"
  if "$binary" "${batch_args[@]}" > "$batch_stdout" 2> "$batch_stderr"; then
    printf "batch_translate_end\t%s\t0\t%s\t%s\tok\t%s\n" "$BASHPID" "$primary" "$(elapsed_ms_since "$translation_start_ms")" "$generated_dir" >> "$worker_events_path"
  else
    printf "batch_translate_end\t%s\t0\t%s\t%s\tfailed\t%s\n" "$BASHPID" "$primary" "$(elapsed_ms_since "$translation_start_ms")" "$generated_dir" >> "$worker_events_path"
    cat "$batch_stderr" >&2
    exit 1
  fi
else
  for ((i = 0; i < ${#java_files[@]}; i++)); do
    index="$((i + 1))"
    fragment_path="$status_fragments_dir/$(printf '%05d' "$index").tsv"
    (
      translate_one "$index" "${java_files[$i]}" "$binary" "$out_dir" "$generated_dir" "$translate_log_dir" "$worker_events_path" "$translate_timeout_sec" "${translate_args[@]}" > "$fragment_path"
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

  for fragment_path in "$status_fragments_dir"/*.tsv; do
    cat "$fragment_path" >> "$status_path"
  done
fi
translation_elapsed_ms="$(elapsed_ms_since "$translation_start_ms")"
printf "translation\tok\t%s\t%s files\n" "$translation_elapsed_ms" "${#java_files[@]}" >> "$phase_events_path"

assembly_start_ms="$(now_ms)"
module_types_path="$out_dir/generated-module-types.tsv"
type_counts_path="$out_dir/generated-type-counts.tsv"
unique_types_path="$out_dir/generated-unique-types.tsv"
source_types_path="$out_dir/generated-source-types.tsv"
shared_support_path="$out_dir/generated-shared-support.rs"
shared_support_types_path="$out_dir/generated-shared-support-types.tsv"
shared_support_conflicts_path="$out_dir/generated-shared-support-conflicts.tsv"
rm -f "$module_types_path" "$type_counts_path" "$unique_types_path" "$source_types_path" "$shared_support_path" "$shared_support_types_path" "$shared_support_conflicts_path"

for i in "${!java_files[@]}"; do
  stem="$(basename "${java_files[$i]%.java}")"
  stem="$(printf '%s' "$stem" | tr -c 'A-Za-z0-9_' '_')"
  printf "%s\n" "$stem" >> "$source_types_path"
done

strip_shared_support_items() {
  local rust_path="$1"
  local shared_types_path="${2:-/dev/null}"
  local source_type
  source_type="$(basename "${rust_path%.rs}")"
  source_type="${source_type#*_}"
  awk -v shared_types_path="$shared_types_path" -v source_type="$source_type" '
    FILENAME == shared_types_path {
      shared[$1] = 1
      next
    }
    BEGIN { in_prelude = 1; pending = ""; skip_depth = 0; copy_depth = 0 }
    copy_depth > 0 {
      print
      copy_depth += gsub(/\{/, "{")
      copy_depth -= gsub(/\}/, "}")
      next
    }
    skip_depth > 0 {
      skip_depth += gsub(/\{/, "{")
      skip_depth -= gsub(/\}/, "}")
      next
    }
    in_prelude {
      if ($0 ~ /^impl[[:space:]]*<[^>]+>[[:space:]]+JavaClass[[:space:]]*</) {
        skip_depth = 0
        skip_depth += gsub(/\{/, "{")
        skip_depth -= gsub(/\}/, "}")
        if (skip_depth < 1) {
          skip_depth = 1
        }
        next
      }
      if ($0 ~ /^impl[[:space:]]*<[^{}]+>[[:space:]]+JavaIterator[[:space:]]*</) {
        skip_depth = 0
        skip_depth += gsub(/\{/, "{")
        skip_depth -= gsub(/\}/, "}")
        if (skip_depth < 1) {
          skip_depth = 1
        }
        next
      }
      if ($0 ~ /^impl([[:space:]]*<[^{}]+>)?[[:space:]]+(dyn[[:space:]]+)?[A-Za-z_][A-Za-z0-9_]*(<[^{}]+>)?[[:space:]]*(\{|where([[:space:]]|$))/) {
        impl_name = $0
        sub(/^impl([[:space:]]*<[^{}]+>)?[[:space:]]+(dyn[[:space:]]+)?/, "", impl_name)
        sub(/(<[^{}]+>)?[[:space:]]*(\{|where([[:space:]]|$)).*$/, "", impl_name)
        if (impl_name == "JavaException" || impl_name == "JavaClass" || impl_name == "FileSystem" || impl_name == "Path" || impl_name == "URI" || impl_name == "File" || impl_name == "URL" || impl_name == "URLConnection" || impl_name == "JarURLConnection" || impl_name == "JarFile" || impl_name == "JarEntry" || impl_name == "Manifest" || impl_name == "Attributes" || impl_name == "AttributesName" || impl_name == "Enumeration" || impl_name == "JavaIterator" || impl_name == "IOUtils" || (shared[impl_name] && impl_name != source_type)) {
          skip_depth = 0
          skip_depth += gsub(/\{/, "{")
          skip_depth -= gsub(/\}/, "}")
          if (skip_depth < 1) {
            skip_depth = 1
          }
          next
        }
        print
        copy_depth = 0
        copy_depth += gsub(/\{/, "{")
        copy_depth -= gsub(/\}/, "}")
        next
      }
      if ($0 ~ /^impl([[:space:]]*<[^{}]+>)?[[:space:]]+.*[[:space:]]+for[[:space:]]+[A-Za-z_][A-Za-z0-9_]*(<.*>)?[[:space:]]*\{/) {
        impl_trait_raw = $0
        sub(/^impl([[:space:]]*<[^{}]+>)?[[:space:]]+/, "", impl_trait_raw)
        sub(/[[:space:]]+for[[:space:]].*/, "", impl_trait_raw)
        impl_trait = impl_trait_raw
        sub(/<.*>/, "", impl_trait)
        from_trait_source = ""
        if (impl_trait_raw ~ /^From<[A-Za-z_][A-Za-z0-9_]*>$/) {
          from_trait_source = impl_trait_raw
          sub(/^From</, "", from_trait_source)
          sub(/>$/, "", from_trait_source)
        }
        impl_target = $0
        sub(/^.*[[:space:]]for[[:space:]]+/, "", impl_target)
        sub(/(<.*>)?[[:space:]]*\{.*/, "", impl_target)
        target_is_shared = shared[impl_target] && impl_target != source_type
        keep_module_from_impl_to_shared = target_is_shared && from_trait_source != "" && !shared[from_trait_source]
        if (impl_target == "JavaException" || impl_target == "JavaClass" || impl_target == "FileSystem" || impl_target == "Path" || impl_target == "URI" || impl_target == "File" || impl_target == "URL" || impl_target == "URLConnection" || impl_target == "JarURLConnection" || impl_target == "JarFile" || impl_target == "JarEntry" || impl_target == "Manifest" || impl_target == "Attributes" || impl_target == "AttributesName" || impl_target == "Enumeration" || impl_target == "JavaIterator" || impl_target == "IOUtils" || (target_is_shared && !keep_module_from_impl_to_shared) || shared[impl_trait]) {
          skip_depth = 0
          skip_depth += gsub(/\{/, "{")
          skip_depth -= gsub(/\}/, "}")
          if (skip_depth < 1) {
            skip_depth = 1
          }
          next
        }
        print
        copy_depth = 0
        copy_depth += gsub(/\{/, "{")
        copy_depth -= gsub(/\}/, "}")
        next
      }
      if ($0 ~ /^#\[derive/) {
        pending = pending $0 "\n"
        next
      }
      if ($0 ~ /^pub[[:space:]]+(struct|enum|trait|type)[[:space:]]+[A-Za-z_][A-Za-z0-9_]*/) {
        name = $3
        sub(/[=;{(<:].*/, "", name)
        if (name == "JavaException" || name == "JavaClass" || name == "FileSystem" || name == "Path" || name == "URI" || name == "File" || name == "URL" || name == "URLConnection" || name == "JarURLConnection" || name == "JarFile" || name == "JarEntry" || name == "Manifest" || name == "Attributes" || name == "AttributesName" || name == "Enumeration" || name == "JavaIterator" || name == "IOUtils" || (shared[name] && name != source_type)) {
          pending = ""
          skip_depth = 0
          skip_depth += gsub(/\{/, "{")
          skip_depth -= gsub(/\}/, "}")
          if (skip_depth > 0) {
            next
          }
          skip_depth = 0
          next
        }
        printf "%s", pending
        pending = ""
        print
        copy_depth = 0
        copy_depth += gsub(/\{/, "{")
        copy_depth -= gsub(/\}/, "}")
        if (shared_types_path == "/dev/null" || name == source_type) {
          in_prelude = 0
          copy_depth = 0
        }
        next
      }
      next
    }
    { print }
  ' "$shared_types_path" "$rust_path" > "$rust_path.stripped"
  mv "$rust_path.stripped" "$rust_path"
}

strip_source_type_facade_stubs() {
  local rust_path="$1"
  local source_types_path="$2"
  python3 - "$rust_path" "$source_types_path" <<'PY'
import re
import sys
from pathlib import Path

rust_path = Path(sys.argv[1])
source_types_path = Path(sys.argv[2])

stem = rust_path.stem
current_source = re.sub(r"^\d+_", "", stem)
source_types = {
    line.strip()
    for line in source_types_path.read_text().splitlines()
    if line.strip()
}
source_types.discard(current_source)

lines = rust_path.read_text().splitlines(keepends=True)
decl_re = re.compile(r"^pub\s+(struct|enum|trait|type)\s+([A-Za-z_][A-Za-z0-9_]*)")
inherent_impl_re = re.compile(
    r"^impl(?:\s*<[^>{}]*>)?\s+(?:dyn\s+)?"
    r"([A-Za-z_][A-Za-z0-9_]*)(?:<[^>{}]*>)?\s*(?:\{|where\b)"
)
trait_impl_re = re.compile(
    r"^impl(?:\s*<[^>{}]*>)?\s+.+?\s+for\s+([A-Za-z_][A-Za-z0-9_]*)\s*(?:\{|where\b)"
)
derive_re = re.compile(r"^#\[derive")


def brace_delta(line: str) -> int:
    delta = 0
    index = 0
    state = "code"
    while index < len(line):
        ch = line[index]
        nxt = line[index + 1] if index + 1 < len(line) else ""
        if state == "code":
            if ch == "/" and nxt == "/":
                break
            if ch == "/" and nxt == "*":
                state = "block_comment"
                index += 2
                continue
            if ch == '"':
                state = "string"
                index += 1
                continue
            is_char_literal = False
            if ch == "'":
                next_index = index + 1
                if next_index < len(line):
                    if line[next_index] == "\\":
                        is_char_literal = index + 3 < len(line) and line[index + 3] == "'"
                    else:
                        is_char_literal = index + 2 < len(line) and line[index + 2] == "'"
            if ch == "'" and is_char_literal:
                state = "char"
                index += 1
                continue
            if ch == "{":
                delta += 1
            elif ch == "}":
                delta -= 1
            index += 1
            continue
        if state == "string":
            if ch == "\\":
                index += 2
                continue
            if ch == '"':
                state = "code"
            index += 1
            continue
        if state == "char":
            if ch == "\\":
                index += 2
                continue
            if ch == "'":
                state = "code"
            index += 1
            continue
        if state == "block_comment":
            if ch == "*" and nxt == "/":
                state = "code"
                index += 2
                continue
            index += 1
    return delta


def item_end(start: int) -> int:
    if "{" not in lines[start]:
        return start + 1
    depth = 0
    for index in range(start, len(lines)):
        depth += brace_delta(lines[index])
        if depth <= 0:
            return index + 1
    return len(lines)


def attached_start(start: int) -> int:
    index = start
    while index > 0 and derive_re.match(lines[index - 1]):
        index -= 1
    return index


remove = [False] * len(lines)
current_source_decl_positions = [
    index
    for index, line in enumerate(lines)
    if (match := decl_re.match(line)) and match.group(2) == current_source
]
depth = 0
index = 0
while index < len(lines):
    line = lines[index]
    if depth == 0:
        decl = decl_re.match(line)
        if decl and decl.group(2) == current_source:
            if any(position > index for position in current_source_decl_positions):
                start = attached_start(index)
                end = item_end(index)
                for remove_index in range(start, end):
                    remove[remove_index] = True
                index = end
                depth = 0
                continue
            break

        target = None
        if decl:
            target = decl.group(2)
        else:
            inherent = inherent_impl_re.match(line)
            if inherent:
                target = inherent.group(1)
            else:
                trait_impl = trait_impl_re.match(line)
                if trait_impl:
                    target = trait_impl.group(1)

        if target in source_types:
            start = attached_start(index) if decl else index
            end = item_end(index)
            for remove_index in range(start, end):
                remove[remove_index] = True
            index = end
            depth = 0
            continue
    depth += brace_delta(line)
    index += 1

rust_path.write_text("".join(line for index, line in enumerate(lines) if not remove[index]))
PY
}

restore_missing_prelude_support_items() {
  local rust_path="$1"
  local raw_path="$2"
  local source_types_path="$3"
  local shared_types_path="$4"
  python3 - "$rust_path" "$raw_path" "$source_types_path" "$shared_types_path" <<'PY'
import re
import sys
from pathlib import Path

rust_path = Path(sys.argv[1])
raw_path = Path(sys.argv[2])
source_types_path = Path(sys.argv[3])
shared_types_path = Path(sys.argv[4])

if not raw_path.exists():
    raise SystemExit(0)

stem = rust_path.stem
current_source = re.sub(r"^\d+_", "", stem)
source_types = {
    line.strip()
    for line in source_types_path.read_text().splitlines()
    if line.strip()
}
shared_types = {
    line.strip()
    for line in shared_types_path.read_text().splitlines()
    if line.strip()
}

decl_re = re.compile(r"^pub\s+(struct|enum|trait|type)\s+([A-Za-z_][A-Za-z0-9_]*)")
alias_re = re.compile(r"^pub\s+type\s+([A-Za-z_][A-Za-z0-9_]*)\s*=\s*([A-Za-z_][A-Za-z0-9_]*)\s*;")
derive_re = re.compile(r"^#\[derive")
inherent_impl_re = re.compile(
    r"^impl(?:\s*<[^>{}]*>)?\s+(?:dyn\s+)?"
    r"([A-Za-z_][A-Za-z0-9_]*)(?:<[^>{}]*>)?\s*(?:\{|where\b)"
)
trait_impl_re = re.compile(r"^impl(?:\s*<[^>{}]*>)?\s+(.+?)\s+for\s+(.+?)\s*(?:\{|where\b)")


def source_owned_nested_type_name(name: str) -> bool:
    if name == current_source or not name.startswith(current_source):
        return False
    suffix = name[len(current_source):]
    return bool(suffix) and suffix[0].isupper()


def is_char_literal_start(line: str, index: int) -> bool:
    next_index = index + 1
    if next_index >= len(line):
        return False
    if line[next_index] == "\\":
        return index + 3 < len(line) and line[index + 3] == "'"
    return index + 2 < len(line) and line[index + 2] == "'"


def brace_delta(line: str) -> int:
    delta = 0
    index = 0
    state = "code"
    while index < len(line):
        ch = line[index]
        nxt = line[index + 1] if index + 1 < len(line) else ""
        if state == "code":
            if ch == "/" and nxt == "/":
                break
            if ch == "/" and nxt == "*":
                state = "block_comment"
                index += 2
                continue
            if ch == '"':
                state = "string"
                index += 1
                continue
            if ch == "'" and is_char_literal_start(line, index):
                state = "char"
                index += 1
                continue
            if ch == "{":
                delta += 1
            elif ch == "}":
                delta -= 1
            index += 1
            continue
        if state == "string":
            if ch == "\\":
                index += 2
                continue
            if ch == '"':
                state = "code"
            index += 1
            continue
        if state == "char":
            if ch == "\\":
                index += 2
                continue
            if ch == "'":
                state = "code"
            index += 1
            continue
        if state == "block_comment":
            if ch == "*" and nxt == "/":
                state = "code"
                index += 2
                continue
            index += 1
    return delta


def item_end(lines: list[str], start: int) -> int:
    if "{" not in lines[start]:
        return start + 1
    depth = 0
    for index in range(start, len(lines)):
        depth += brace_delta(lines[index])
        if depth <= 0:
            return index + 1
    return len(lines)


def function_item_end(lines: list[str], start: int) -> int:
    depth = 0
    saw_body = False
    for index in range(start, len(lines)):
        if "{" in lines[index]:
            saw_body = True
        depth += brace_delta(lines[index])
        if saw_body and depth <= 0:
            return index + 1
        if not saw_body and ";" in lines[index]:
            return index + 1
    return len(lines)


def attached_start(lines: list[str], start: int) -> int:
    index = start
    while index > 0 and derive_re.match(lines[index - 1]):
        index -= 1
    return index


def declared_names(text: str) -> set[str]:
    names = set()
    for line in text.splitlines():
        if match := decl_re.match(line):
            names.add(match.group(2))
        elif match := alias_re.match(line):
            names.add(match.group(1))
    return names


def top_level_function_items(lines: list[str]) -> dict[str, list[tuple[int, int, str]]]:
    functions = {}
    depth = 0
    index = 0
    while index < len(lines):
        line = lines[index]
        if depth == 0 and (match := re.match(r"^\s*(?:pub\s+)?fn\s+([A-Za-z_][A-Za-z0-9_]*)\b", line)):
            end = function_item_end(lines, index)
            functions.setdefault(match.group(1), []).append(
                (index, end, "".join(lines[index:end]).rstrip())
            )
            index = end
            depth = 0
            continue
        depth += brace_delta(line)
        index += 1
    return functions


def referenced_function_names(text: str, candidate_names: set[str]) -> set[str]:
    referenced = set()
    for name in candidate_names:
        if re.search(rf"\b{re.escape(name)}\s*(?:<|\()", text):
            referenced.add(name)
    return referenced


def support_item_name(line: str):
    if match := decl_re.match(line):
        return match.group(2)
    if match := alias_re.match(line):
        return match.group(1)
    if match := inherent_impl_re.match(line):
        return match.group(1)
    if match := trait_impl_re.match(line):
        return match.group(2)
    return None


def restorable_support_name(name: str) -> bool:
    return (
        name
        and name != current_source
        and name not in source_types
        and name not in shared_types
        and not source_owned_nested_type_name(name)
    )


current = rust_path.read_text()
present = declared_names(current)
present_functions = set(top_level_function_items(current.splitlines(keepends=True)))
raw_lines = raw_path.read_text().splitlines(keepends=True)
candidate_names = set()
depth = 0
index = 0
while index < len(raw_lines):
    line = raw_lines[index]
    if depth == 0:
        if (match := decl_re.match(line)) and match.group(2) == current_source:
            break
        if match := decl_re.match(line):
            name = match.group(2)
            if restorable_support_name(name) and name not in present:
                candidate_names.add(name)
        elif match := alias_re.match(line):
            name = match.group(1)
            if restorable_support_name(name) and name not in present:
                candidate_names.add(name)
    depth += brace_delta(line)
    index += 1

chunks = []
if candidate_names:
    depth = 0
    index = 0
    while index < len(raw_lines):
        line = raw_lines[index]
        if depth == 0:
            if (match := decl_re.match(line)) and match.group(2) == current_source:
                break
            name = support_item_name(line)
            if name and name in candidate_names:
                start = attached_start(raw_lines, index) if decl_re.match(line) else index
                end = item_end(raw_lines, index)
                chunks.append("".join(raw_lines[start:end]).rstrip())
                index = end
                depth = 0
                continue
        depth += brace_delta(line)
        index += 1

raw_functions = top_level_function_items(raw_lines)
selected_functions = set()
pending_texts = [current] + list(chunks)
while pending_texts:
    text = pending_texts.pop()
    for name in referenced_function_names(text, set(raw_functions)):
        if name in selected_functions or name in present_functions:
            continue
        selected_functions.add(name)
        pending_texts.extend(item[2] for item in raw_functions[name])

for name in sorted(selected_functions):
    variants = []
    seen = set()
    for _, _, text in raw_functions[name]:
        if text in seen:
            continue
        variants.append(text)
        seen.add(text)
    if variants:
        chunks.append(variants[0])

if not chunks:
    raise SystemExit(0)

current_lines = current.splitlines(keepends=True)
insert_at = None
for line_index, line in enumerate(current_lines):
    if (match := decl_re.match(line)) and match.group(2) == current_source:
        insert_at = attached_start(current_lines, line_index)
        break
restored = "\n".join(chunks) + "\n"
if insert_at is None:
    separator = "" if current.endswith("\n") or not current else "\n"
    rust_path.write_text(current + separator + restored)
else:
    current_lines[insert_at:insert_at] = [restored]
    rust_path.write_text("".join(current_lines))
PY
}

restore_missing_source_type_items() {
  local rust_path="$1"
  local raw_path="$2"
  local shared_types_path="${3:-}"
  python3 - "$rust_path" "$raw_path" "$shared_types_path" <<'PY'
import re
import sys
from pathlib import Path

rust_path = Path(sys.argv[1])
raw_path = Path(sys.argv[2])
shared_types_path = Path(sys.argv[3]) if len(sys.argv) > 3 and sys.argv[3] else None

if not raw_path.exists():
    raise SystemExit(0)

stem = rust_path.stem
current_source = re.sub(r"^\d+_", "", stem)
decl_re = re.compile(r"^pub\s+(struct|enum|trait|type)\s+([A-Za-z_][A-Za-z0-9_]*)")
derive_re = re.compile(r"^#\[derive")
inherent_impl_re = re.compile(
    r"^impl(?:\s*<[^>{}]*>)?\s+(?:dyn\s+)?"
    r"([A-Za-z_][A-Za-z0-9_]*)(?:<[^>{}]*>)?\s*(?:\{|where\b)"
)
trait_impl_re = re.compile(r"^impl(?:\s*<[^>{}]*>)?\s+(.+?)\s+for\s+(.+?)\s*(?:\{|where\b)")
alias_re = re.compile(r"^pub\s+type\s+([A-Za-z_][A-Za-z0-9_]*)\s*=\s*([A-Za-z_][A-Za-z0-9_]*)\s*;")


def source_owned_nested_type_name(name: str) -> bool:
    if name == current_source or not name.startswith(current_source):
        return False
    suffix = name[len(current_source):]
    return bool(suffix) and suffix[0].isupper()


def declared_names(text: str) -> set[str]:
    return {
        match.group(2)
        for line in text.splitlines()
        if (match := decl_re.match(line))
    }


def attached_start(lines: list[str], start: int) -> int:
    index = start
    while index > 0 and derive_re.match(lines[index - 1]):
        index -= 1
    return index


def is_char_literal_start(line: str, index: int) -> bool:
    next_index = index + 1
    if next_index >= len(line):
        return False
    if line[next_index] == "\\":
        return index + 3 < len(line) and line[index + 3] == "'"
    return index + 2 < len(line) and line[index + 2] == "'"


def brace_delta(line: str) -> int:
    delta = 0
    index = 0
    state = "code"
    while index < len(line):
        ch = line[index]
        nxt = line[index + 1] if index + 1 < len(line) else ""
        if state == "code":
            if ch == "/" and nxt == "/":
                break
            if ch == "/" and nxt == "*":
                state = "block_comment"
                index += 2
                continue
            if ch == '"':
                state = "string"
                index += 1
                continue
            if ch == "'" and is_char_literal_start(line, index):
                state = "char"
                index += 1
                continue
            if ch == "{":
                delta += 1
            elif ch == "}":
                delta -= 1
            index += 1
            continue
        if state == "string":
            if ch == "\\":
                index += 2
                continue
            if ch == '"':
                state = "code"
            index += 1
            continue
        if state == "char":
            if ch == "\\":
                index += 2
                continue
            if ch == "'":
                state = "code"
            index += 1
            continue
        if state == "block_comment":
            if ch == "*" and nxt == "/":
                state = "code"
                index += 2
                continue
            index += 1
    return delta


def item_end(lines: list[str], start: int) -> int:
    if "{" not in lines[start]:
        return start + 1
    depth = 0
    for index in range(start, len(lines)):
        depth += brace_delta(lines[index])
        if depth <= 0:
            return index + 1
    return len(lines)


def remove_names_from_crate_import(text: str, names: set[str]) -> str:
    if not names:
        return text
    lines = text.splitlines(keepends=True)
    out = []
    for line in lines:
        if line.startswith("use crate::{") and line.rstrip().endswith("};"):
            prefix = "use crate::{"
            suffix = "};"
            body = line[len(prefix):line.rfind(suffix)]
            entries = [entry for entry in body.split(",") if entry and entry not in names]
            if entries:
                out.append(prefix + ",".join(entries) + suffix + ("\n" if line.endswith("\n") else ""))
            continue
        out.append(line)
    return "".join(out)


current = rust_path.read_text()
present = declared_names(current)
raw_lines = raw_path.read_text().splitlines(keepends=True)
source_owned_names = {
    match.group(2)
    for line in raw_lines
    if (match := decl_re.match(line))
    and (match.group(2) == current_source or source_owned_nested_type_name(match.group(2)))
}
shared_names = set()
if shared_types_path and shared_types_path.exists():
    shared_names = {
        line.strip()
        for line in shared_types_path.read_text().splitlines()
        if line.strip()
    }
source_owned_names = {
    name
    for name in source_owned_names
    if name == current_source or name not in shared_names
}
missing_names = source_owned_names - present
if not missing_names:
    raise SystemExit(0)

chunks = []
index = 0
while index < len(raw_lines):
    line = raw_lines[index]
    target = None
    start = index
    alias = None
    if match := decl_re.match(line):
        target = match.group(2)
        start = attached_start(raw_lines, index)
    elif match := inherent_impl_re.match(line):
        target = match.group(1)
    elif match := trait_impl_re.match(line):
        target = match.group(2)
    elif match := alias_re.match(line):
        alias = (match.group(1), match.group(2))
        target = match.group(2)
    if target in missing_names or (alias and alias[1] in missing_names):
        end = item_end(raw_lines, index)
        chunks.append("".join(raw_lines[start:end]))
        index = end
        continue
    index += 1

if chunks:
    restored_names = set(missing_names)
    current = remove_names_from_crate_import(current, restored_names)
    restored = "\n".join(chunk.rstrip() for chunk in chunks) + "\n"
    current_lines = current.splitlines(keepends=True)
    insert_at = None
    for line_index, line in enumerate(current_lines):
        if (match := decl_re.match(line)) and match.group(2) == current_source:
            insert_at = attached_start(current_lines, line_index)
            break
    if insert_at is None:
        separator = "" if current.endswith("\n") or not current else "\n"
        rust_path.write_text(current + separator + restored)
    else:
        current_lines[insert_at:insert_at] = [restored]
        rust_path.write_text("".join(current_lines))
PY
}

if grep -R -q '^pub struct JavaException' "$generated_dir"; then
  cat >> "$shared_support_path" <<'RUST'
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
pub struct InputStream;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JavaException {
    pub class_name: &'static str,
    pub message: String,
    pub cause: Option<Box<JavaException>>,
}

impl JavaException {
    pub fn new(class_name: &'static str, message: String) -> JavaException {
        JavaException { class_name, message, cause: None }
    }

    pub fn with_cause(
        class_name: &'static str,
        message: String,
        cause: JavaException,
    ) -> JavaException {
        JavaException { class_name, message, cause: Some(Box::new(cause)) }
    }

    pub fn from_display(class_name: &'static str, err: impl std::fmt::Display) -> JavaException {
        JavaException { class_name, message: err.to_string(), cause: None }
    }

    pub fn is_instance(&self, class_names: &[&str]) -> bool {
        class_names.iter().any(|name| self.class_name == *name)
    }

    pub fn cause_or_self_if_instance(mut self, class_names: &[&str]) -> JavaException {
        if self.cause.as_ref().is_some_and(|cause| cause.is_instance(class_names)) {
            *self.cause.take().expect("checked Java exception cause")
        } else {
            self
        }
    }

    pub fn get_message(&self) -> Option<String> {
        Some(self.message.clone())
    }

    pub fn get_localized_message(&self) -> Option<String> {
        self.get_message()
    }
}

impl InputStream {
    pub fn read_0(&self) -> Result<i32, JavaException> {
        Ok(0)
    }

    pub fn read<T0: ?Sized>(&self, _arg0: &T0) -> Result<i32, JavaException> {
        Ok(0)
    }

    pub fn read_3<T0: ?Sized>(
        &self,
        _arg0: &T0,
        _arg1: i32,
        _arg2: i32,
    ) -> Result<i32, JavaException> {
        Ok(0)
    }

    pub fn close(&self) -> Result<(), JavaException> {
        Ok(())
    }
}

impl std::fmt::Display for JavaException {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.class_name, self.message)
    }
}

RUST
  if [[ -n "$external_exception_conversion_impls" ]]; then
    printf '%s\n' "$external_exception_conversion_impls" >> "$shared_support_path"
    printf '\n' >> "$shared_support_path"
  fi
  printf "InputStream\n" >> "$shared_support_types_path"
  printf "JavaException\n" >> "$shared_support_types_path"
fi

if grep -R -q -E '^pub struct JavaClass|JavaClass(::|<)' "$generated_dir"; then
  cat >> "$shared_support_path" <<'RUST'
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JavaClass<T> {
    name: String,
    _marker: std::marker::PhantomData<T>,
}

impl<T> JavaClass<T> {
    pub fn literal(name: &'static str) -> JavaClass<T> {
        JavaClass { name: name.to_string(), _marker: std::marker::PhantomData }
    }

    pub fn for_name<S: Into<String>>(name: S) -> JavaClass<T> {
        JavaClass { name: name.into(), _marker: std::marker::PhantomData }
    }

    pub fn get_name(&self) -> String {
        self.name.clone()
    }

    pub fn get_simple_name(&self) -> String {
        self.name.rsplit('.').next().unwrap_or(&self.name).to_string()
    }

    pub fn is_array(&self) -> bool {
        false
    }

    pub fn get_resource_as_stream<S: Into<String>>(&self, _path: S) -> Option<InputStream> {
        Some(InputStream)
    }

    pub fn get_class_loader(&self) -> ClassLoader {
        ClassLoader
    }

    pub fn equals<U>(&self, other: JavaClass<U>) -> bool {
        self.name == other.name
    }
}

RUST
  printf "JavaClass\n" >> "$shared_support_types_path"
fi

if grep -R -q '^pub struct Path' "$generated_dir" || grep -R -q '^pub struct URI' "$generated_dir" || grep -R -q '^pub struct File' "$generated_dir" || grep -R -q '^pub struct URL' "$generated_dir" || grep -R -q '^pub struct JarURLConnection' "$generated_dir" || grep -R -q '^pub struct JarFile' "$generated_dir" || grep -R -q '^pub struct JarEntry' "$generated_dir" || grep -R -q '^pub struct Manifest' "$generated_dir" || grep -R -q '^pub struct Attributes' "$generated_dir" || grep -R -q '^pub struct AttributesName' "$generated_dir" || grep -R -q '^pub struct Enumeration' "$generated_dir" || grep -R -q 'JavaIterator' "$generated_dir" || grep -R -q '^pub struct AtomicInteger' "$generated_dir" || grep -R -q '^pub struct IOUtils' "$generated_dir" || grep -R -q '^pub struct BufferedImage' "$generated_dir" || grep -R -q '^pub struct Graphics' "$generated_dir"; then
  cat >> "$shared_support_path" <<'RUST'
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
pub struct FileSystem {}

impl FileSystem {
    pub fn delete<T0>(&self, _path: T0, _recursive: bool) -> bool {
        true
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
pub struct Graphics;

impl Graphics {
    pub fn dispose(&self) {}
    pub fn draw_image<T0, T1, T2, T3>(&self, _image: T0, _x: T1, _y: T2, _observer: T3) -> bool {
        true
    }
    pub fn get_clip<R: Default>(&self) -> R {
        Default::default()
    }
    pub fn get_clip_bounds<R: Default>(&self) -> R {
        Default::default()
    }
    pub fn get_font_metrics<R: Default, T0>(&self, _font: T0) -> R {
        Default::default()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
pub struct BufferedImage;

impl BufferedImage {
    pub const TYPE_INT_RGB: i32 = 1;

    pub fn new<T0, T1, T2>(_width: T0, _height: T1, _image_type: T2) -> Self {
        Self
    }

    pub fn get_graphics(&self) -> Graphics {
        Graphics
    }

    pub fn create_graphics(&self) -> Graphics {
        Graphics
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
pub struct Path {
    value: String,
}

impl Path {
    pub fn new<P: JavaPathArg>(value: P) -> Path {
        Path { value: value.into_path_value() }
    }

    pub fn to_uri(&self) -> URI {
        URI::new(self.value.clone())
    }

    pub fn to_absolute_path(&self) -> Path {
        self.clone()
    }

    pub fn to_file(&self) -> File {
        File::new(self.value.clone())
    }

    pub fn resolve_sibling<P: JavaPathArg>(&self, sibling: P) -> Path {
        Path::new(sibling)
    }

    pub fn get_file_name(&self) -> String {
        self.value.rsplit('/').next().unwrap_or(&self.value).to_string()
    }

    pub fn get_file_system<T0>(&self, _arg0: T0) -> FileSystem {
        FileSystem {}
    }
}

impl std::fmt::Display for Path {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.value)
    }
}

impl AsRef<std::path::Path> for Path {
    fn as_ref(&self) -> &std::path::Path {
        std::path::Path::new(&self.value)
    }
}

impl From<Path> for std::path::PathBuf {
    fn from(path: Path) -> std::path::PathBuf {
        std::path::PathBuf::from(path.value)
    }
}

impl From<&Path> for std::path::PathBuf {
    fn from(path: &Path) -> std::path::PathBuf {
        std::path::PathBuf::from(path.value.clone())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
pub struct URI {
    value: String,
}

impl URI {
    pub fn new<P: JavaPathArg>(value: P) -> URI {
        URI { value: value.into_path_value() }
    }

    pub fn parse<P: JavaPathArg>(value: P) -> Result<URI, JavaException> {
        Ok(URI::new(value))
    }

    pub fn get_path(&self) -> String {
        self.value.clone()
    }

    pub fn get_raw_path(&self) -> String {
        self.value.clone()
    }

    pub fn to_string(&self) -> String {
        self.value.clone()
    }
}

impl std::fmt::Display for URI {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.value)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
pub struct URL {
    value: String,
}

impl URL {
    pub fn new<S: Into<String>>(value: S) -> URL {
        URL { value: value.into() }
    }

    pub fn get_file(&self) -> String {
        self.value.clone()
    }

    pub fn open_connection<T: Default>(&self) -> Result<T, JavaException> {
        Ok(T::default())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
pub struct URLConnection;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
pub struct JarURLConnection;

impl JarURLConnection {
    pub fn get_jar_file(&self) -> Result<JarFile, JavaException> {
        Ok(JarFile)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
pub struct JarFile;

impl JarFile {
    pub fn entries(&self) -> Result<Enumeration, JavaException> {
        Ok(Enumeration)
    }

    pub fn get_manifest(&self) -> Result<Manifest, JavaException> {
        Ok(Manifest)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
pub struct JarEntry;

impl JarEntry {
    pub fn is_directory(&self) -> bool {
        false
    }

    pub fn get_name(&self) -> String {
        String::new()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
pub struct Manifest;

impl Manifest {
    pub fn get_main_attributes(&self) -> Attributes {
        Attributes
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
pub struct Attributes;

impl Attributes {
    pub fn get_value<T>(&self, _name: T) -> String {
        String::new()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
pub struct AttributesName {
    value: String,
}

impl AttributesName {
    pub fn new<S: Into<String>>(value: S) -> AttributesName {
        AttributesName { value: value.into() }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
pub struct Enumeration;

impl Enumeration {
    pub fn has_more_elements(&self) -> Result<bool, JavaException> {
        Ok(false)
    }

    pub fn next_element<T: Default>(&self) -> Result<T, JavaException> {
        Ok(T::default())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
pub struct JavaIterator<T> {
    items: Vec<T>,
    index: usize,
}

impl<T: Clone> JavaIterator<T> {
    pub fn new<I: IntoIterator<Item = T>>(items: I) -> JavaIterator<T> {
        JavaIterator { items: items.into_iter().collect(), index: 0 }
    }

    pub fn has_next(&self) -> bool {
        self.index < self.items.len()
    }

    pub fn next(&mut self) -> T {
        let value = self.items.get(self.index).expect("Java iterator exhausted").clone();
        self.index += 1;
        value
    }

    pub fn for_each_remaining<F>(&mut self, mut f: F)
    where
        F: FnMut(T),
    {
        while self.has_next() {
            f(self.next());
        }
    }
}

#[derive(Debug, Default)]
pub struct AtomicInteger {
    value: std::cell::Cell<i32>,
}

impl AtomicInteger {
    pub fn new(value: i32) -> AtomicInteger {
        AtomicInteger { value: std::cell::Cell::new(value) }
    }

    pub fn get(&self) -> i32 {
        self.value.get()
    }

    pub fn set(&self, value: i32) {
        self.value.set(value);
    }

    pub fn increment_and_get(&self) -> i32 {
        let value = self.value.get().wrapping_add(1);
        self.value.set(value);
        value
    }

    pub fn get_and_increment(&self) -> i32 {
        let value = self.value.get();
        self.value.set(value.wrapping_add(1));
        value
    }
}

impl Clone for AtomicInteger {
    fn clone(&self) -> Self {
        AtomicInteger::new(self.value.get())
    }
}

impl PartialEq for AtomicInteger {
    fn eq(&self, other: &Self) -> bool {
        self.value.get() == other.value.get()
    }
}

impl Eq for AtomicInteger {}

impl std::hash::Hash for AtomicInteger {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        std::hash::Hash::hash(&self.value.get(), state);
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
pub struct File {
    value: String,
}

impl File {
    pub fn new<P: JavaPathArg>(value: P) -> File {
        File { value: value.into_path_value() }
    }

    pub fn new_1<P: JavaPathArg>(value: P) -> File {
        Self::new(value)
    }

    pub fn new_2<P: JavaPathArg, C: JavaPathArg>(parent: P, child: C) -> File {
        let parent = parent.into_path_value();
        let child = child.into_path_value();
        if parent.is_empty() {
            Self::new(child)
        } else if parent.ends_with('/') {
            Self::new(format!("{parent}{child}"))
        } else {
            Self::new(format!("{parent}/{child}"))
        }
    }

    pub fn to_path(&self) -> Path {
        Path::new(self.value.clone())
    }

    pub fn get_path(&self) -> String {
        self.value.clone()
    }

    pub fn get_absolute_path(&self) -> String {
        self.value.clone()
    }

    pub fn get_name(&self) -> String {
        self.value
            .rsplit('/')
            .next()
            .unwrap_or(&self.value)
            .to_string()
    }

    pub fn get_parent(&self) -> String {
        self.value
            .rsplit_once('/')
            .map(|(parent, _)| parent.to_string())
            .unwrap_or_default()
    }

    pub fn get_absolute_file(&self) -> File {
        self.clone()
    }

    pub fn delete_on_exit(&self) {}

    pub fn is_directory(&self) -> bool {
        std::path::Path::new(&self.value).is_dir()
    }

    pub fn is_file(&self) -> bool {
        std::path::Path::new(&self.value).is_file()
    }

    pub fn can_read(&self) -> bool {
        std::fs::File::open(std::path::Path::new(&self.value)).is_ok()
    }

    pub fn length(&self) -> i64 {
        std::fs::metadata(std::path::Path::new(&self.value))
            .map(|metadata| metadata.len() as i64)
            .unwrap_or(0)
    }

    pub fn list(&self) -> Vec<String> {
        std::fs::read_dir(std::path::Path::new(&self.value))
            .map(|entries| {
                entries
                    .filter_map(|entry| {
                        entry
                            .ok()
                            .and_then(|entry| entry.file_name().into_string().ok())
                    })
                    .collect()
            })
            .unwrap_or_default()
    }

    pub fn mkdir(&self) -> bool {
        std::fs::create_dir(std::path::Path::new(&self.value)).is_ok()
    }
}

impl std::fmt::Display for File {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.value)
    }
}

impl AsRef<std::path::Path> for File {
    fn as_ref(&self) -> &std::path::Path {
        std::path::Path::new(&self.value)
    }
}

impl From<File> for std::path::PathBuf {
    fn from(file: File) -> std::path::PathBuf {
        std::path::PathBuf::from(file.value)
    }
}

impl From<&File> for std::path::PathBuf {
    fn from(file: &File) -> std::path::PathBuf {
        std::path::PathBuf::from(file.value.clone())
    }
}

pub trait JavaPathArg {
    fn into_path_value(self) -> String;
}

impl JavaPathArg for String {
    fn into_path_value(self) -> String {
        self
    }
}

impl JavaPathArg for &String {
    fn into_path_value(self) -> String {
        self.clone()
    }
}

impl JavaPathArg for &str {
    fn into_path_value(self) -> String {
        self.to_string()
    }
}

impl JavaPathArg for Option<String> {
    fn into_path_value(self) -> String {
        self.unwrap_or_default()
    }
}

impl JavaPathArg for &Option<String> {
    fn into_path_value(self) -> String {
        self.clone().unwrap_or_default()
    }
}

impl JavaPathArg for Path {
    fn into_path_value(self) -> String {
        self.value
    }
}

impl JavaPathArg for &Path {
    fn into_path_value(self) -> String {
        self.value.clone()
    }
}

impl JavaPathArg for Option<Path> {
    fn into_path_value(self) -> String {
        self.map(|path| path.value).unwrap_or_default()
    }
}

impl JavaPathArg for &Option<Path> {
    fn into_path_value(self) -> String {
        self.as_ref().map(|path| path.value.clone()).unwrap_or_default()
    }
}

impl JavaPathArg for URI {
    fn into_path_value(self) -> String {
        self.value
    }
}

impl JavaPathArg for &URI {
    fn into_path_value(self) -> String {
        self.value.clone()
    }
}

impl JavaPathArg for File {
    fn into_path_value(self) -> String {
        self.value
    }
}

impl JavaPathArg for &File {
    fn into_path_value(self) -> String {
        self.value.clone()
    }
}

pub struct IOUtils;

impl IOUtils {
    pub fn get_path<P: JavaPathArg>(path: P) -> Path {
        Path::new(path.into_path_value())
    }

    pub fn append_path_to_dir<D: JavaPathArg, P: JavaPathArg>(dir: D, path: P) -> String {
        let dir = dir.into_path_value();
        let path = path.into_path_value();
        if dir.is_empty() {
            path
        } else if dir.ends_with('/') {
            format!("{dir}{path}")
        } else {
            format!("{dir}/{path}")
        }
    }

    pub fn assert_file_is_readable<P: JavaPathArg>(_path: P) {}

    pub fn assert_paths_are_readable<P: JavaPathArg>(_path: P) {}

    pub fn create_temp_file<P: JavaPathArg, S: JavaPathArg>(prefix: P, suffix: S) -> File {
        File::new(format!("{}{}", prefix.into_path_value(), suffix.into_path_value()))
    }
}

pub mod logger {
    pub fn info<M: std::fmt::Display>(_message: M) {}
    pub fn warn<M: std::fmt::Display>(_message: M) {}
    pub fn debug<M: std::fmt::Display>(_message: M) {}
    pub fn error<M: std::fmt::Display>(_message: M) {}
    pub fn trace<M: std::fmt::Display>(_message: M) {}
}

RUST
  printf "FileSystem\nGraphics\nBufferedImage\nPath\nURI\nFile\nURL\nURLConnection\nJarURLConnection\nJarFile\nJarEntry\nManifest\nAttributes\nAttributesName\nEnumeration\nJavaIterator\nAtomicInteger\nJavaPathArg\nIOUtils\nlogger\n" >> "$shared_support_types_path"
fi

if grep -R -q -E 'java_string_(split|substring|hash_code|index_of|index_of_from|from_byte_range|matches|replace_first|replace_all|compare_to|compare_to_ignore_case|equals_ignore_case)|java_(list|set|map|value)_hash_code|java_primitive_compare_to|java_math_(floor_(div|mod)_i32|round_(f32|f64)|m(ax|in)_(f32|f64))|java_parse_(i8|i16|i32|i64|f32|f64)' "$generated_dir"; then
  cat >> "$shared_support_path" <<'RUST'
pub fn java_string_substring<V: AsRef<str>>(value: V, begin: i32, end: i32) -> String {
    let value = value.as_ref();
    let begin = begin.max(0) as usize;
    let end = end.max(begin as i32) as usize;
    value
        .chars()
        .skip(begin)
        .take(end.saturating_sub(begin))
        .collect()
}

pub fn java_string_hash_code(value: &str) -> i32 {
    value
        .encode_utf16()
        .fold(0i32, |hash, unit| hash.wrapping_mul(31).wrapping_add(unit as i32))
}

pub fn java_list_hash_code<T>(value: &[T]) -> i32 {
    value.len() as i32
}

pub fn java_set_hash_code<T, S>(value: &std::collections::HashSet<T, S>) -> i32 {
    value.len() as i32
}

pub fn java_map_hash_code<K, V, S>(value: &std::collections::HashMap<K, V, S>) -> i32 {
    value.len() as i32
}

pub fn java_value_hash_code<T: std::hash::Hash + ?Sized>(value: &T) -> i32 {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    value.hash(&mut hasher);
    hasher.finish() as i32
}

pub fn java_string_index_of<V: AsRef<str>, N: AsRef<str>>(value: V, needle: N) -> i32 {
    value
        .as_ref()
        .find(needle.as_ref())
        .map(|index| index as i32)
        .unwrap_or(-1)
}

pub fn java_string_index_of_from<V: AsRef<str>, N: AsRef<str>>(
    value: V,
    needle: N,
    from_index: i32,
) -> i32 {
    let value = value.as_ref();
    let needle = needle.as_ref();
    let from_index = from_index.max(0) as usize;
    if from_index >= value.len() {
        return if needle.is_empty() { value.len() as i32 } else { -1 };
    }
    value[from_index..]
        .find(needle)
        .map(|index| (from_index + index) as i32)
        .unwrap_or(-1)
}

pub fn java_string_last_index_of<V: AsRef<str>, N: AsRef<str>>(value: V, needle: N) -> i32 {
    value
        .as_ref()
        .rfind(needle.as_ref())
        .map(|index| index as i32)
        .unwrap_or(-1)
}

pub fn java_string_last_index_of_from<V: AsRef<str>, N: AsRef<str>>(
    value: V,
    needle: N,
    from_index: i32,
) -> i32 {
    let value = value.as_ref();
    let needle = needle.as_ref();
    if from_index < 0 {
        return -1;
    }
    if needle.is_empty() {
        return std::cmp::min(from_index as usize, value.len()) as i32;
    }
    let end = std::cmp::min((from_index as usize).saturating_add(needle.len()), value.len());
    value[..end]
        .rfind(needle)
        .map(|index| index as i32)
        .unwrap_or(-1)
}

pub fn java_string_compare_to<L: AsRef<str>, R: AsRef<str>>(left: L, right: R) -> i32 {
    match left.as_ref().cmp(right.as_ref()) {
        std::cmp::Ordering::Less => -1,
        std::cmp::Ordering::Equal => 0,
        std::cmp::Ordering::Greater => 1,
    }
}

pub fn java_string_compare_to_ignore_case<L: AsRef<str>, R: AsRef<str>>(
    left: L,
    right: R,
) -> i32 {
    java_string_compare_to(left.as_ref().to_lowercase(), right.as_ref().to_lowercase())
}

pub fn java_string_equals_ignore_case<L: AsRef<str>, R: AsRef<str>>(left: L, right: R) -> bool {
    left.as_ref().eq_ignore_ascii_case(right.as_ref())
}

pub fn java_primitive_compare_to<T: PartialOrd>(left: T, right: T) -> i32 {
    if left < right {
        -1
    } else if left > right {
        1
    } else {
        0
    }
}

pub fn java_math_floor_div_i32(left: i32, right: i32) -> i32 {
    let quotient = left / right;
    let remainder = left % right;
    if remainder != 0 && ((left < 0) != (right < 0)) {
        quotient - 1
    } else {
        quotient
    }
}

pub fn java_math_floor_mod_i32(left: i32, right: i32) -> i32 {
    left - java_math_floor_div_i32(left, right) * right
}

pub fn java_math_round_f32(value: f32) -> i32 {
    if value.is_nan() {
        0
    } else if value >= i32::MAX as f32 {
        i32::MAX
    } else if value <= i32::MIN as f32 {
        i32::MIN
    } else {
        (value + 0.5).floor() as i32
    }
}

pub fn java_math_round_f64(value: f64) -> i64 {
    if value.is_nan() {
        0
    } else if value >= i64::MAX as f64 {
        i64::MAX
    } else if value <= i64::MIN as f64 {
        i64::MIN
    } else {
        (value + 0.5).floor() as i64
    }
}

pub fn java_math_max_f32(left: f32, right: f32) -> f32 {
    if left.is_nan() || right.is_nan() {
        f32::NAN
    } else if left == 0.0 && right == 0.0 {
        if left.is_sign_positive() || right.is_sign_positive() {
            0.0
        } else {
            -0.0
        }
    } else {
        left.max(right)
    }
}

pub fn java_math_max_f64(left: f64, right: f64) -> f64 {
    if left.is_nan() || right.is_nan() {
        f64::NAN
    } else if left == 0.0 && right == 0.0 {
        if left.is_sign_positive() || right.is_sign_positive() {
            0.0
        } else {
            -0.0
        }
    } else {
        left.max(right)
    }
}

pub fn java_math_min_f32(left: f32, right: f32) -> f32 {
    if left.is_nan() || right.is_nan() {
        f32::NAN
    } else if left == 0.0 && right == 0.0 {
        if left.is_sign_negative() || right.is_sign_negative() {
            -0.0
        } else {
            0.0
        }
    } else {
        left.min(right)
    }
}

pub fn java_math_min_f64(left: f64, right: f64) -> f64 {
    if left.is_nan() || right.is_nan() {
        f64::NAN
    } else if left == 0.0 && right == 0.0 {
        if left.is_sign_negative() || right.is_sign_negative() {
            -0.0
        } else {
            0.0
        }
    } else {
        left.min(right)
    }
}

pub fn java_parse_i8<V: AsRef<str>>(value: V) -> i8 {
    value.as_ref().parse::<i8>().expect("Java number format")
}

pub fn java_parse_i16<V: AsRef<str>>(value: V) -> i16 {
    value.as_ref().parse::<i16>().expect("Java number format")
}

pub fn java_parse_i32<V: AsRef<str>>(value: V) -> i32 {
    value.as_ref().parse::<i32>().expect("Java number format")
}

pub fn java_parse_i64<V: AsRef<str>>(value: V) -> i64 {
    value.as_ref().parse::<i64>().expect("Java number format")
}

pub fn java_parse_f32<V: AsRef<str>>(value: V) -> f32 {
    value.as_ref().parse::<f32>().expect("Java number format")
}

pub fn java_parse_f64<V: AsRef<str>>(value: V) -> f64 {
    value.as_ref().parse::<f64>().expect("Java number format")
}

pub fn java_string_from_byte_range(bytes: &[i8], offset: i32, len: i32) -> String {
    let start = offset.max(0) as usize;
    let end = start.saturating_add(len.max(0) as usize).min(bytes.len());
    let bytes = bytes[start..end]
        .iter()
        .map(|byte| *byte as u8)
        .collect::<Vec<_>>();
    String::from_utf8_lossy(&bytes).into_owned()
}

pub fn java_string_from_bytes(bytes: &[i8]) -> String {
    let bytes = bytes.iter().map(|byte| *byte as u8).collect::<Vec<_>>();
    String::from_utf8_lossy(&bytes).into_owned()
}

pub fn java_string_split<R: AsRef<str>>(value: &str, regex: R, limit: i32) -> Vec<String> {
    let regex = regex.as_ref();
    let split_whitespace = regex == "\\s+";
    let delimiter = match regex {
        "\\t" | "\\\\t" => "\t",
        "\\n" | "\\\\n" => "\n",
        "\\r" | "\\\\r" => "\r",
        "\\s+" => " ",
        other => other,
    };
    let mut parts = if split_whitespace {
        value.split_whitespace().map(|part| part.to_string()).collect::<Vec<_>>()
    } else {
        value.split(delimiter).map(|part| part.to_string()).collect::<Vec<_>>()
    };
    if limit == 0 {
        while parts.last().is_some_and(|part| part.is_empty()) {
            parts.pop();
        }
    } else if limit > 0 && parts.len() > limit as usize {
        let keep = limit as usize - 1;
        let tail = parts.split_off(keep).join(delimiter);
        parts.push(tail);
    }
    parts
}

pub fn java_string_matches<R: AsRef<str>>(value: &str, regex: R) -> bool {
    match regex.as_ref() {
        "[0-9]" => value.chars().count() == 1 && value.chars().all(|ch| ch.is_ascii_digit()),
        "\\d+" | "[0-9]+" => !value.is_empty() && value.chars().all(|ch| ch.is_ascii_digit()),
        "\\s+" => !value.is_empty() && value.chars().all(|ch| ch.is_whitespace()),
        regex => value == regex,
    }
}

pub fn java_string_replace_first<R: AsRef<str>, T: AsRef<str>>(
    value: &str,
    regex: R,
    replacement: T,
) -> String {
    let regex = regex.as_ref();
    let replacement = replacement.as_ref();
    value.replacen(java_simple_regex_literal(regex), replacement, 1)
}

pub fn java_string_replace_all<R: AsRef<str>, T: AsRef<str>>(
    value: &str,
    regex: R,
    replacement: T,
) -> String {
    let regex = regex.as_ref();
    let replacement = replacement.as_ref();
    match regex {
        "[^0-9]" => value
            .chars()
            .fold(String::new(), |mut out, ch| {
                if ch.is_ascii_digit() {
                    out.push(ch);
                } else {
                    out.push_str(replacement);
                }
                out
            }),
        "\\s+" => value.split_whitespace().collect::<Vec<_>>().join(replacement),
        regex => value.replace(java_simple_regex_literal(regex), replacement),
    }
}

fn java_simple_regex_literal(regex: &str) -> &str {
    match regex {
        "\\t" | "\\\\t" => "\t",
        "\\n" | "\\\\n" => "\n",
        "\\r" | "\\\\r" => "\r",
        other => other,
    }
}

RUST
  printf "java_string_substring\njava_string_hash_code\njava_list_hash_code\njava_set_hash_code\njava_map_hash_code\njava_value_hash_code\njava_string_index_of\njava_string_index_of_from\njava_string_last_index_of\njava_string_last_index_of_from\njava_string_compare_to\njava_string_compare_to_ignore_case\njava_string_equals_ignore_case\njava_primitive_compare_to\njava_math_floor_div_i32\njava_math_floor_mod_i32\njava_math_round_f32\njava_math_round_f64\njava_math_max_f32\njava_math_max_f64\njava_math_min_f32\njava_math_min_f64\njava_parse_i8\njava_parse_i16\njava_parse_i32\njava_parse_i64\njava_parse_f32\njava_parse_f64\njava_string_from_byte_range\njava_string_from_bytes\njava_string_split\njava_string_matches\njava_string_replace_first\njava_string_replace_all\n" >> "$shared_support_types_path"
fi

if grep -R -q 'java_is_serializable' "$generated_dir"; then
  cat >> "$shared_support_path" <<'RUST'
pub fn java_is_serializable<T: ?Sized>(_value: &T) -> bool {
    true
}

RUST
  printf "java_is_serializable\n" >> "$shared_support_types_path"
fi

if grep -R -q -E 'java_(clone_erased_object|erased_object_to_string)' "$generated_dir"; then
  cat >> "$shared_support_path" <<'RUST'
pub fn java_clone_erased_object_ref(_value: &Box<dyn std::any::Any>) -> Box<dyn std::any::Any> {
    Box::new(())
}

pub fn java_clone_erased_object_vec_ref(
    _value: &Vec<Box<dyn std::any::Any>>,
) -> Vec<Box<dyn std::any::Any>> {
    Vec::new()
}

pub fn java_erased_object_to_string<T: ?Sized>(_value: &T) -> String {
    Default::default()
}

RUST
  printf "java_clone_erased_object_ref\njava_clone_erased_object_vec_ref\njava_erased_object_to_string\n" >> "$shared_support_types_path"
fi

if grep -R -q 'java_string_format' "$generated_dir"; then
  cat >> "$shared_support_path" <<'RUST'
pub fn java_string_format<S: AsRef<str>>(template: S, args: Vec<String>) -> String {
    let mut out = String::new();
    let mut chars = template.as_ref().chars().peekable();
    let mut arg_index = 0usize;
    while let Some(ch) = chars.next() {
        if ch != '%' {
            out.push(ch);
            continue;
        }
        if matches!(chars.peek(), Some('%')) {
            chars.next();
            out.push('%');
            continue;
        }
        while let Some(next) = chars.next() {
            if next.is_ascii_alphabetic() {
                if matches!(next, 's' | 'd' | 'b' | 'c' | 'f') {
                    if let Some(value) = args.get(arg_index) {
                        out.push_str(value);
                    }
                    arg_index += 1;
                } else {
                    out.push('%');
                    out.push(next);
                }
                break;
            }
        }
    }
    out
}

RUST
  printf "java_string_format\n" >> "$shared_support_types_path"
fi

if grep -R -q 'FlatMapFunction' "$generated_dir"; then
  cat >> "$shared_support_path" <<'RUST'
#[derive(Clone, Debug, Default)]
pub struct FlatMapFunction;

RUST
  printf "FlatMapFunction\n" >> "$shared_support_types_path"
fi

if grep -R -q 'OrgBroadinstituteHellbenderUtilsReadReadUtils\|OrgBroadinstituteHellbenderUtilsReadSAMRecordToGATKReadAdapter' "$generated_dir"; then
  cat >> "$shared_support_path" <<'RUST'
pub type OrgBroadinstituteHellbenderUtilsReadReadUtils = ReadUtils;
pub type OrgBroadinstituteHellbenderUtilsReadSAMRecordToGATKReadAdapter = SAMRecordToGATKReadAdapter;

RUST
  printf "OrgBroadinstituteHellbenderUtilsReadReadUtils\nOrgBroadinstituteHellbenderUtilsReadSAMRecordToGATKReadAdapter\n" >> "$shared_support_types_path"
fi

raw_generated_backup_dir="$out_dir/generated-raw-before-support-strip"
rm -rf "$raw_generated_backup_dir"
mkdir -p "$raw_generated_backup_dir"
for rust_path in "$generated_dir"/*.rs; do
  cp "$rust_path" "$raw_generated_backup_dir/$(basename "$rust_path")"
done

if [[ -s "$shared_support_path" ]]; then
  for rust_path in "$generated_dir"/*.rs; do
    strip_shared_support_items "$rust_path" "$shared_support_types_path"
    strip_source_type_facade_stubs "$rust_path" "$source_types_path"
    restore_missing_prelude_support_items "$rust_path" "$raw_generated_backup_dir/$(basename "$rust_path")" "$source_types_path" "$shared_support_types_path"
    restore_missing_source_type_items "$rust_path" "$raw_generated_backup_dir/$(basename "$rust_path")"
  done
else
  touch "$shared_support_path" "$shared_support_types_path"
fi

python3 "$repo_root/scripts/merge-shared-support-facades.py" \
  --generated-dir "$generated_dir" \
  --merge-input-dir "$raw_generated_backup_dir" \
  --shared-support "$shared_support_path" \
  --shared-types "$shared_support_types_path" \
  --conflicts "$shared_support_conflicts_path"

for rust_path in "$generated_dir"/*.rs; do
  restore_missing_source_type_items "$rust_path" "$raw_generated_backup_dir/$(basename "$rust_path")" "$shared_support_types_path"
done

if grep -R -q 'ndarray::' "$generated_dir" "$shared_support_path"; then
  if ! grep -q '^ndarray[[:space:]]*=' "$crate_dir/Cargo.toml"; then
    write_ndarray_shim "$crate_dir/ndarray-shim"
    printf 'ndarray = { path = "ndarray-shim" }\n' >> "$crate_dir/Cargo.toml"
  fi
fi

for i in "${!java_files[@]}"; do
  index="$((i + 1))"
  stem="$(printf '%05d_%s' "$index" "$(basename "${java_files[$i]%.java}")")"
  stem="$(printf '%s' "$stem" | tr -c 'A-Za-z0-9_' '_')"
  rust_path="$generated_dir/$stem.rs"
  awk -v module="generated_$stem" '
    /^pub[[:space:]]+(struct|enum|type|trait)[[:space:]]+[A-Za-z_][A-Za-z0-9_]*/ {
      name = $3
      sub(/[=;{(<:].*/, "", name)
      if (name != "JavaException" &&
          name != "Box" &&
          name != "String" &&
          name != "Vec" &&
          name != "Option" &&
          name != "Result") {
        print module "\t" name
      }
    }
  ' "$rust_path" >> "$module_types_path"
done

if [[ -s "$module_types_path" ]]; then
  awk -F '\t' '{ count[$2]++ } END { for (name in count) print name "\t" count[name] }' \
    "$module_types_path" > "$type_counts_path"
  awk -F '\t' '
    FILENAME == ARGV[1] { count[$1] = $2; next }
    FILENAME == ARGV[2] { shared[$1] = 1; next }
    count[$2] == 1 && !shared[$2] { print $1 "\t" $2 }
  ' "$type_counts_path" "$shared_support_types_path" "$module_types_path" \
    | sort -k2,2 > "$unique_types_path"
else
  touch "$type_counts_path" "$unique_types_path"
fi

for i in "${!java_files[@]}"; do
  index="$((i + 1))"
  stem="$(printf '%05d_%s' "$index" "$(basename "${java_files[$i]%.java}")")"
  stem="$(printf '%s' "$stem" | tr -c 'A-Za-z0-9_' '_')"
  module="generated_$stem"
  rust_path="$generated_dir/$stem.rs"
  import_list="$(
    awk -F '\t' -v module="$module" '
      FILENAME == ARGV[1] {
        if ($1 == module) {
          local[$2] = 1
        }
        next
      }
      FILENAME == ARGV[2] {
        if (!local[$1]) {
          print $1
        }
        next
      }
      FILENAME == ARGV[3] {
        if ($1 != module && !local[$2]) {
          print $2
        }
        next
      }
    ' "$module_types_path" "$shared_support_types_path" "$unique_types_path" \
      | sort -u \
      | paste -sd, -
  )"
  if [[ -n "$import_list" ]]; then
    tmp_import_path="$rust_path.imports"
    {
      printf "use crate::{%s};\n" "$import_list"
      cat "$rust_path"
    } > "$tmp_import_path"
    mv "$tmp_import_path" "$rust_path"
  fi
done

{
  printf "#![allow(dead_code, unused_imports, unused_variables, non_snake_case, non_camel_case_types, non_upper_case_globals)]\n"
  if [[ -s "$shared_support_path" ]]; then
    cat "$shared_support_path"
  fi
  for i in "${!java_files[@]}"; do
    index="$((i + 1))"
    stem="$(printf '%05d_%s' "$index" "$(basename "${java_files[$i]%.java}")")"
    stem="$(printf '%s' "$stem" | tr -c 'A-Za-z0-9_' '_')"
    printf "#[path = \"generated/%s.rs\"]\npub mod generated_%s;\n" "$stem" "$stem"
  done
  if [[ -s "$unique_types_path" ]]; then
    while IFS=$'\t' read -r module type_name; do
      printf "pub use %s::%s;\n" "$module" "$type_name"
    done < "$unique_types_path"
  fi
} > "$crate_dir/src/lib.rs"
assembly_elapsed_ms="$(elapsed_ms_since "$assembly_start_ms")"
printf "crate_assembly\tok\t%s\t%s\n" "$assembly_elapsed_ms" "$crate_dir" >> "$phase_events_path"

compile_status="ok"
cargo_start_ms="$(now_ms)"
if [[ -z "${CARGO_TARGET_DIR:-}" ]]; then
  export CARGO_TARGET_DIR="$repo_root/.tmp/focused-cargo-target"
fi
cargo_args=("check")
if [[ "$verbose_cargo" -eq 0 ]]; then
  cargo_args+=("--quiet")
fi
if [[ "$offline_cargo" -eq 1 ]]; then
  cargo_args+=("--offline")
fi
if (cd "$crate_dir" && cargo "${cargo_args[@]}") > "$compile_log" 2>&1; then
  compile_status="ok"
else
  compile_status="failed"
fi
cargo_elapsed_ms="$(elapsed_ms_since "$cargo_start_ms")"
printf "cargo_check\t%s\t%s\t%s\n" "$compile_status" "$cargo_elapsed_ms" "$compile_log" >> "$phase_events_path"

parse_failure_scan_start_ms="$(now_ms)"
write_parse_failures "$compile_log" "$parse_failures_path"
parse_failure_count="$(parse_failure_count_from_report "$parse_failures_path")"
cargo_type_errors_valid="$(cargo_type_errors_valid_from_parse_failure_count "$parse_failure_count")"
parse_failure_scan_elapsed_ms="$(elapsed_ms_since "$parse_failure_scan_start_ms")"
printf "parse_failure_scan\tok\t%s\t%s\n" "$parse_failure_scan_elapsed_ms" "$parse_failures_path" >> "$phase_events_path"

total_files="${#java_files[@]}"
translated_failed="$(awk -F '\t' 'NR > 1 && $3 != "ok" { count++ } END { print count + 0 }' "$status_path")"
diagnostic_files="$(awk -F '\t' 'NR > 1 && $4 > 0 { count++ } END { print count + 0 }' "$status_path")"
invalid_pattern_files="$(awk -F '\t' 'NR > 1 && $5 > 0 { count++ } END { print count + 0 }' "$status_path")"
compile_error_count="$(grep -E 'error(\[[A-Z0-9]+\])?:' "$compile_log" | wc -l | tr -d ' ')"
cargo_failure_kind="none"
if [[ "$compile_status" != "ok" ]]; then
  if grep -E -q 'failed to get .* as a dependency|failed to download|no matching package named|download of .* failed|Unable to update registry|could not resolve|offline mode|attempting to make an HTTP request' "$compile_log"; then
    cargo_failure_kind="dependency_cache"
  else
    cargo_failure_kind="generated_rust"
  fi
fi
invalid_scan_start_ms="$(now_ms)"
{
  printf "pattern\tcount\tfiles\n"
  for spec in \
    'compile_error_placeholder	compile_error!' \
    'invalid_java_cast_downcast	downcast::<.*>\(\)\.expect\("invalid Java cast"\)' \
    'concrete_none_assignment	= None;' \
    'java_add_all_call	\.add_all\(' \
    'java_put_call	\.put\(' \
    'unsupported_lambda	unsupported Java expression: lambda' \
    'unsupported_method_reference	unsupported Java expression: method reference' \
    'unsupported_array_creation	unsupported Java array creation'
  do
    name="${spec%%	*}"
    pattern="${spec#*	}"
    if [[ "$name" == "concrete_none_assignment" ]]; then
      count="$(count_concrete_none_assignments "$generated_dir")"
      files="$(count_concrete_none_assignment_files "$generated_dir")"
    else
      count="$(grep -R -E "$pattern" "$generated_dir" | wc -l | tr -d ' ')"
      files="$(grep -R -l -E "$pattern" "$generated_dir" | wc -l | tr -d ' ')"
    fi
    printf "%s\t%s\t%s\n" "$name" "$count" "$files"
  done
} > "$invalid_patterns_path"
invalid_scan_elapsed_ms="$(elapsed_ms_since "$invalid_scan_start_ms")"
printf "invalid_pattern_scan\tok\t%s\t%s\n" "$invalid_scan_elapsed_ms" "$invalid_patterns_path" >> "$phase_events_path"
{
  printf "count\terror\n"
  grep -E '^error(\[[A-Z0-9]+\])?:' "$compile_log" \
    | sed 's/[[:space:]]\+/ /g' \
    | sort \
    | uniq -c \
    | sort -nr \
    | awk '{ count = $1; $1 = ""; sub(/^ /, ""); gsub(/\t/, " "); printf "%s\t%s\n", count, $0 }'
} > "$top_compile_errors"

idiom_sweep_start_ms="$(now_ms)"
python3 "$repo_root/scripts/sweep-generated-idioms.py" \
  --generated-root "$generated_dir" \
  --status "$status_path" \
  --out-tsv "$idiom_sweep_tsv" \
  --out-md "$idiom_sweep_md"
idiom_sweep_elapsed_ms="$(elapsed_ms_since "$idiom_sweep_start_ms")"
printf "idiom_sweep\tok\t%s\t%s\n" "$idiom_sweep_elapsed_ms" "$idiom_sweep_md" >> "$phase_events_path"
unsupported_statement_count="$(idiom_signal_count "$idiom_sweep_tsv" "unsupported_statements")"
unsupported_member_count="$(( $(idiom_signal_count "$idiom_sweep_tsv" "unsupported_members") + $(status_column_sum "$status_path" "unsupported_members") ))"
unsupported_expression_count="$(idiom_signal_count "$idiom_sweep_tsv" "unsupported_expressions")"
unmodelled_type_count="$(idiom_signal_count "$idiom_sweep_tsv" "unmodelled_types")"
unmodelled_call_site_count="$(idiom_signal_count "$idiom_sweep_tsv" "unmodelled_call_sites")"
noop_facade_method_count="$(idiom_signal_count "$idiom_sweep_tsv" "noop_facade_methods")"
stubbed_constant_count="$(idiom_signal_count "$idiom_sweep_tsv" "stubbed_constants")"
total_elapsed_ms="$(elapsed_ms_since "$script_start_ms")"
printf "total\tok\t%s\t%s\n" "$total_elapsed_ms" "$out_dir" >> "$phase_events_path"

{
  printf "elapsed_ms\tphase\tstatus\tdetail\n"
  awk -F '\t' 'NR > 1 { gsub(/\t/, " ", $4); printf "%s\t%s\t%s\t%s\n", $3, $1, $2, $4 }' "$phase_events_path" \
    | sort -nr \
    | head -20
} > "$slowest_phases_path"

{
  printf "elapsed_ms\tevent\tstatus\tindex\tsource\trust_path\n"
  awk -F '\t' 'NR > 1 && $5 > 0 { gsub(/\t/, " ", $4); gsub(/\t/, " ", $7); printf "%s\t%s\t%s\t%s\t%s\t%s\n", $5, $1, $6, $3, $4, $7 }' "$worker_events_path" \
    | sort -nr \
    | head -20
} > "$slowest_workers_path"

{
  printf "primary\t%s\n" "$primary"
  printf "out_dir\t%s\n" "$out_dir"
  printf "generated_crate\t%s\n" "$crate_dir"
  printf "translator_binary\t%s\n" "$binary"
  printf "cargo_target_dir\t%s\n" "${CARGO_TARGET_DIR:-}"
  printf "files\t%s\n" "$total_files"
  printf "resolver_bench\t%s\n" "$resolver_status"
  printf "resolver_elapsed_ms\t%s\n" "$resolver_elapsed_ms"
  printf "translation_elapsed_ms\t%s\n" "$translation_elapsed_ms"
  printf "crate_assembly_elapsed_ms\t%s\n" "$assembly_elapsed_ms"
  printf "cargo_check_elapsed_ms\t%s\n" "$cargo_elapsed_ms"
  printf "parse_failure_scan_elapsed_ms\t%s\n" "$parse_failure_scan_elapsed_ms"
  printf "invalid_scan_elapsed_ms\t%s\n" "$invalid_scan_elapsed_ms"
  printf "idiom_sweep_elapsed_ms\t%s\n" "$idiom_sweep_elapsed_ms"
  printf "total_elapsed_ms\t%s\n" "$total_elapsed_ms"
  printf "translate_failed_files\t%s\n" "$translated_failed"
  printf "diagnostic_files\t%s\n" "$diagnostic_files"
  printf "parse_failures\t%s\n" "$parse_failure_count"
  printf "parse_failures_tsv\t%s\n" "$parse_failures_path"
  printf "cargo_type_errors_valid\t%s\n" "$cargo_type_errors_valid"
  printf "unsupported_statements\t%s\n" "$unsupported_statement_count"
  printf "unsupported_members\t%s\n" "$unsupported_member_count"
  printf "unsupported_expressions\t%s\n" "$unsupported_expression_count"
  printf "unmodelled_types\t%s\n" "$unmodelled_type_count"
  printf "unmodelled_call_sites\t%s\n" "$unmodelled_call_site_count"
  printf "noop_facade_methods\t%s\n" "$noop_facade_method_count"
  printf "stubbed_constants\t%s\n" "$stubbed_constant_count"
  printf "invalid_pattern_files\t%s\n" "$invalid_pattern_files"
  printf "invalid_patterns\t%s\n" "$invalid_patterns_path"
  if [[ -f "$batch_manifest_path" ]]; then
    printf "batch_manifest\t%s\n" "$batch_manifest_path"
  fi
  if [[ -f "$analysis_profile_path" ]]; then
    printf "analysis_profile\t%s\n" "$analysis_profile_path"
  fi
  printf "idiom_sweep\t%s\n" "$idiom_sweep_md"
  printf "idiom_sweep_tsv\t%s\n" "$idiom_sweep_tsv"
  printf "cargo_check\t%s\n" "$compile_status"
  printf "cargo_check_errors\t%s\n" "$compile_error_count"
  printf "cargo_failure_kind\t%s\n" "$cargo_failure_kind"
  printf "cargo_offline\t%s\n" "$offline_cargo"
  printf "top_compile_errors\t%s\n" "$top_compile_errors"
  printf "slowest_phases\t%s\n" "$slowest_phases_path"
  printf "slowest_workers\t%s\n" "$slowest_workers_path"
  printf "phase_events\t%s\n" "$phase_events_path"
  printf "worker_events\t%s\n" "$worker_events_path"
} > "$out_dir/summary.tsv"

cat "$out_dir/summary.tsv"

if [[ "$compile_status" != "ok" || "$translated_failed" -ne 0 || "$invalid_pattern_files" -ne 0 ]]; then
  exit 1
fi
