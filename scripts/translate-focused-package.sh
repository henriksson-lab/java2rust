#!/usr/bin/env bash
set -u
set -o pipefail

usage() {
  cat >&2 <<'USAGE'
usage: scripts/translate-focused-package.sh --primary <java-dir> --out <out-dir> [options]

Options:
  --source-root <mode:path>           Add a resolver-bench source root preflight input.
  --focus-list <path>                 Emit only Java files named in this newline-delimited list.
                                      Relative entries are resolved below --primary; the complete
                                      primary tree remains source-summary dependency context.
  --emit-list <path>                  Translation-only profiling mode: lower this subset of
                                      --focus-list while retaining the complete focus set in
                                      global analysis. Skips crate assembly and cargo check.
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
  --resume-assembly                   Reuse a completed translation in --out and resume crate assembly.
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
focus_list=""
emit_list=""
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
resume_assembly=0

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
    --focus-list)
      [[ $# -ge 2 ]] || { echo "missing value for --focus-list" >&2; exit 2; }
      focus_list="$2"
      shift 2
      ;;
    --emit-list)
      [[ $# -ge 2 ]] || { echo "missing value for --emit-list" >&2; exit 2; }
      emit_list="$2"
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
    --resume-assembly)
      resume_assembly=1
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
if [[ -n "$focus_list" && ! -f "$focus_list" ]]; then
  echo "focus list does not exist: $focus_list" >&2
  exit 1
fi
if [[ -n "$emit_list" && ! -f "$emit_list" ]]; then
  echo "emit list does not exist: $emit_list" >&2
  exit 1
fi
if [[ -n "$emit_list" && -z "$focus_list" ]]; then
  echo "--emit-list requires --focus-list so the analyzed dependency set is explicit" >&2
  exit 2
fi
if [[ -n "$emit_list" && "$batch_translate" -ne 1 ]]; then
  echo "--emit-list requires batch translation" >&2
  exit 2
fi
if [[ -n "$emit_list" && "$resume_assembly" -eq 1 ]]; then
  echo "--emit-list is translation-only and cannot be combined with --resume-assembly" >&2
  exit 2
fi
if [[ -n "$focus_list" ]]; then
  # Batch translation below already analyzes the complete primary tree as
  # source-summary context. Do not repeat a full resolver benchmark on every
  # small focused iteration.
  run_resolver_bench=0
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
    /// Java's `array.length` on a two-dimensional array reports the outer
    /// dimension. Generated Java array code therefore uses `len()` for rows,
    /// not the total scalar element count used by ndarray proper.
    pub fn len(&self) -> usize {
        self.rows
    }

    pub fn is_empty(&self) -> bool {
        self.rows == 0
    }

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

    /// Iterate the Java outer-array dimension without flattening the matrix.
    /// This mirrors ndarray's row-lane API used by enhanced-for lowering.
    pub fn rows(&self) -> Rows<'_, T> {
        Rows {
            matrix: self,
            row: 0,
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

impl<T> Index<usize> for Array2<T> {
    type Output = [T];

    fn index(&self, row: usize) -> &Self::Output {
        let start = row * self.cols;
        &self.data[start..start + self.cols]
    }
}

impl<T> IndexMut<usize> for Array2<T> {
    fn index_mut(&mut self, row: usize) -> &mut Self::Output {
        let start = row * self.cols;
        &mut self.data[start..start + self.cols]
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

    pub fn fill(&mut self, value: T) {
        self.data.fill(value);
    }
}

impl<'a, T> RowMut<'a, T> {
    pub fn as_slice_mut(&mut self) -> Option<&mut [T]> {
        Some(self.data)
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
if [[ "$resume_assembly" -eq 0 ]]; then
  rm -f "$status_path" "$batch_manifest_path" "$analysis_profile_path" "$compile_log" "$invalid_patterns_path" "$parse_failures_path" "$top_compile_errors" "$resolver_log" "$idiom_sweep_tsv" "$idiom_sweep_md" "$phase_events_path" "$worker_events_path" "$slowest_phases_path" "$slowest_workers_path"
  find "$generated_dir" -type f -name '*.rs' -delete
  printf "phase\tstatus\telapsed_ms\tdetail\n" > "$phase_events_path"
  printf "event\tpid\tindex\tsource\telapsed_ms\tstatus\trust_path\n" > "$worker_events_path"
else
  raw_resume_dir="$out_dir/generated-raw-before-support-strip"
  if [[ ! -s "$status_path" || ! -d "$raw_resume_dir" || ! -f "$crate_dir/Cargo.toml" ]]; then
    echo "--resume-assembly requires completed translation artifacts in: $out_dir" >&2
    exit 1
  fi
  find "$generated_dir" -type f -name '*.rs' -delete
  cp "$raw_resume_dir"/*.rs "$generated_dir"/
  printf "assembly_resume\trunning\t0\t%s\n" "$out_dir" >> "$phase_events_path"
fi

translator_build_elapsed_ms=0
if [[ "$resume_assembly" -eq 1 ]]; then
  :
elif [[ -n "${NEW_JAVA2RUST_BIN:-}" ]]; then
  if [[ ! -x "$binary" ]]; then
    echo "NEW_JAVA2RUST_BIN is not executable: $binary" >&2
    exit 1
  fi
else
  # The focused package gate is an end-to-end translator test.  An existing
  # release executable may predate the source changes under test, so let Cargo
  # perform its incremental freshness check on every default invocation.
  translator_build_start_ms="$(now_ms)"
  cargo build --quiet "${build_profile_args[@]}" --bin new-java2rust
  translator_build_elapsed_ms="$(elapsed_ms_since "$translator_build_start_ms")"
  printf "translator_build\tok\t%s\t%s\n" "$translator_build_elapsed_ms" "$binary" >> "$phase_events_path"
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

if [[ "$resume_assembly" -eq 0 ]]; then
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
fi

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
if [[ "$resume_assembly" -eq 0 && "$run_resolver_bench" -eq 1 ]]; then
  resolver_start_ms="$(now_ms)"
  if "$binary" "${resolver_args[@]}" > "$resolver_log" 2>&1; then
    resolver_status="ok"
  else
    resolver_status="failed"
  fi
  resolver_elapsed_ms="$(elapsed_ms_since "$resolver_start_ms")"
fi
if [[ "$resume_assembly" -eq 0 ]]; then
  printf "resolver_bench\t%s\t%s\t%s\n" "$resolver_status" "$resolver_elapsed_ms" "$resolver_log" >> "$phase_events_path"
fi

if [[ "$resume_assembly" -eq 0 ]]; then
  printf "source\trust_path\ttranslate_status\tdiagnostics\tinvalid_patterns\tunsupported_members\tstubbed_constants\n" > "$status_path"
fi
if [[ -n "$focus_list" ]]; then
  java_files=()
  while IFS= read -r selected || [[ -n "$selected" ]]; do
    selected="${selected%$'\r'}"
    [[ -z "$selected" || "$selected" == \#* ]] && continue
    if [[ "$selected" = /* ]]; then
      selected_path="$selected"
    else
      selected_path="$primary/$selected"
    fi
    if [[ ! -f "$selected_path" || "$selected_path" != *.java ]]; then
      echo "focus-list entry is not a Java source file: $selected_path" >&2
      exit 1
    fi
    java_files+=("$selected_path")
  done < "$focus_list"
  if [[ "${#java_files[@]}" -gt 0 ]]; then
    mapfile -t java_files < <(printf '%s\n' "${java_files[@]}" | sort -u)
  fi
else
  mapfile -t java_files < <(find "$primary" -type f -name '*.java' | sort)
fi

if [[ "${#java_files[@]}" -eq 0 ]]; then
  echo "no Java files found under: $primary" >&2
  exit 1
fi
analyzed_file_count="${#java_files[@]}"

emit_java_files=()
if [[ -n "$emit_list" ]]; then
  while IFS= read -r selected || [[ -n "$selected" ]]; do
    selected="${selected%$'\r'}"
    [[ -z "$selected" || "$selected" == \#* ]] && continue
    if [[ "$selected" = /* ]]; then
      selected_path="$selected"
    else
      selected_path="$primary/$selected"
    fi
    if [[ ! -f "$selected_path" || "$selected_path" != *.java ]]; then
      echo "emit-list entry is not a Java source file: $selected_path" >&2
      exit 1
    fi
    if [[ ! " ${java_files[*]} " =~ " ${selected_path} " ]]; then
      echo "emit-list entry is not in the analyzed focus set: $selected_path" >&2
      exit 1
    fi
    emit_java_files+=("$selected_path")
  done < "$emit_list"
  if [[ "${#emit_java_files[@]}" -gt 0 ]]; then
    mapfile -t emit_java_files < <(printf '%s\n' "${emit_java_files[@]}" | sort -u)
  else
    echo "emit list contains no Java files: $emit_list" >&2
    exit 1
  fi
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
  awk -F '\t' -v signal="$signal" '
    NR > 1 && $1 == signal {
      found = 1
      print $2
    }
    END {
      if (!found) {
        printf "missing idiom signal: %s\n", signal > "/dev/stderr"
        exit 1
      }
    }
  ' "$path"
}

analysis_profile_metric() {
  local path="$1"
  local metric="$2"
  if [[ ! -f "$path" ]]; then
    printf 'missing analysis profile: %s\n' "$path" >&2
    return 1
  fi
  awk -F '\t' -v metric="$metric" '
    NR > 1 && $1 == metric {
      found = 1
      print $2
    }
    END {
      if (!found) {
        printf "missing analysis profile metric: %s\n", metric > "/dev/stderr"
        exit 1
      }
    }
  ' "$path"
}

analysis_profile_metric_or_idiom_signal() {
  local profile_path="$1"
  local idiom_path="$2"
  local metric="$3"
  local value
  if [[ -f "$profile_path" ]]; then
    value="$(analysis_profile_metric "$profile_path" "$metric")" || return 1
    printf '%s\n' "$value"
    return
  fi
  idiom_signal_count "$idiom_path" "$metric"
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
    NR == 2 && target == 0 {
      printf "missing status column: %s\n", column > "/dev/stderr"
      exit 1
    }
    target > 0 {
      sum += $target
    }
    END {
      if (target == 0) {
        printf "missing status column: %s\n", column > "/dev/stderr"
        exit 1
      }
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
  invalid_patterns="$("$binary" count-invalid-patterns "$rust_path")" || exit 1
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

translation_elapsed_ms=0
if [[ "$resume_assembly" -eq 0 ]]; then
status_fragments_dir="$out_dir/status-fragments"
mkdir -p "$status_fragments_dir"
find "$status_fragments_dir" -type f -name '*.tsv' -delete

active_jobs=0
translation_start_ms="$(now_ms)"
if [[ "$batch_translate" -eq 1 ]]; then
  batch_args=("translate-batch" "--primary" "$primary" "--out-dir" "$out_dir" "--generated-dir" "$generated_dir" "--jobs" "$jobs" "--phase-events" "$phase_events_path" "--worker-events" "$worker_events_path" "--context-dir-emitted")
  if [[ -n "$focus_list" ]]; then
    for java_file in "${java_files[@]}"; do
      batch_args+=("--primary-file" "$java_file")
    done
    batch_args+=("--context-source-summary-dir" "$primary")
  fi
  for java_file in "${emit_java_files[@]}"; do
    batch_args+=("--emit-primary-file" "$java_file")
  done
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
if [[ "${#emit_java_files[@]}" -gt 0 ]]; then
  # Translation indices and generated filenames are assigned over the emitted
  # subset. Assembly must therefore iterate that same set, while the complete
  # focus set above remains the analyzed primary set passed to translate-batch.
  java_files=("${emit_java_files[@]}")
fi
translation_elapsed_ms="$(elapsed_ms_since "$translation_start_ms")"
printf "translation\tok\t%s\t%s files\n" "$translation_elapsed_ms" "${#java_files[@]}" >> "$phase_events_path"
else
  printf "translation_reuse\tok\t0\t%s files\n" "${#java_files[@]}" >> "$phase_events_path"
fi
if [[ "${#emit_java_files[@]}" -gt 0 ]]; then
  # Resume skips the translation branch above, but must retain the same
  # emitted-subset indexing during assembly.
  java_files=("${emit_java_files[@]}")
fi

if [[ "${#emit_java_files[@]}" -gt 0 ]]; then
  emitted_file_count="$(awk -F '\t' 'NR > 1 { count++ } END { print count + 0 }' "$status_path")"
  raw_generated_backup_dir="$out_dir/generated-raw-before-support-strip"
  mkdir -p "$raw_generated_backup_dir"
  find "$raw_generated_backup_dir" -type f -name '*.rs' -delete
  cp "$generated_dir"/*.rs "$raw_generated_backup_dir"/
  printf "translation_subset_only\tok\t0\t%s emitted from %s analyzed files\n" \
    "$emitted_file_count" "$analyzed_file_count" >> "$phase_events_path"
  printf "primary\t%s\n" "$primary"
  printf "out_dir\t%s\n" "$out_dir"
  printf "files_analyzed\t%s\n" "$analyzed_file_count"
  printf "files_emitted\t%s\n" "$emitted_file_count"
  printf "translation_elapsed_ms\t%s\n" "$translation_elapsed_ms"
  printf "assembly\tskipped\n"
  printf "cargo_check\tskipped\n"
  printf "phase_events\t%s\n" "$phase_events_path"
  printf "worker_events\t%s\n" "$worker_events_path"
  exit 0
fi

assembly_start_ms="$(now_ms)"
assembly_java_files=()
assembly_rust_paths=()
while IFS=$'\t' read -r source rust_path _; do
  [[ "$source" == "source" || -z "$source" ]] && continue
  assembly_java_files+=("$source")
  assembly_rust_paths+=("$rust_path")
done < "$status_path"
module_types_path="$out_dir/generated-module-types.tsv"
type_counts_path="$out_dir/generated-type-counts.tsv"
unique_types_path="$out_dir/generated-unique-types.tsv"
source_types_path="$out_dir/generated-source-types.tsv"
shared_support_path="$out_dir/generated-shared-support.rs"
shared_support_types_path="$out_dir/generated-shared-support-types.tsv"
shared_support_conflicts_path="$out_dir/generated-shared-support-conflicts.tsv"
rm -f "$module_types_path" "$type_counts_path" "$unique_types_path" "$source_types_path" "$shared_support_path" "$shared_support_types_path" "$shared_support_conflicts_path"

for i in "${!assembly_java_files[@]}"; do
  stem="$(basename "${assembly_java_files[$i]%.java}")"
  stem="$(printf '%s' "$stem" | tr -c 'A-Za-z0-9_' '_')"
  printf "%s\n" "$stem" >> "$source_types_path"
  rust_path="${assembly_rust_paths[$i]}"
  if [[ -f "$rust_path" ]]; then
    awk -v outer="$stem" '
      /^pub[[:space:]]+(struct|enum|trait|type)[[:space:]]+[A-Za-z_][A-Za-z0-9_]*/ {
        name = $3
        sub(/[=;{(<:].*/, "", name)
        if (name == outer) {
          after_primary = 1
          next
        }
        prefix = outer "_"
        if (index(name, prefix) == 1) {
          suffix = substr(name, length(prefix) + 1)
          if (suffix ~ /^[A-Z]/) print name
        } else if (after_primary) {
          # Java permits additional package-private top-level declarations in
          # the same source file. The translator emits those after the
          # filename primary declaration; record them as source-owned so
          # late dependency restoration cannot resurrect a prelude facade
          # that shadows their real constructors and methods.
          print name
        }
      }
    ' "$rust_path" >> "$source_types_path"
  fi
done
sort -u -o "$source_types_path" "$source_types_path"

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
      if ($0 ~ /^impl([[:space:]]*<[^{}]+>)?[[:space:]]+.*[[:space:]]+for[[:space:]]+&?([[:space:]]*)?([A-Za-z_][A-Za-z0-9_]*::)*[A-Za-z_][A-Za-z0-9_]*(<.*>)?[[:space:]]*\{/) {
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
        impl_target_raw = impl_target
        sub(/[[:space:]]*\{.*/, "", impl_target_raw)
        sub(/(<.*>)?[[:space:]]*\{.*/, "", impl_target)
        target_is_shared = shared[impl_target] && impl_target != source_type
        keep_module_from_impl_to_shared = target_is_shared && from_trait_source != "" && !shared[from_trait_source]
        source_owned_shared_adapter = index(impl_target, "__JavaShared" source_type "As") == 1
        source_owned_wrapper = impl_target_raw ~ ("(^|::)(Rc|Arc)[[:space:]]*<[[:space:]]*" source_type "[[:space:]]*>$")
        if (impl_target == "JavaException" || impl_target == "JavaClass" || impl_target == "FileSystem" || impl_target == "Path" || impl_target == "URI" || impl_target == "File" || impl_target == "URL" || impl_target == "URLConnection" || impl_target == "JarURLConnection" || impl_target == "JarFile" || impl_target == "JarEntry" || impl_target == "Manifest" || impl_target == "Attributes" || impl_target == "AttributesName" || impl_target == "Enumeration" || impl_target == "JavaIterator" || impl_target == "IOUtils" || (target_is_shared && !keep_module_from_impl_to_shared) || (shared[impl_trait] && impl_trait != source_type && !source_owned_shared_adapter && !source_owned_wrapper)) {
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
  local rust_item_utils_dir="${3:-}"
  python3 - "$rust_path" "$source_types_path" "$rust_item_utils_dir" <<'PY'
import re
import sys
from pathlib import Path

if not sys.argv[3]:
    raise SystemExit("strip_source_type_facade_stubs requires the shared Rust item utility directory")
sys.path.insert(0, sys.argv[3])
from rust_item_utils import impl_header_parts, impl_target_name, inherent_impl_target_name

rust_path = Path(sys.argv[1])
source_types_path = Path(sys.argv[2])

stem = rust_path.stem
current_source = re.sub(r"^\d+_", "", stem)
source_types = {
    line.strip()
    for line in source_types_path.read_text().splitlines()
    if line.strip()
}
source_types = {
    name
    for name in source_types
    if name != current_source
    and not re.match(rf"^{re.escape(current_source)}[_$][A-Z]", name)
}

lines = rust_path.read_text().splitlines(keepends=True)
decl_re = re.compile(r"^pub\s+(struct|enum|trait|type)\s+([A-Za-z_][A-Za-z0-9_]*)")
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
            target = inherent_impl_target_name(line)
            if target is None and (parts := impl_header_parts(line)) is not None:
                target = impl_target_name(parts[1])

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
  local rust_item_utils_dir="${5:-}"
  python3 - "$rust_path" "$raw_path" "$source_types_path" "$shared_types_path" "$rust_item_utils_dir" <<'PY'
import re
import sys
from pathlib import Path

if not sys.argv[5]:
    raise SystemExit("restore_missing_prelude_support_items requires the shared Rust item utility directory")
sys.path.insert(0, sys.argv[5])
from rust_item_utils import impl_header_parts, impl_target_name

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
static_re = re.compile(
    r"^(?:(?:pub\s+)?static\s+|std::thread_local!\s*\{\s*static\s+)"
    r"([A-Za-z_][A-Za-z0-9_]*)\b"
)
derive_re = re.compile(r"^#\[derive")


def source_owned_nested_type_name(name: str) -> bool:
    if name == current_source or not name.startswith(current_source):
        return False
    suffix = name[len(current_source):]
    # Nested source declarations use the collision-free flattened spelling
    # `Outer_Child` (and legacy inputs may use `Outer$Child`). A concatenated
    # top-level name such as `SequenceInputStream` is not evidence of nesting.
    nested_suffix = suffix[1:] if suffix.startswith(("_", "$")) else ""
    return bool(nested_suffix) and nested_suffix[0].isupper()


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


def imported_top_level_names(lines: list[str]) -> set[str]:
    """Names made available by flat `use crate::{...}` module imports.

    Focused-package assembly writes this import surface before reconciliation.
    A helper imported from the merged shared facade already satisfies a
    restored source method and must not also be copied back as a local item.
    """
    names = set()
    depth = 0
    index = 0
    while index < len(lines):
        line = lines[index]
        if depth == 0 and re.match(r"^\s*use\s+crate(?:::\{?|\s)", line):
            statement = line
            while ";" not in statement and index + 1 < len(lines):
                index += 1
                statement += lines[index]
            body = re.sub(r"^\s*use\s+crate::?", "", statement.strip())
            body = body.rstrip(";").strip()
            if body.startswith("{") and body.endswith("}"):
                body = body[1:-1]
            for item in body.split(","):
                item = item.strip()
                if not item:
                    continue
                if " as " in item:
                    leaf = item.rsplit(" as ", 1)[1].strip()
                else:
                    leaf = item.rsplit("::", 1)[-1].strip()
                if re.fullmatch(r"[A-Za-z_][A-Za-z0-9_]*", leaf):
                    names.add(leaf)
            index += 1
            continue
        depth += brace_delta(line)
        index += 1
    return names


def referenced_function_names(text: str, candidate_names: set[str]) -> set[str]:
    return candidate_names.intersection(
        re.findall(r"\b[A-Za-z_][A-Za-z0-9_]*\b", text)
    )


def support_item_name(line: str):
    if match := decl_re.match(line):
        return match.group(2)
    if match := alias_re.match(line):
        return match.group(1)
    if (parts := impl_header_parts(line)) is not None:
        return impl_target_name(parts[1])
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
current_lines = current.splitlines(keepends=True)
present_functions = set(top_level_function_items(current_lines))
present_functions.update(imported_top_level_names(current_lines))
# Shared facade names are imported only after reconciliation, but they are
# already satisfied at crate scope.  Treat them as present now so dependency
# closure restores only helpers absent from both the module and the facade.
present_functions.update(shared_types)
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

deduplicate_generated_coherence_items() {
  local rust_path="$1"
  local rust_item_utils_dir="${2:-}"
  local shared_support_path="${3:-}"
  python3 - "$rust_path" "$rust_item_utils_dir" "$shared_support_path" <<'PY'
import re
import sys
from pathlib import Path

if not sys.argv[2]:
    raise SystemExit("deduplicate_generated_coherence_items requires the shared Rust item utility directory")
sys.path.insert(0, sys.argv[2])
from rust_item_utils import impl_header_parts

path = Path(sys.argv[1])
lines = path.read_text().splitlines(keepends=True)
shared_support_path = Path(sys.argv[3]) if len(sys.argv) > 3 and sys.argv[3] else None
static_re = re.compile(
    r"^(?:(?:pub\s+)?static\s+|std::thread_local!\s*\{\s*static\s+)"
    r"([A-Za-z_][A-Za-z0-9_]*)\b"
)


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
        if state in {"string", "char"}:
            if ch == "\\":
                index += 2
                continue
            if (state == "string" and ch == '"') or (state == "char" and ch == "'"):
                state = "code"
            index += 1
            continue
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


# Source-surface restoration is intentionally repeatable at multiple assembly
# boundaries.  Keep the first top-level storage item and the first exact Rust
# coherence key in a module; later identical keys cannot coexist in Rust even
# when their bodies happen to differ.  Inherent impls and unique companion
# impls (notably Clone for Box<dyn SourceTrait>) are outside this filter.
seen = set()
# Rust coherence is crate-wide, not module-local. Shared support is included at
# the crate root before source modules, so an exact `(trait, target)` already
# present there owns the key. Seed the module cleanup with those keys; otherwise
# the final source-surface restoration can reintroduce a duplicate such as
# `ObservationBound for ObservationDiscrete<E>` after its earlier guard ran.
if shared_support_path and shared_support_path.exists():
    for shared_line in shared_support_path.read_text().splitlines():
        parts = impl_header_parts(shared_line)
        if parts is not None and parts[0] is not None:
            seen.add(("trait_impl", parts[0], parts[1]))
remove = set()
depth = 0
index = 0
while index < len(lines):
    line = lines[index]
    key = None
    if depth == 0:
        if match := static_re.match(line):
            key = ("static", match.group(1))
        elif (parts := impl_header_parts(line)) is not None and parts[0] is not None:
            key = ("trait_impl", parts[0], parts[1])
    if key is not None:
        end = item_end(index)
        if key in seen:
            remove.update(range(index, end))
        else:
            seen.add(key)
        index = end
        depth = 0
        continue
    depth += brace_delta(line)
    index += 1

if remove:
    path.write_text("".join(line for index, line in enumerate(lines) if index not in remove))
PY
}

restore_missing_source_type_items() {
  local rust_path="$1"
  local raw_path="$2"
  local shared_types_path="${3:-}"
  local shared_support_path="${4:-}"
  local rust_item_utils_dir="${5:-}"
  python3 - "$rust_path" "$raw_path" "$shared_types_path" "$shared_support_path" "$rust_item_utils_dir" <<'PY'
import re
import sys
from pathlib import Path

if not sys.argv[5]:
    raise SystemExit("restore_missing_source_type_items requires the shared Rust item utility directory")
sys.path.insert(0, sys.argv[5])
from rust_item_utils import impl_header_parts, impl_target_name, inherent_impl_target_name

rust_path = Path(sys.argv[1])
raw_path = Path(sys.argv[2])
shared_types_path = Path(sys.argv[3]) if len(sys.argv) > 3 and sys.argv[3] else None
shared_support_path = Path(sys.argv[4]) if len(sys.argv) > 4 and sys.argv[4] else None

if not raw_path.exists():
    raise SystemExit(0)

stem = rust_path.stem
current_source = re.sub(r"^\d+_", "", stem)
decl_re = re.compile(r"^pub\s+(struct|enum|trait|type)\s+([A-Za-z_][A-Za-z0-9_]*)")
derive_re = re.compile(r"^#\[derive")
alias_re = re.compile(r"^pub\s+type\s+([A-Za-z_][A-Za-z0-9_]*)\s*=\s*([A-Za-z_][A-Za-z0-9_]*)\s*;")
source_alias_re = re.compile(
    r"^pub\s+type\s+([A-Za-z_][A-Za-z0-9_]*)"
    r"(?:\s*<[^=]*>)?\s*=\s*"
    r"(?:(?:[A-Za-z_][A-Za-z0-9_]*)::)*([A-Za-z_][A-Za-z0-9_]*)"
)
static_re = re.compile(
    r"^(?:(?:pub\s+)?static\s+|std::thread_local!\s*\{\s*static\s+)"
    r"([A-Za-z_][A-Za-z0-9_]*)\b"
)
method_re = re.compile(
    r"^\s*(?:pub(?:\(crate\))?\s+)?(?:const\s+)?fn\s+([A-Za-z_][A-Za-z0-9_]*)\b"
)


def source_owned_nested_type_name(name: str) -> bool:
    if name == current_source or not name.startswith(current_source):
        return False
    suffix = name[len(current_source):]
    nested_suffix = suffix[1:] if suffix.startswith(("_", "$")) else ""
    return bool(nested_suffix) and nested_suffix[0].isupper()


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
# The primary declaration is an ownership boundary in translator output.
# Nominal declarations emitted after it are representation companions of that
# source type (for example, the enum carrier selected for a source hierarchy),
# not reusable prelude support.  Preserve them together with the primary and
# structurally identified nested source declarations.
post_primary_names = set()
after_primary = False
for line in raw_lines:
    if match := decl_re.match(line):
        name = match.group(2)
        if name == current_source:
            after_primary = True
        if after_primary:
            post_primary_names.add(name)

source_owned_names = {
    match.group(2)
    for line in raw_lines
    if (match := decl_re.match(line))
    and (
        match.group(2) == current_source
        or source_owned_nested_type_name(match.group(2))
        or match.group(2) in post_primary_names
    )
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
# A flattened nested source declaration is often followed by its Java-visible
# short-name alias (`type Arguments = Owner_Arguments`).  The alias is part of
# the source API even when support merging indexed it as a duplicate.  Close
# ownership transitively so alias chains and generic aliases stay beside the
# nominal declaration they name.
while True:
    aliases = {
        match.group(1)
        for line in raw_lines
        if (match := source_alias_re.match(line))
        and match.group(2) in source_owned_names
    }
    new_aliases = aliases - source_owned_names
    if not new_aliases:
        break
    source_owned_names.update(new_aliases)
missing_names = source_owned_names - present
# A wholly missing source declaration is restored below together with all of
# its attached impls. Surface-level reconciliation is only for declarations
# that survived assembly; otherwise the same raw impl would be restored once
# as a missing method and again as part of the missing nominal type.
surface_owned_names = source_owned_names - missing_names


def inherent_method_names(lines: list[str], owned_names: set[str]) -> dict[str, set[str]]:
    names = {name: set() for name in owned_names}
    index = 0
    while index < len(lines):
        target = inherent_impl_target_name(lines[index])
        if target not in owned_names:
            index += 1
            continue
        end = item_end(lines, index)
        inner = index + 1
        while inner < max(index + 1, end - 1):
            if method := method_re.match(lines[inner]):
                names[target].add(method.group(1))
                inner = item_end(lines, inner)
            else:
                inner += 1
        index = end
    return names


def boxed_dyn_trait_target_name(target: str):
    """Return `Trait` for the owned carrier `Box<dyn Trait<...>>`.

    Trait-object support is emitted immediately after a source interface, but
    its impl target is the carrier rather than the source declaration itself.
    Treating ownership as a bare target-name comparison therefore lets facade
    reconciliation silently discard `Clone for Box<dyn Trait>`.
    """
    match = re.fullmatch(
        r"(?:(?:std|alloc)::boxed::)?Box\s*<\s*dyn\s+"
        r"([A-Za-z_][A-Za-z0-9_]*)(?:\s*<.*>)?\s*>",
        target,
    )
    return match.group(1) if match else None


def source_carrier_target_name(target: str):
    """Return the source nominal wrapped by a transparent shared carrier.

    Generated dispatch impls commonly target `Rc<Source>` or `Arc<Source>`;
    synchronization/interior-mutability carriers can add another transparent
    layer.  Peel only wrappers that preserve object identity so unrelated
    generic support impls do not become source-owned accidentally.
    """
    target = target.strip()
    target = target.removeprefix("&mut ").removeprefix("&").strip()
    plain = re.fullmatch(
        r"(?:(?:[A-Za-z_][A-Za-z0-9_]*)::)*([A-Za-z_][A-Za-z0-9_]*)",
        target,
    )
    if plain:
        return plain.group(1)
    wrapped = re.fullmatch(r"(.+?)\s*<\s*(.+)\s*>", target)
    if not wrapped:
        return None
    wrapper = wrapped.group(1).strip().rsplit("::", 1)[-1]
    if wrapper not in {"Rc", "Arc", "RefCell", "Cell", "Mutex", "RwLock"}:
        return impl_target_name(target)
    return source_carrier_target_name(wrapped.group(2))


def source_owned_trait_impl(
    trait: str, target: str, owned_names: set[str]
) -> bool:
    if source_carrier_target_name(target) in owned_names:
        return True
    trait_name = trait.split("<", 1)[0].strip().rsplit("::", 1)[-1]
    if (
        trait_name in owned_names
        and re.search(r"\b__Java(?:Boxed|Shared)\b", target)
    ):
        # Generic forwarding impls are part of the source interface surface
        # even though their target is a placeholder carrier rather than the
        # nominal interface (`Box<__JavaBoxed>`, `Rc<RefCell<__JavaShared>>`,
        # and the Arc/Mutex counterpart). Assembly may deduplicate the
        # prelude, but it must restore these implementations with the trait.
        return True
    return (
        trait == "Clone"
        and boxed_dyn_trait_target_name(target) in owned_names
    )


def trait_impl_keys(lines: list[str], owned_names: set[str]) -> set[tuple[str, str]]:
    keys = set()
    for line in lines:
        parts = impl_header_parts(line)
        if parts is not None and parts[0] is not None:
            trait, target = parts
            if source_owned_trait_impl(trait, target, owned_names):
                keys.add((trait, target))
    return keys


def all_trait_impl_keys(lines: list[str]) -> set[tuple[str, str]]:
    return {
        (parts[0], parts[1])
        for line in lines
        if (parts := impl_header_parts(line)) is not None and parts[0] is not None
    }


# Shared-support stripping may move or remove the structural alias that proves
# a flattened nested class belongs to this source module. Reconcile the source
# surface from the raw translator output at the end: support assembly may
# deduplicate facades, but it must never discard methods or trait impls emitted
# for a real source type. Only post-primary fallback impls are considered here;
# pre-primary inherent impls include deliberately weak facade variants.
current_lines_for_surface = current.splitlines(keepends=True)
existing_methods = inherent_method_names(current_lines_for_surface, surface_owned_names)
existing_traits = trait_impl_keys(current_lines_for_surface, surface_owned_names)
existing_all_traits = all_trait_impl_keys(current_lines_for_surface)
# The facade merge runs before this reconciliation pass and can already own a
# conversion at crate scope.  Rust coherence is global, so restoring the same
# `(trait, target)` pair in its source file would produce E0119 even though the
# implementations live in different assembly inputs.  Compare exact keys: a
# shared target by itself must not suppress a genuinely missing source
# conversion.
shared_trait_keys = set()
if shared_support_path and shared_support_path.exists():
    for line in shared_support_path.read_text().splitlines():
        parts = impl_header_parts(line)
        if parts is not None and parts[0] is not None:
            shared_trait_keys.add((parts[0], parts[1]))
source_surface_chunks = []
raw_after_primary = False
index = 0
while index < len(raw_lines):
    line = raw_lines[index]
    if match := decl_re.match(line):
        if match.group(2) == current_source:
            raw_after_primary = True
    target = inherent_impl_target_name(line) if raw_after_primary else None
    if target is not None:
        end = item_end(raw_lines, index)
        if target in surface_owned_names:
            missing_methods = []
            inner = index + 1
            while inner < max(index + 1, end - 1):
                method = method_re.match(raw_lines[inner])
                if not method:
                    inner += 1
                    continue
                method_end = item_end(raw_lines, inner)
                if method.group(1) not in existing_methods[target]:
                    missing_methods.append("".join(raw_lines[inner:method_end]))
                    existing_methods[target].add(method.group(1))
                inner = method_end
            if missing_methods:
                header = line.split("{", 1)[0].rstrip()
                source_surface_chunks.append(
                    header + " {\n" + "".join(missing_methods) + "}\n"
                )
        index = end
        continue
    parts = impl_header_parts(line) if raw_after_primary else None
    if parts is not None and parts[0] is not None:
        target = parts[1]
        key = (parts[0], target)
        end = item_end(raw_lines, index)
        if (
            source_owned_trait_impl(key[0], target, surface_owned_names)
            and key not in existing_traits
            and key not in shared_trait_keys
        ):
            source_surface_chunks.append("".join(raw_lines[index:end]))
            existing_traits.add(key)
        index = end
        continue
    index += 1

# A conversion whose input is a source-owned type is part of that source
# representation even when its target carrier lives in a sibling module.
# Facade reconciliation may remove such an impl while merging generic support;
# restore it once in the module that owns the input type.
from_source_re = re.compile(r"^From<(.+)>$")
index = 0
while index < len(raw_lines):
    parts = impl_header_parts(raw_lines[index])
    if parts is None or parts[0] is None:
        index += 1
        continue
    trait, target = parts
    source_match = from_source_re.match(trait)
    from_source_names = (
        set(re.findall(r"[A-Za-z_][A-Za-z0-9_]*", source_match.group(1)))
        if source_match
        else set()
    )
    end = item_end(raw_lines, index)
    key = (trait, target)
    if (
        source_match
        and bool(from_source_names & source_owned_names)
        and key not in existing_all_traits
        and key not in shared_trait_keys
    ):
        source_surface_chunks.append("".join(raw_lines[index:end]))
        existing_all_traits.add(key)
    index = end

# Source static storage is emitted as top-level Rust state immediately before
# the source declaration.  Shared-support stripping must restore every storage
# item still referenced by the surviving source implementation; otherwise the
# accessors compile while their backing state silently disappears.
raw_static_items = {}
for index, line in enumerate(raw_lines):
    if match := static_re.match(line):
        end = item_end(raw_lines, index)
        raw_static_items[match.group(1)] = "".join(raw_lines[index:end])
static_reference_text = current + "".join(source_surface_chunks)
existing_static_names = {
    match.group(1)
    for line in current.splitlines()
    if (match := static_re.match(line))
}
referenced_static_names = {
    name
    for name in raw_static_items
    if name not in existing_static_names
    and re.search(rf"\b{re.escape(name)}\b", static_reference_text)
}

if not missing_names and not referenced_static_names and not source_surface_chunks:
    raise SystemExit(0)

chunks = [raw_static_items[name] for name in sorted(referenced_static_names)]
chunks.extend(source_surface_chunks)
index = 0
while index < len(raw_lines):
    line = raw_lines[index]
    target = None
    start = index
    alias = None
    if match := decl_re.match(line):
        target = match.group(2)
        start = attached_start(raw_lines, index)
    elif (parts := impl_header_parts(line)) is not None:
        target = impl_target_name(parts[1])
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

# Close static dependencies against the final reconciled surface.  A method
# restored above can be the first surviving reference to its backing storage,
# so deciding from the pre-reconciliation module alone is insufficient.
final_text = rust_path.read_text()
final_static_names = {
    match.group(1)
    for line in final_text.splitlines()
    if (match := static_re.match(line))
}
missing_static_chunks = [
    raw_static_items[name].rstrip()
    for name in sorted(raw_static_items)
    if name not in final_static_names
    and re.search(rf"\b{re.escape(name)}\b", final_text)
]
if missing_static_chunks:
    final_lines = final_text.splitlines(keepends=True)
    insert_at = None
    for line_index, line in enumerate(final_lines):
        if (match := decl_re.match(line)) and match.group(2) == current_source:
            insert_at = attached_start(final_lines, line_index)
            break
    restored_statics = "\n".join(missing_static_chunks) + "\n"
    if insert_at is None:
        separator = "" if final_text.endswith("\n") or not final_text else "\n"
        rust_path.write_text(final_text + separator + restored_statics)
    else:
        final_lines[insert_at:insert_at] = [restored_statics]
        rust_path.write_text("".join(final_lines))
PY
}

if grep -R -q '^pub struct JavaException' "$generated_dir"; then
  cat >> "$shared_support_path" <<'RUST'
#[derive(Clone, Default)]
pub struct InputStream {
    bytes: Option<std::rc::Rc<Vec<u8>>>,
    position: Option<std::rc::Rc<std::cell::Cell<usize>>>,
    closed: Option<std::rc::Rc<std::cell::Cell<bool>>>,
}

impl InputStream {
    pub const fn empty() -> InputStream {
        InputStream { bytes: None, position: None, closed: None }
    }

    pub fn from_bytes(bytes: Vec<u8>) -> InputStream {
        InputStream {
            bytes: Some(std::rc::Rc::new(bytes)),
            position: Some(std::rc::Rc::new(std::cell::Cell::new(0))),
            closed: Some(std::rc::Rc::new(std::cell::Cell::new(false))),
        }
    }

    pub fn read_line(&self) -> Result<Option<String>, JavaException> {
        let Some(bytes) = self.bytes.as_ref() else {
            return Ok(None);
        };
        let position_cell = self.position.as_ref().expect("byte-backed stream position");
        let closed = self.closed.as_ref().expect("byte-backed stream closed state");
        if closed.get() {
            return Err(JavaException::new("IOException", "Stream closed".to_string()));
        }
        let mut position = position_cell.get();
        if position >= bytes.len() {
            return Ok(None);
        }
        let start = position;
        while position < bytes.len() && bytes[position] != b'\n' {
            position += 1;
        }
        let mut end = position;
        if position < bytes.len() {
            position += 1;
        }
        position_cell.set(position);
        if end > start && bytes[end - 1] == b'\r' {
            end -= 1;
        }
        Ok(Some(String::from_utf8_lossy(&bytes[start..end]).into_owned()))
    }
}

impl std::fmt::Debug for InputStream {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("InputStream").finish_non_exhaustive()
    }
}

impl PartialEq for InputStream {
    fn eq(&self, other: &Self) -> bool {
        match (&self.position, &other.position) {
            (Some(left), Some(right)) => std::rc::Rc::ptr_eq(left, right),
            (None, None) => true,
            _ => false,
        }
    }
}

impl Eq for InputStream {}

impl std::hash::Hash for InputStream {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        std::hash::Hash::hash(&self.position.as_ref().map(std::rc::Rc::as_ptr), state);
    }
}

impl std::fmt::Display for InputStream {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "java.io.InputStream@0")
    }
}

pub trait JavaIntoInputStream {
    fn into_input_stream(self) -> InputStream;
}

impl JavaIntoInputStream for InputStream {
    fn into_input_stream(self) -> InputStream {
        self
    }
}

impl JavaIntoInputStream for Option<InputStream> {
    fn into_input_stream(self) -> InputStream {
        self.expect("Java null InputStream")
    }
}

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
        let Some(bytes) = self.bytes.as_ref() else {
            return Ok(-1);
        };
        let position_cell = self.position.as_ref().expect("byte-backed stream position");
        let closed = self.closed.as_ref().expect("byte-backed stream closed state");
        if closed.get() {
            return Err(JavaException::new("IOException", "Stream closed".to_string()));
        }
        let position = position_cell.get();
        if position >= bytes.len() {
            return Ok(-1);
        }
        let value = bytes[position];
        position_cell.set(position + 1);
        Ok(i32::from(value))
    }

    pub fn read(&self, buffer: &mut [i8]) -> Result<i32, JavaException> {
        self.read_3(buffer, 0, buffer.len() as i32)
    }

    pub fn read_3(
        &self,
        buffer: &mut [i8],
        offset: i32,
        length: i32,
    ) -> Result<i32, JavaException> {
        let buffer_len = buffer.len() as i32;
        if offset < 0 || length < 0 || offset > buffer_len || length > buffer_len - offset {
            return Err(JavaException::new("IndexOutOfBoundsException", "InputStream read range out of bounds".to_string()));
        }
        let Some(bytes) = self.bytes.as_ref() else {
            return Ok(if length == 0 { 0 } else { -1 });
        };
        let position_cell = self.position.as_ref().expect("byte-backed stream position");
        let closed = self.closed.as_ref().expect("byte-backed stream closed state");
        if closed.get() {
            return Err(JavaException::new("IOException", "Stream closed".to_string()));
        }
        if length == 0 {
            return Ok(0);
        }
        let position = position_cell.get();
        if position >= bytes.len() {
            return Ok(-1);
        }
        let count = usize::min(length as usize, bytes.len() - position);
        for index in 0..count {
            buffer[offset as usize + index] = bytes[position + index] as i8;
        }
        position_cell.set(position + count);
        Ok(count as i32)
    }

    pub fn close(&self) -> Result<(), JavaException> {
        if let Some(closed) = self.closed.as_ref() {
            closed.set(true);
        }
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
#[derive(Debug, Clone)]
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
        Some(InputStream::default())
    }

    pub fn get_class_loader(&self) -> ClassLoader {
        ClassLoader
    }

    pub fn get_protection_domain(&self) -> ProtectionDomain {
        ProtectionDomain::for_runtime_class(&self.name)
    }

    pub fn equals<U>(&self, other: JavaClass<U>) -> bool {
        self.name == other.name
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
pub struct ProtectionDomain {
    code_source: Option<CodeSource>,
}

impl ProtectionDomain {
    fn for_runtime_class(_class_name: &str) -> ProtectionDomain {
        let location = std::env::current_exe()
            .ok()
            .and_then(|path| path.parent().map(std::path::Path::to_path_buf))
            .map(|path| format!("file:{}", path.to_string_lossy()))
            .unwrap_or_else(|| "file:.".to_string());
        ProtectionDomain {
            code_source: Some(CodeSource { location: URL::new(location) }),
        }
    }

    pub fn get_code_source(&self) -> Option<CodeSource> {
        self.code_source.clone()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
pub struct CodeSource {
    location: URL,
}

impl CodeSource {
    pub fn get_location(&self) -> URL {
        self.location.clone()
    }
}

RUST
  printf "JavaClass\nProtectionDomain\nCodeSource\n" >> "$shared_support_types_path"
fi

# These declarations are used by the root JavaClass facade and by the erased
# display helpers that remain attached to a generated source module.  They are
# intentionally small shared identities: concrete display implementations stay
# with their source module, while every module imports these names below.
if grep -R -q 'JavaErasedCollectionDisplayElement\|JavaErasedOptionalDisplayElement\|ClassLoader' "$generated_dir"; then
  cat >> "$shared_support_path" <<'RUST'
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
pub struct ClassLoader;

impl ClassLoader {
    pub fn get_system_resource<S: Into<String>>(path: S) -> URL {
        URL::new(path)
    }

    pub fn get_resource<S: Into<String>>(&self, path: S) -> Option<URL> {
        Some(URL::new(path))
    }

    pub fn get_resource_as_stream<S: Into<String>>(&self, _path: S) -> Option<InputStream> {
        Some(InputStream::default())
    }
}

pub trait JavaErasedCollectionDisplayElement {
    fn java_erased_collection_display(&self) -> String;
}

pub trait JavaErasedOptionalDisplayElement {
    fn java_erased_optional_display(&self) -> String;
}

macro_rules! impl_java_erased_display_primitives {
    ($trait_name:ident, $method:ident) => {
        impl $trait_name for bool { fn $method(&self) -> String { self.to_string() } }
        impl $trait_name for i8 { fn $method(&self) -> String { self.to_string() } }
        impl $trait_name for i16 { fn $method(&self) -> String { self.to_string() } }
        impl $trait_name for i32 { fn $method(&self) -> String { self.to_string() } }
        impl $trait_name for i64 { fn $method(&self) -> String { self.to_string() } }
        impl $trait_name for char { fn $method(&self) -> String { self.to_string() } }
        impl $trait_name for String { fn $method(&self) -> String { self.to_string() } }
    };
}
impl_java_erased_display_primitives!(JavaErasedCollectionDisplayElement, java_erased_collection_display);
impl_java_erased_display_primitives!(JavaErasedOptionalDisplayElement, java_erased_optional_display);

impl JavaErasedCollectionDisplayElement for f32 {
    fn java_erased_collection_display(&self) -> String { java_float_to_string(*self) }
}
impl JavaErasedCollectionDisplayElement for f64 {
    fn java_erased_collection_display(&self) -> String { java_double_to_string(*self) }
}
impl JavaErasedOptionalDisplayElement for f32 {
    fn java_erased_optional_display(&self) -> String { java_float_to_string(*self) }
}
impl JavaErasedOptionalDisplayElement for f64 {
    fn java_erased_optional_display(&self) -> String { java_double_to_string(*self) }
}
impl JavaErasedOptionalDisplayElement for Box<dyn std::any::Any> {
    fn java_erased_optional_display(&self) -> String { java_erased_any_to_string(self.as_ref()) }
}
impl<T: JavaErasedCollectionDisplayElement> JavaErasedCollectionDisplayElement for Option<T> {
    fn java_erased_collection_display(&self) -> String {
        self.as_ref().map(JavaErasedCollectionDisplayElement::java_erased_collection_display)
            .unwrap_or_else(|| "null".to_string())
    }
}
impl<T: JavaErasedCollectionDisplayElement> JavaErasedOptionalDisplayElement for Vec<T> {
    fn java_erased_optional_display(&self) -> String { java_erased_collection_to_string(self) }
}
fn java_normalize_scientific_string(rendered: String) -> String {
    let Some((mantissa, exponent)) = rendered.split_once('e') else { return rendered; };
    let mantissa = if mantissa.contains('.') { mantissa.to_string() } else { format!("{mantissa}.0") };
    let exponent = exponent.parse::<i32>().expect("Rust scientific exponent");
    format!("{mantissa}E{exponent}")
}

pub fn java_double_to_string(value: f64) -> String {
    if value.is_nan() { return "NaN".to_string(); }
    if value.is_infinite() { return if value > 0.0 { "Infinity" } else { "-Infinity" }.to_string(); }
    let magnitude = value.abs();
    if magnitude != 0.0 && !(1e-3..1e7).contains(&magnitude) {
        return java_normalize_scientific_string(format!("{value:e}"));
    }
    if value == value.trunc() && value.abs() < 1e7 { return format!("{value:.1}"); }
    value.to_string()
}

pub fn java_float_to_string(value: f32) -> String {
    if value.is_nan() { return "NaN".to_string(); }
    if value.is_infinite() { return if value > 0.0 { "Infinity" } else { "-Infinity" }.to_string(); }
    let magnitude = value.abs();
    if magnitude != 0.0 && !(1e-3f32..1e7f32).contains(&magnitude) {
        return java_normalize_scientific_string(format!("{value:e}"));
    }
    if value == value.trunc() && value.abs() < 1e7 { return format!("{value:.1}"); }
    value.to_string()
}
RUST
  printf "ClassLoader\nJavaErasedCollectionDisplayElement\nJavaErasedOptionalDisplayElement\njava_erased_collection_to_string\njava_erased_optional_to_string\njava_erased_any_to_string\njava_normalize_scientific_string\njava_float_to_string\njava_double_to_string\n" >> "$shared_support_types_path"
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

    pub fn to_u_r_i(&self) -> Result<URI, JavaException> {
        URI::parse(self.value.clone())
    }

    pub fn open_connection(&self) -> Result<URLConnection, JavaException> {
        Ok(URLConnection)
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
    pub fn entries(&self) -> Result<Enumeration<JarEntry>, JavaException> {
        Ok(Enumeration::default())
    }

    pub fn get_manifest(&self) -> Result<Manifest, JavaException> {
        Ok(Manifest)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
pub struct JarEntry {
    name: String,
}

impl JarEntry {
    pub fn new() -> JarEntry {
        JarEntry::default()
    }

    pub fn new_0() -> JarEntry {
        JarEntry::default()
    }

    pub fn new_1<S: Into<String>>(name: S) -> JarEntry {
        JarEntry { name: name.into() }
    }

    pub fn is_directory(&self) -> Result<bool, JavaException> {
        Ok(self.name.ends_with('/'))
    }

    pub fn get_name(&self) -> Result<String, JavaException> {
        Ok(self.name.clone())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
pub struct ZipEntry {
    name: String,
}

impl ZipEntry {
    pub fn new() -> ZipEntry {
        ZipEntry::default()
    }

    pub fn new_0() -> ZipEntry {
        ZipEntry::default()
    }

    pub fn new_1<S: Into<String>>(name: S) -> ZipEntry {
        ZipEntry { name: name.into() }
    }

    pub fn is_directory(&self) -> Result<bool, JavaException> {
        Ok(self.name.ends_with('/'))
    }

    pub fn get_name(&self) -> Result<String, JavaException> {
        Ok(self.name.clone())
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

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Enumeration<T = ZipEntry>(std::marker::PhantomData<T>);

impl<T> Default for Enumeration<T> {
    fn default() -> Self {
        Enumeration(std::marker::PhantomData)
    }
}

impl<T: Default> Enumeration<T> {
    pub fn has_more_elements(&self) -> Result<bool, JavaException> {
        Ok(false)
    }

    pub fn next_element(&self) -> Result<T, JavaException> {
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
        self.next_result()
            .unwrap_or_else(|ex| panic!("uncaught Java exception: {:?}", ex))
    }

    pub fn next_result(&mut self) -> Result<T, JavaException> {
        let value = self.items.get(self.index).cloned().ok_or_else(|| {
            JavaException::new(
                "NoSuchElementException",
                "Java iterator has no more elements".to_string(),
            )
        })?;
        self.index += 1;
        Ok(value)
    }

    pub fn for_each_remaining<F>(&mut self, mut f: F)
    where
        F: FnMut(T),
    {
        while self.has_next() {
            f(self.next());
        }
    }

    pub fn remove(&mut self) {
        self.remove_result()
            .unwrap_or_else(|ex| panic!("uncaught Java exception: {:?}", ex));
    }

    pub fn remove_result(&mut self) -> Result<(), JavaException> {
        Err(JavaException::new(
            "UnsupportedOperationException",
            "Java snapshot iterator mutation is not supported".to_string(),
        ))
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

    pub fn is_absolute(&self) -> bool {
        std::path::Path::new(&self.value).is_absolute()
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

    pub fn get_parent_file(&self) -> Option<File> {
        std::path::Path::new(&self.value)
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
            .map(|parent| File::new(parent.to_string_lossy().into_owned()))
    }

    pub fn get_canonical_path(&self) -> Result<String, JavaException> {
        std::fs::canonicalize(std::path::Path::new(&self.value))
            .map(|path| path.to_string_lossy().into_owned())
            .map_err(|error| JavaException::new("IOException", error.to_string()))
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

    pub fn list(&self) -> Vec<Option<String>> {
        std::fs::read_dir(std::path::Path::new(&self.value))
            .map(|entries| {
                entries
                    .filter_map(|entry| {
                        entry
                            .ok()
                            .and_then(|entry| entry.file_name().into_string().ok())
                    })
                    .map(Some)
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

impl JavaPathArg for Option<File> {
    fn into_path_value(self) -> String {
        self.map(|file| file.value).unwrap_or_default()
    }
}

impl JavaPathArg for &Option<File> {
    fn into_path_value(self) -> String {
        self.as_ref().map(|file| file.value.clone()).unwrap_or_default()
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

if grep -R -q 'java_is_serializable' "$generated_dir"; then
  cat >> "$shared_support_path" <<'RUST'
pub fn java_is_serializable<T: ?Sized>(_value: &T) -> bool {
    true
}

RUST
  printf "java_is_serializable\n" >> "$shared_support_types_path"
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

# The closure and cleanup passes below operate on one generated module at a
# time.  Large translated modules make their Rust item scans noticeable, but
# there is no cross-module mutation within a stage.  Reuse the translation
# job limit here instead of serially rescanning every module.
assembly_stage_begin() {
  assembly_stage_active_jobs=0
  assembly_stage_failed=0
}

assembly_stage_start_job() {
  "$@" &
  assembly_stage_active_jobs="$((assembly_stage_active_jobs + 1))"
  if [[ "$assembly_stage_active_jobs" -ge "$jobs" ]]; then
    if ! wait -n; then
      assembly_stage_failed=1
    fi
    assembly_stage_active_jobs="$((assembly_stage_active_jobs - 1))"
  fi
}

assembly_stage_finish() {
  while [[ "$assembly_stage_active_jobs" -gt 0 ]]; do
    if ! wait -n; then
      assembly_stage_failed=1
    fi
    assembly_stage_active_jobs="$((assembly_stage_active_jobs - 1))"
  done
  if [[ "$assembly_stage_failed" -ne 0 ]]; then
    echo "generated module assembly stage failed" >&2
    return 1
  fi
}

strip_and_restore_generated_module() {
  local rust_path="$1"
  strip_shared_support_items "$rust_path" "$shared_support_types_path" &&
    strip_source_type_facade_stubs "$rust_path" "$source_types_path" "$repo_root/scripts" &&
    restore_missing_prelude_support_items "$rust_path" "$raw_generated_backup_dir/$(basename "$rust_path")" "$source_types_path" "$shared_support_types_path" "$repo_root/scripts"
}

restore_generated_module_dependencies() {
  local rust_path="$1"
  restore_missing_source_type_items "$rust_path" "$raw_generated_backup_dir/$(basename "$rust_path")" "$shared_support_types_path" "$shared_support_path" "$repo_root/scripts" &&
    restore_missing_prelude_support_items "$rust_path" "$raw_generated_backup_dir/$(basename "$rust_path")" "$source_types_path" "$shared_support_types_path" "$repo_root/scripts"
}

finalize_generated_module_dependencies() {
  local rust_path="$1"
  restore_generated_module_dependencies "$rust_path" &&
    deduplicate_generated_coherence_items "$rust_path" "$repo_root/scripts" "$shared_support_path" &&
    python3 "$repo_root/scripts/merge-shared-support-facades.py" --cleanup-module "$rust_path"
}

if [[ -s "$shared_support_path" ]]; then
  assembly_stage_begin
  for rust_path in "$generated_dir"/*.rs; do
    assembly_stage_start_job strip_and_restore_generated_module "$rust_path"
  done
  assembly_stage_finish || exit 1
else
  touch "$shared_support_path" "$shared_support_types_path"
fi

python3 "$repo_root/scripts/merge-shared-support-facades.py" \
  --generated-dir "$generated_dir" \
  --merge-input-dir "$raw_generated_backup_dir" \
  --shared-support "$shared_support_path" \
  --shared-types "$shared_support_types_path" \
  --conflicts "$shared_support_conflicts_path"

# Facade merging can promote a helper function without promoting a nominal
# type.  Modules still need an explicit crate import for such helpers.
awk '
  /^pub[[:space:]]+fn[[:space:]]+[A-Za-z_][A-Za-z0-9_]*/ {
    name = $3
    sub(/<.*/, "", name)
    sub(/\(.*/, "", name)
    print name
  }
' "$shared_support_path" >> "$shared_support_types_path"

# A blanket string helper has no nominal implementation target, so the
# facade merger cannot associate it with a selected struct.  It is nevertheless
# crate-global Rust coherence, and must be emitted once.
if grep -q '^pub trait JavaStringComparisonHelper' "$shared_support_path" &&
   ! grep -q '^impl<T: AsRef<str> + ?Sized> JavaStringComparisonHelper for T {' "$shared_support_path"; then
  cat >> "$shared_support_path" <<'RUST'
impl<T: AsRef<str> + ?Sized> JavaStringComparisonHelper for T {
    fn java_string_compare_to<R: AsRef<str>>(&self, other: R) -> i32 {
        java_string_compare_to(self.as_ref(), other)
    }

    fn java_string_compare_to_ignore_case<R: AsRef<str>>(&self, other: R) -> i32 {
        java_string_compare_to_ignore_case(self.as_ref(), other)
    }

    fn java_string_equals_ignore_case<R: AsRef<str>>(&self, other: R) -> bool {
        java_string_equals_ignore_case(self.as_ref(), other)
    }
}
RUST
fi

# Natural-order queue construction calls the marker trait.  The facade merger
# combines inherent impl blocks, so restore the required bound on that merged
# block instead of emitting an unconstrained method body.
python3 - "$shared_support_path" <<'PY'
import sys
from pathlib import Path

path = Path(sys.argv[1])
text = path.read_text()
text = text.replace(
    "impl<T> JavaPriorityQueue<T> {",
    "impl<T: JavaPriorityQueueNaturalOrder + 'static> JavaPriorityQueue<T> {",
)
path.write_text(text)
PY

# The private erased-display traits have one crate-wide identity.  Their two
# generic implementations are therefore emitted once above, rather than once
# per translated Java source module.
python3 - "$generated_dir" <<'PY'
import re
import sys
from pathlib import Path

patterns = (
    re.compile(r"^impl<T: JavaErasedCollectionDisplayElement> JavaErasedCollectionDisplayElement for Option<T> \{"),
    re.compile(r"^impl<T: JavaErasedCollectionDisplayElement> JavaErasedOptionalDisplayElement for Vec<T> \{"),
    re.compile(r"^impl<T: AsRef<str> \+ \?Sized> JavaStringComparisonHelper for T \{"),
)
for path in Path(sys.argv[1]).glob("*.rs"):
    lines = path.read_text().splitlines(keepends=True)
    kept = []
    index = 0
    while index < len(lines):
        if any(pattern.match(lines[index]) for pattern in patterns):
            depth = 0
            while index < len(lines):
                depth += lines[index].count("{") - lines[index].count("}")
                index += 1
                if depth == 0:
                    break
            continue
        kept.append(lines[index])
        index += 1
    path.write_text("".join(kept))
PY

if grep -R -q 'ndarray::' "$generated_dir" "$shared_support_path"; then
  if ! grep -q '^ndarray[[:space:]]*=' "$crate_dir/Cargo.toml"; then
    write_ndarray_shim "$crate_dir/ndarray-shim"
    printf 'ndarray = { path = "ndarray-shim" }\n' >> "$crate_dir/Cargo.toml"
  fi
fi

# Run dependency restoration after every assembler rewrite. In particular,
# source-owned `static` storage is not a nominal facade item and a late source
# surface reconciliation may remove it while leaving its accessors intact.
# The restoration helper only copies raw statics still referenced by the final
# module, so support-prelude state remains excluded.
assembly_stage_begin
for rust_path in "$generated_dir"/*.rs; do
  assembly_stage_start_job restore_generated_module_dependencies "$rust_path"
done
assembly_stage_finish || exit 1

# Dependency-bearing expressions can be restored from the raw module by the
# closure pass above. Detect dependencies against the final generated surface,
# not only the pre-restoration modules.
if grep -R -q 'ndarray::' "$generated_dir" "$shared_support_path"; then
  if ! grep -q '^ndarray[[:space:]]*=' "$crate_dir/Cargo.toml"; then
    write_ndarray_shim "$crate_dir/ndarray-shim"
    printf 'ndarray = { path = "ndarray-shim" }\n' >> "$crate_dir/Cargo.toml"
  fi
fi

for i in "${!assembly_java_files[@]}"; do
  rust_path="${assembly_rust_paths[$i]}"
  stem="$(basename "${rust_path%.rs}")"
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

for i in "${!assembly_java_files[@]}"; do
  rust_path="${assembly_rust_paths[$i]}"
  stem="$(basename "${rust_path%.rs}")"
  module="generated_$stem"
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

# Import materialization above is the last rewrite of each generated source
# module.  Re-run the idempotent source-surface closure at that actual module
# boundary.  The earlier pass must remain before type indexing because it can
# restore nominal declarations; this final pass protects non-nominal companion
# items such as trait-object carrier impls from every assembly ordering.
assembly_stage_begin
for rust_path in "$generated_dir"/*.rs; do
  assembly_stage_start_job finalize_generated_module_dependencies "$rust_path"
  # Restored companion items can introduce dependencies on prelude helpers
  # after the main facade merge has already computed its closure. Re-run the
  # local support closure at the final module surface so those helpers remain
  # beside their source-owned companion instead of being dropped.
done
assembly_stage_finish || exit 1

# Batch translation records marker counts before crate assembly strips and
# centralizes shared support. Those raw counts describe the intermediate
# transport format, not the Rust modules users compile. Refresh only the
# marker column from the final module surface while preserving per-source
# diagnostics and status.
final_status_path="$status_path.final"
printf "source\trust_path\ttranslate_status\tdiagnostics\tinvalid_patterns\tunsupported_members\tstubbed_constants\n" > "$final_status_path"
while IFS=$'\t' read -r source rust_path translate_status diagnostics _invalid_patterns unsupported_members stubbed_constants; do
  if ! final_invalid_patterns="$("$binary" count-invalid-patterns "$rust_path" 2>/dev/null)" \
      || [[ ! "$final_invalid_patterns" =~ ^[0-9]+$ ]]; then
    # Test doubles and older externally supplied translators may not expose
    # the counter subcommand. Preserve their reported value rather than
    # turning a reporting refresh into an assembly failure.
    final_invalid_patterns="$_invalid_patterns"
  fi
  printf "%s\t%s\t%s\t%s\t%s\t%s\t%s\n" \
    "$source" "$rust_path" "$translate_status" "$diagnostics" \
    "$final_invalid_patterns" "$unsupported_members" "${stubbed_constants:-0}" \
    >> "$final_status_path"
done < <(tail -n +2 "$status_path")
mv "$final_status_path" "$status_path"

{
  printf "#![allow(dead_code, unused_imports, unused_variables, non_snake_case, non_camel_case_types, non_upper_case_globals)]\n"
  if [[ -s "$shared_support_path" ]]; then
    cat "$shared_support_path"
  fi
  for i in "${!assembly_java_files[@]}"; do
    stem="$(basename "${assembly_rust_paths[$i]%.rs}")"
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

total_files="${#assembly_java_files[@]}"
translated_failed="$(awk -F '\t' 'NR > 1 && $3 == "errors" { count++ } END { print count + 0 }' "$status_path")"
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
invalid_pattern_count="$(status_column_sum "$status_path" "invalid_patterns")" || exit 1
{
  printf "pattern\tcount\tfiles\n"
  printf "registry_invalid_patterns\t%s\t%s\n" "$invalid_pattern_count" "$invalid_pattern_files"
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
  --extra-rust "$shared_support_path" \
  --status "$status_path" \
  --out-tsv "$idiom_sweep_tsv" \
  --out-md "$idiom_sweep_md"
idiom_sweep_elapsed_ms="$(elapsed_ms_since "$idiom_sweep_start_ms")"
printf "idiom_sweep\tok\t%s\t%s\n" "$idiom_sweep_elapsed_ms" "$idiom_sweep_md" >> "$phase_events_path"
unsupported_statement_count="$(idiom_signal_count "$idiom_sweep_tsv" "unsupported_statements")" || exit 1
unsupported_member_idiom_count="$(idiom_signal_count "$idiom_sweep_tsv" "unsupported_members")" || exit 1
unsupported_member_status_count="$(status_column_sum "$status_path" "unsupported_members")" || exit 1
unsupported_member_count="$(( unsupported_member_idiom_count + unsupported_member_status_count ))"
unsupported_expression_count="$(idiom_signal_count "$idiom_sweep_tsv" "unsupported_expressions")" || exit 1
unmodelled_type_count="$(idiom_signal_count "$idiom_sweep_tsv" "unmodelled_types")" || exit 1
unmodelled_call_site_count="$(idiom_signal_count "$idiom_sweep_tsv" "unmodelled_call_sites")" || exit 1
noop_facade_method_count="$(idiom_signal_count "$idiom_sweep_tsv" "noop_facade_methods")" || exit 1
noop_facade_call_site_count="$(analysis_profile_metric_or_idiom_signal "$analysis_profile_path" "$idiom_sweep_tsv" "noop_facade_call_sites")" || exit 1
unmodelled_upcast_count="$(idiom_signal_count "$idiom_sweep_tsv" "unmodelled_upcast_conversions")" || exit 1
stubbed_constant_count="$(idiom_signal_count "$idiom_sweep_tsv" "stubbed_constants")" || exit 1
total_elapsed_ms="$(elapsed_ms_since "$script_start_ms")"
printf "total\tok\t%s\t%s\n" "$total_elapsed_ms" "$out_dir" >> "$phase_events_path"

{
  printf "elapsed_ms\tphase\tstatus\tdetail\n"
  awk -F '\t' 'NR > 1 { gsub(/\t/, " ", $4); printf "%s\t%s\t%s\t%s\n", $3, $1, $2, $4 }' "$phase_events_path" \
    | sort -nr \
    | awk 'NR <= 20'
} > "$slowest_phases_path"

{
  printf "elapsed_ms\tevent\tstatus\tindex\tsource\trust_path\n"
  awk -F '\t' 'NR > 1 && $5 > 0 { gsub(/\t/, " ", $4); gsub(/\t/, " ", $7); printf "%s\t%s\t%s\t%s\t%s\t%s\n", $5, $1, $6, $3, $4, $7 }' "$worker_events_path" \
    | sort -nr \
    | awk 'NR <= 20'
} > "$slowest_workers_path"

{
  printf "primary\t%s\n" "$primary"
  printf "out_dir\t%s\n" "$out_dir"
  printf "generated_crate\t%s\n" "$crate_dir"
  printf "translator_binary\t%s\n" "$binary"
  printf "translator_build_elapsed_ms\t%s\n" "$translator_build_elapsed_ms"
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
  printf "noop_facade_call_sites\t%s\n" "$noop_facade_call_site_count"
  printf "unmodelled_upcast_conversions\t%s\n" "$unmodelled_upcast_count"
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
