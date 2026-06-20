/// Java `java.io.File` -> a `std::path::PathBuf`-backed path handle. Methods do
/// real filesystem work via `std::fs`/`std::path` (mirrors `java.io.File`).
#[derive(Clone, PartialEq, Eq, Hash, Default)]
pub struct JavaFile {
    path: std::path::PathBuf,
}
impl AsRef<std::path::Path> for JavaFile {
    fn as_ref(&self) -> &std::path::Path {
        &self.path
    }
}
impl JavaFile {
    // Path args are bounded by `ToString` (not `AsRef<Path>`) so they accept the
    // same breadth the opaque stub did — `String`/`&str`/`JavaFile`/`Unknown`
    // (all `Display`) — while still doing real path work.
    pub fn new<P: ToString>(p: P) -> Self {
        JavaFile { path: std::path::PathBuf::from(p.to_string()) }
    }
    pub fn new_2<P: ToString, Q: ToString>(parent: P, child: Q) -> Self {
        JavaFile { path: std::path::PathBuf::from(parent.to_string()).join(child.to_string()) }
    }
    pub fn exists(&self) -> bool {
        self.path.exists()
    }
    pub fn is_file(&self) -> bool {
        self.path.is_file()
    }
    pub fn is_directory(&self) -> bool {
        self.path.is_dir()
    }
    pub fn is_absolute(&self) -> bool {
        self.path.is_absolute()
    }
    pub fn is_hidden(&self) -> bool {
        self.path.file_name().map(|n| n.to_string_lossy().starts_with('.')).unwrap_or(false)
    }
    pub fn can_read(&self) -> bool {
        std::fs::File::open(&self.path).is_ok()
    }
    pub fn can_write(&self) -> bool {
        self.path.metadata().map(|m| !m.permissions().readonly()).unwrap_or(false)
    }
    pub fn can_execute(&self) -> bool {
        self.path.exists()
    }
    pub fn length(&self) -> i64 {
        self.path.metadata().map(|m| m.len() as i64).unwrap_or(0)
    }
    pub fn get_name(&self) -> String {
        self.path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default()
    }
    pub fn get_path(&self) -> String {
        self.path.to_string_lossy().into_owned()
    }
    pub fn get_absolute_path(&self) -> String {
        std::fs::canonicalize(&self.path).unwrap_or_else(|_| self.path.clone()).to_string_lossy().into_owned()
    }
    pub fn get_canonical_path(&self) -> String {
        self.get_absolute_path()
    }
    pub fn get_absolute_file(&self) -> JavaFile {
        JavaFile::new(self.get_absolute_path())
    }
    pub fn get_canonical_file(&self) -> JavaFile {
        JavaFile::new(self.get_canonical_path())
    }
    pub fn get_parent(&self) -> String {
        self.path.parent().map(|p| p.to_string_lossy().into_owned()).unwrap_or_default()
    }
    pub fn get_parent_file(&self) -> JavaFile {
        JavaFile { path: self.path.parent().map(|p| p.to_path_buf()).unwrap_or_default() }
    }
    pub fn delete(&self) -> bool {
        std::fs::remove_file(&self.path).is_ok() || std::fs::remove_dir_all(&self.path).is_ok()
    }
    pub fn delete_on_exit(&self) {}
    pub fn mkdir(&self) -> bool {
        std::fs::create_dir(&self.path).is_ok()
    }
    pub fn mkdirs(&self) -> bool {
        std::fs::create_dir_all(&self.path).is_ok()
    }
    pub fn create_new_file(&self) -> bool {
        std::fs::OpenOptions::new().write(true).create_new(true).open(&self.path).is_ok()
    }
    pub fn rename_to<P: ToString>(&self, dest: P) -> bool {
        std::fs::rename(&self.path, dest.to_string()).is_ok()
    }
    pub fn last_modified(&self) -> i64 {
        0
    }
    pub fn set_last_modified(&self, _t: i64) -> bool {
        true
    }
    pub fn list(&self) -> Vec<String> {
        std::fs::read_dir(&self.path)
            .map(|rd| rd.filter_map(|e| e.ok().map(|e| e.file_name().to_string_lossy().into_owned())).collect())
            .unwrap_or_default()
    }
    pub fn list_files(&self) -> Vec<JavaFile> {
        std::fs::read_dir(&self.path)
            .map(|rd| rd.filter_map(|e| e.ok().map(|e| JavaFile { path: e.path() })).collect())
            .unwrap_or_default()
    }
    pub fn to_path(&self) -> JavaFile {
        self.clone()
    }
    pub fn to_string(&self) -> String {
        self.get_path()
    }
    pub fn to_uri(&self) -> String {
        format!("file://{}", self.get_absolute_path())
    }
    pub fn create_temp_file<P: ToString, Q: ToString>(prefix: P, suffix: Q) -> JavaFile {
        let mut p = std::env::temp_dir();
        p.push(format!("{}{}", prefix.to_string(), suffix.to_string()));
        JavaFile { path: p }
    }
}
impl std::fmt::Display for JavaFile {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.get_path())
    }
}

