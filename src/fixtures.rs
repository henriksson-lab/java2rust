use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Fixture {
    pub relative_path: PathBuf,
    pub source: String,
}

pub fn load_java_fixtures(root: impl AsRef<Path>) -> std::io::Result<Vec<Fixture>> {
    let root = root.as_ref();
    let mut files = Vec::new();
    collect_java_files(root, root, &mut files)?;
    files.sort_by(|a, b| a.relative_path.cmp(&b.relative_path));
    Ok(files)
}

fn collect_java_files(root: &Path, dir: &Path, out: &mut Vec<Fixture>) -> std::io::Result<()> {
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            collect_java_files(root, &path, out)?;
        } else if path.extension().and_then(|e| e.to_str()) == Some("java") {
            out.push(Fixture {
                relative_path: path.strip_prefix(root).unwrap_or(&path).to_path_buf(),
                source: fs::read_to_string(&path)?,
            });
        }
    }
    Ok(())
}
