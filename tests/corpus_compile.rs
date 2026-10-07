mod common;

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use common::{load_java_fixtures, run_with_watchdog};
use new_java2rust::translate_source;

const EXPECTED_FAILURES: &str = include_str!("baselines/corpus-compile-failures.txt");

#[test]
fn checked_in_example_corpora_have_named_compile_failures() {
    assert_tool_on_path("rustc");

    let baseline = parse_baseline(EXPECTED_FAILURES);
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let temp_dir = std::env::temp_dir().join(format!("new_java2rust_corpus_compile_{unique}"));
    fs::create_dir_all(&temp_dir).unwrap();

    let fixtures = Arc::new(load_corpus_fixtures());
    let jobs = corpus_compile_jobs().min(fixtures.len().max(1));
    let next = Arc::new(AtomicUsize::new(0));
    let (tx, rx) = mpsc::channel();

    std::thread::scope(|scope| {
        for _ in 0..jobs {
            let fixtures = Arc::clone(&fixtures);
            let next = Arc::clone(&next);
            let temp_dir = temp_dir.clone();
            let tx = tx.clone();
            scope.spawn(move || {
                loop {
                    let index = next.fetch_add(1, Ordering::Relaxed);
                    if index >= fixtures.len() {
                        break;
                    }
                    let fixture = &fixtures[index];
                    tx.send(compile_corpus_fixture(fixture, &temp_dir)).unwrap();
                }
            });
        }
    });
    drop(tx);

    let mut actual = BTreeSet::new();
    let mut details = BTreeMap::new();
    for (display_path, maybe_error) in rx {
        if let Some(error) = maybe_error {
            actual.insert(display_path.clone());
            details.insert(display_path, error);
        }
    }

    if actual != baseline {
        keep_rust_sources_drop_metadata(&temp_dir);
        panic!(
            "corpus compile failure baseline changed; generated Rust kept at {}\n{}",
            temp_dir.display(),
            format_baseline_delta(&baseline, &actual, &details)
        );
    }

    let _ = fs::remove_dir_all(&temp_dir);
}

struct CorpusFixture {
    display_path: String,
    source: String,
}

fn load_corpus_fixtures() -> Vec<CorpusFixture> {
    let mut fixtures = Vec::new();
    for corpus_root in ["examples/java", "examples/idiom-gaps"] {
        for fixture in load_java_fixtures(corpus_root).unwrap() {
            let display_path = format!(
                "{}/{}",
                corpus_root.strip_prefix("examples/").unwrap(),
                fixture.relative_path.display()
            );
            fixtures.push(CorpusFixture {
                display_path,
                source: fixture.source,
            });
        }
    }
    fixtures.sort_by(|left, right| left.display_path.cmp(&right.display_path));
    fixtures
}

fn compile_corpus_fixture(fixture: &CorpusFixture, temp_dir: &Path) -> (String, Option<String>) {
    let translation = translate_source(&fixture.source);
    let rust_path = temp_dir.join(format!("{}.rs", sanitize_path(&fixture.display_path)));
    let meta_path = rust_path.with_extension("rmeta");
    fs::write(&rust_path, corpus_rust_source(&translation.rust)).unwrap();

    let mut command = Command::new("rustc");
    command
        .arg("--edition")
        .arg("2021")
        .arg("--crate-type")
        .arg("lib")
        .arg("--emit=metadata")
        .arg("--crate-name")
        .arg("corpus_fixture")
        .arg(&rust_path)
        .arg("-o")
        .arg(&meta_path);
    let output = run_with_watchdog(&mut command, Duration::from_secs(30));
    let error = output
        .status
        .map_or(true, |status| !status.success())
        .then(|| {
            format!(
                "{}{}",
                if output.status.is_none() {
                    "rustc timed out after 30s\n"
                } else {
                    ""
                },
                first_diagnostic(&output.stderr)
            )
        });
    (fixture.display_path.clone(), error)
}

fn corpus_rust_source(rust: &str) -> String {
    if rust.contains("ndarray::") {
        format!("{}\n{rust}", corpus_ndarray_shim())
    } else {
        rust.to_string()
    }
}

fn corpus_ndarray_shim() -> &'static str {
    r#"
mod ndarray {
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
}
"#
}

fn corpus_compile_jobs() -> usize {
    std::env::var("NEW_JAVA2RUST_CORPUS_COMPILE_JOBS")
        .ok()
        .and_then(|raw| raw.parse::<usize>().ok())
        .filter(|jobs| *jobs > 0)
        .unwrap_or_else(|| std::thread::available_parallelism().map_or(4, |jobs| jobs.get().min(4)))
}

fn parse_baseline(raw: &str) -> BTreeSet<String> {
    raw.lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .map(str::to_owned)
        .collect()
}

fn sanitize_path(path: &str) -> String {
    path.chars()
        .map(|ch| if ch.is_ascii_alphanumeric() { ch } else { '_' })
        .collect()
}

fn first_diagnostic(stderr: &str) -> String {
    stderr.lines().take(12).collect::<Vec<_>>().join("\n")
}

fn format_baseline_delta(
    baseline: &BTreeSet<String>,
    actual: &BTreeSet<String>,
    details: &BTreeMap<String, String>,
) -> String {
    let mut message = String::new();
    let added: Vec<_> = actual.difference(baseline).collect();
    let removed: Vec<_> = baseline.difference(actual).collect();

    if !added.is_empty() {
        message.push_str("new compile failures:\n");
        for path in added {
            message.push_str(&format!("- {path}\n{}\n", details[path]));
        }
    }
    if !removed.is_empty() {
        message.push_str("compile failures fixed; remove these baseline entries:\n");
        for path in removed {
            message.push_str(&format!("- {path}\n"));
        }
    }
    message
}

fn keep_rust_sources_drop_metadata(temp_dir: &Path) {
    if let Ok(entries) = fs::read_dir(temp_dir) {
        for entry in entries.flatten() {
            let path: PathBuf = entry.path();
            if path.extension().and_then(|ext| ext.to_str()) == Some("rmeta") {
                let _ = fs::remove_file(path);
            }
        }
    }
}

fn assert_tool_on_path(tool: &str) {
    let output = Command::new(tool)
        .arg("--version")
        .output()
        .unwrap_or_else(|err| {
            panic!("{tool} is required for corpus compile checks but could not be run: {err}")
        });
    assert!(
        output.status.success(),
        "{tool} --version failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}
