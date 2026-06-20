// Java output/writer I/O stack -> real Rust runtime types. Every concrete type
// is NON-generic (annotations carry no generics) and uses interior mutability
// so its methods take `&self` — a Java field / `static final` of one of these
// types is reached as `&Self`, so a writing/advancing method must not need
// `&mut self` (that fails E0596 at field/static call sites).
//
// State lives behind `Rc<RefCell<…>>`: that gives interior mutability AND lets
// the whole type `#[derive(Clone)]` (the translator clones fields freely, the
// way the opaque stub it replaces was `Clone`). A clone shares the same sink,
// which is the closest analogue of Java's reference semantics. To match the
// opaque stub's full derive set (`Clone, Default, PartialEq, Eq, Hash,
// PartialOrd, Ord`), the comparison/hash traits are implemented trivially
// (every instance compares equal / hashes the same) — a writer is never a
// meaningful map key or sort element, so this only needs to type-check.
//
// Composition (`new PrintStream(new FileOutputStream(f))`,
// `new BufferedWriter(new OutputStreamWriter(new FileOutputStream(f)))`) type-
// checks because every concrete type impls `std::io::Write`, and the wrapping
// factory fns/ctors take `impl std::io::Write + 'static` and box it.
//
// I/O errors are swallowed (best-effort): writes/flushes never panic; a sticky
// error flag is exposed via `check_error()` where Java has it.

use std::io::Write as _IoWrite;

/// Anything that can become an owned, `'static` boxed sink. Implemented for any
/// `W: Write + 'static` (boxed directly) AND for `&Carrier` (cloned — the Rc
/// state is cheap to share and the clone IS `'static`). This lets the wrapping
/// ctors/factory fns accept BOTH a freshly-moved sink and a BORROWED carrier the
/// translator hands them (a field/param receiver is reached as `&Carrier`), so
/// `new BufferedWriter(new OutputStreamWriter(out))` AND `new
/// BufferedWriter(osw)` (osw a borrowed local) both compose.
pub trait IntoBoxedWrite {
    fn into_boxed_write(self) -> Box<dyn std::io::Write>;
}
impl<W: std::io::Write + 'static> IntoBoxedWrite for W {
    fn into_boxed_write(self) -> Box<dyn std::io::Write> {
        Box::new(self)
    }
}
impl IntoBoxedWrite for &JavaOutputStream {
    fn into_boxed_write(self) -> Box<dyn std::io::Write> {
        Box::new(self.clone())
    }
}
impl IntoBoxedWrite for &JavaWriter {
    fn into_boxed_write(self) -> Box<dyn std::io::Write> {
        Box::new(self.clone())
    }
}
impl IntoBoxedWrite for &JavaByteArrayOutputStream {
    fn into_boxed_write(self) -> Box<dyn std::io::Write> {
        Box::new(self.clone())
    }
}
impl IntoBoxedWrite for &JavaStringWriter {
    fn into_boxed_write(self) -> Box<dyn std::io::Write> {
        Box::new(self.clone())
    }
}
// `&&Carrier` too: a borrowed-carrier field/param is itself reached as a
// reference, so a composition site nests another `&` on top.
impl IntoBoxedWrite for &&JavaOutputStream {
    fn into_boxed_write(self) -> Box<dyn std::io::Write> {
        Box::new((*self).clone())
    }
}
impl IntoBoxedWrite for &&JavaWriter {
    fn into_boxed_write(self) -> Box<dyn std::io::Write> {
        Box::new((*self).clone())
    }
}

/// Trivial equality/ordering/hash so a runtime writer type can derive the same
/// trait set the opaque stub did (it is never a meaningful key/sort element).
macro_rules! io_write_trivial_traits {
    ($t:ty) => {
        impl PartialEq for $t {
            fn eq(&self, _other: &Self) -> bool {
                true
            }
        }
        impl Eq for $t {}
        impl PartialOrd for $t {
            fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
                Some(self.cmp(other))
            }
        }
        impl Ord for $t {
            fn cmp(&self, _other: &Self) -> std::cmp::Ordering {
                std::cmp::Ordering::Equal
            }
        }
        impl std::hash::Hash for $t {
            fn hash<H: std::hash::Hasher>(&self, _state: &mut H) {}
        }
    };
}

// ===========================================================================
// Byte streams (impl std::io::Write)
// ===========================================================================

/// Carrier for `OutputStream`/`FileOutputStream`/`BufferedOutputStream`/… — an
/// abstract supertype handle over a boxed `std::io::Write`. Concrete factory
/// fns (`java_file_output_stream`, …) build the right inner and wrap it here so
/// abstract-typed fields/locals all share one Rust type.
#[derive(Clone)]
pub struct JavaOutputStream {
    inner: Rc<RefCell<Box<dyn std::io::Write>>>,
    err: Rc<RefCell<bool>>,
}
impl Default for JavaOutputStream {
    fn default() -> Self {
        JavaOutputStream::new_1(std::io::sink())
    }
}
io_write_trivial_traits!(JavaOutputStream);
impl JavaOutputStream {
    pub fn new_1<W: IntoBoxedWrite>(inner: W) -> Self {
        JavaOutputStream {
            inner: Rc::new(RefCell::new(inner.into_boxed_write())),
            err: Rc::new(RefCell::new(false)),
        }
    }
    fn note<T>(&self, r: std::io::Result<T>) {
        if r.is_err() {
            *self.err.borrow_mut() = true;
        }
    }
    /// `write(byte[] b, int off, int len)`.
    pub fn write_3(&self, buf: &[i8], off: i32, len: i32) {
        let off = off.max(0) as usize;
        let len = len.max(0) as usize;
        let start = off.min(buf.len());
        let end = (off + len).min(buf.len());
        let bytes: Vec<u8> = buf[start..end].iter().map(|&b| b as u8).collect();
        let r = self.inner.borrow_mut().write_all(&bytes);
        self.note(r);
    }
    /// `write(byte[] b)`.
    pub fn write_1(&self, buf: &[i8]) {
        let bytes: Vec<u8> = buf.iter().map(|&b| b as u8).collect();
        let r = self.inner.borrow_mut().write_all(&bytes);
        self.note(r);
    }
    /// `write(int b)` — writes the low byte.
    pub fn write(&self, b: i32) {
        let r = self.inner.borrow_mut().write_all(&[b as u8]);
        self.note(r);
    }
    pub fn flush(&self) {
        let r = self.inner.borrow_mut().flush();
        self.note(r);
    }
    pub fn close(&self) {
        self.flush();
    }
    pub fn check_error(&self) -> bool {
        *self.err.borrow()
    }
}
impl std::io::Write for JavaOutputStream {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.inner.borrow_mut().write(buf)
    }
    fn flush(&mut self) -> std::io::Result<()> {
        self.inner.borrow_mut().flush()
    }
}
// `OutputStream`/`Writer` carriers occasionally land in a `format!`/`toString`
// position (Java's identity `toString`); a trivial `Display` keeps that compiling.
impl std::fmt::Display for JavaOutputStream {
    fn fmt(&self, _f: &mut std::fmt::Formatter) -> std::fmt::Result { Ok(()) }
}
impl std::fmt::Display for JavaWriter {
    fn fmt(&self, _f: &mut std::fmt::Formatter) -> std::fmt::Result { Ok(()) }
}
/// `java.io.FileOutputStream` — opens a file for writing. Used directly as the
/// inner sink of the carriers (it is not itself a mapped Java type — its name
/// maps to `JavaOutputStream` and the ctor lowers to a factory fn).
#[derive(Clone)]
pub struct JavaFileOutputStream {
    inner: Rc<RefCell<Option<std::fs::File>>>,
    err: Rc<RefCell<bool>>,
}
impl JavaFileOutputStream {
    pub fn new_1<P: ToString>(path: P) -> Self {
        let f = std::fs::File::create(path.to_string()).ok();
        JavaFileOutputStream { inner: Rc::new(RefCell::new(f)), err: Rc::new(RefCell::new(false)) }
    }
    pub fn new_2<P: ToString>(path: P, append: bool) -> Self {
        let f = std::fs::OpenOptions::new()
            .write(true)
            .create(true)
            .append(append)
            .truncate(!append)
            .open(path.to_string())
            .ok();
        JavaFileOutputStream { inner: Rc::new(RefCell::new(f)), err: Rc::new(RefCell::new(false)) }
    }
}
impl std::io::Write for JavaFileOutputStream {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        match self.inner.borrow_mut().as_mut() {
            Some(f) => f.write(buf),
            None => {
                *self.err.borrow_mut() = true;
                Ok(buf.len())
            }
        }
    }
    fn flush(&mut self) -> std::io::Result<()> {
        match self.inner.borrow_mut().as_mut() {
            Some(f) => f.flush(),
            None => Ok(()),
        }
    }
}

/// `java.io.ByteArrayOutputStream` — an in-memory growable byte buffer. Kept as
/// its OWN mapped type (not the carrier) because `to_byte_array`/`to_string` are
/// not on the abstract carrier.
#[derive(Clone)]
pub struct JavaByteArrayOutputStream {
    buf: Rc<RefCell<Vec<u8>>>,
}
impl Default for JavaByteArrayOutputStream {
    fn default() -> Self {
        Self::new()
    }
}
io_write_trivial_traits!(JavaByteArrayOutputStream);
impl JavaByteArrayOutputStream {
    pub fn new() -> Self {
        JavaByteArrayOutputStream { buf: Rc::new(RefCell::new(Vec::new())) }
    }
    pub fn new_1(_size: i32) -> Self {
        Self::new()
    }
    pub fn write_3(&self, b: &[i8], off: i32, len: i32) {
        let off = off.max(0) as usize;
        let len = len.max(0) as usize;
        let start = off.min(b.len());
        let end = (off + len).min(b.len());
        let mut buf = self.buf.borrow_mut();
        for &x in &b[start..end] {
            buf.push(x as u8);
        }
    }
    pub fn write_1(&self, b: &[i8]) {
        let mut buf = self.buf.borrow_mut();
        for &x in b {
            buf.push(x as u8);
        }
    }
    pub fn write(&self, b: i32) {
        self.buf.borrow_mut().push(b as u8);
    }
    pub fn to_byte_array(&self) -> Vec<i8> {
        self.buf.borrow().iter().map(|&b| b as i8).collect()
    }
    pub fn to_string(&self) -> String {
        String::from_utf8_lossy(&self.buf.borrow()).into_owned()
    }
    pub fn size(&self) -> i32 {
        self.buf.borrow().len() as i32
    }
    pub fn reset(&self) {
        self.buf.borrow_mut().clear();
    }
    pub fn flush(&self) {}
    pub fn close(&self) {}
    /// `writeTo(OutputStream)` — drains into another stream.
    pub fn write_to(&self, out: &JavaOutputStream) {
        out.write_1(&self.to_byte_array());
    }
}
impl std::io::Write for JavaByteArrayOutputStream {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.buf.borrow_mut().extend_from_slice(buf);
        Ok(buf.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

// ===========================================================================
// Char / print writers
// ===========================================================================

/// Carrier for the whole char-writer family
/// (`Writer`/`OutputStreamWriter`/`BufferedWriter`/`FileWriter`/`PrintWriter`/
/// `PrintStream`) — a boxed `std::io::Write` sink that text is UTF-8-encoded
/// into. Has the union of write/print/println methods so any abstract-typed
/// writer field resolves on this one Rust type.
#[derive(Clone)]
pub struct JavaWriter {
    inner: Rc<RefCell<Box<dyn std::io::Write>>>,
    err: Rc<RefCell<bool>>,
}
impl Default for JavaWriter {
    fn default() -> Self {
        JavaWriter::new_1(std::io::sink())
    }
}
io_write_trivial_traits!(JavaWriter);
impl JavaWriter {
    pub fn new_1<W: IntoBoxedWrite>(inner: W) -> Self {
        JavaWriter {
            inner: Rc::new(RefCell::new(inner.into_boxed_write())),
            err: Rc::new(RefCell::new(false)),
        }
    }
    fn put(&self, s: &str) {
        let r = self.inner.borrow_mut().write_all(s.as_bytes());
        if r.is_err() {
            *self.err.borrow_mut() = true;
        }
    }
    /// `write(String)` / `write(char[])` / `write(int)` / `write(char)` —
    /// anything `ToString`.
    pub fn write<S: ToString>(&self, s: S) {
        self.put(&s.to_string());
    }
    /// `write(String s, int off, int len)`.
    pub fn write_3<S: ToString>(&self, s: S, off: i32, len: i32) {
        let s = s.to_string();
        let chars: Vec<char> = s.chars().collect();
        let off = (off.max(0) as usize).min(chars.len());
        let end = (off + len.max(0) as usize).min(chars.len());
        let slice: String = chars[off..end].iter().collect();
        self.put(&slice);
    }
    /// `append(char)` / `append(CharSequence)`.
    pub fn append<S: ToString>(&self, c: S) -> &Self {
        self.put(&c.to_string());
        self
    }
    pub fn new_line(&self) {
        self.put("\n");
    }
    /// `print()` (arity-0) — does not exist in the JDK; no-op for safety.
    pub fn print(&self) {}
    /// `print(x)` (arity-1).
    pub fn print_1<D: std::fmt::Display>(&self, x: D) {
        self.put(&format!("{}", x));
    }
    /// `println()` (arity-0) — just the line terminator.
    pub fn println(&self) {
        self.put("\n");
    }
    /// `println(x)` (arity-1).
    pub fn println_1<D: std::fmt::Display>(&self, x: D) {
        self.put(&format!("{}\n", x));
    }
    /// `printf(fmt, ...)` / `format(...)` — best-effort: prints the format
    /// string (real `%`-formatting needs runtime varargs, out of scope).
    /// Returns `&self` like Java's `PrintWriter.printf`.
    pub fn printf<F: ToString>(&self, fmt: F) -> &Self {
        self.put(&fmt.to_string());
        self
    }
    pub fn format<F: ToString>(&self, fmt: F) -> &Self {
        self.put(&fmt.to_string());
        self
    }
    pub fn flush(&self) {
        let r = self.inner.borrow_mut().flush();
        if r.is_err() {
            *self.err.borrow_mut() = true;
        }
    }
    pub fn close(&self) {
        self.flush();
    }
    pub fn check_error(&self) -> bool {
        *self.err.borrow()
    }
}
impl std::io::Write for JavaWriter {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.inner.borrow_mut().write(buf)
    }
    fn flush(&mut self) -> std::io::Result<()> {
        self.inner.borrow_mut().flush()
    }
}

/// `java.io.StringWriter` — accumulates text in an in-memory `String`. Kept as
/// its OWN mapped type because `to_string`/`get_buffer` aren't on the carrier.
#[derive(Clone)]
pub struct JavaStringWriter {
    buf: Rc<RefCell<String>>,
}
impl Default for JavaStringWriter {
    fn default() -> Self {
        Self::new()
    }
}
io_write_trivial_traits!(JavaStringWriter);
impl JavaStringWriter {
    pub fn new() -> Self {
        JavaStringWriter { buf: Rc::new(RefCell::new(String::new())) }
    }
    pub fn new_1(_size: i32) -> Self {
        Self::new()
    }
    pub fn write<S: ToString>(&self, s: S) {
        self.buf.borrow_mut().push_str(&s.to_string());
    }
    pub fn write_3<S: ToString>(&self, s: S, off: i32, len: i32) {
        let s = s.to_string();
        let chars: Vec<char> = s.chars().collect();
        let off = (off.max(0) as usize).min(chars.len());
        let end = (off + len.max(0) as usize).min(chars.len());
        let slice: String = chars[off..end].iter().collect();
        self.buf.borrow_mut().push_str(&slice);
    }
    pub fn append<S: ToString>(&self, s: S) -> &Self {
        self.buf.borrow_mut().push_str(&s.to_string());
        self
    }
    pub fn print(&self) {}
    pub fn print_1<D: std::fmt::Display>(&self, x: D) {
        self.buf.borrow_mut().push_str(&format!("{}", x));
    }
    pub fn println(&self) {
        self.buf.borrow_mut().push('\n');
    }
    pub fn println_1<D: std::fmt::Display>(&self, x: D) {
        self.buf.borrow_mut().push_str(&format!("{}\n", x));
    }
    pub fn to_string(&self) -> String {
        self.buf.borrow().clone()
    }
    pub fn get_buffer(&self) -> String {
        self.buf.borrow().clone()
    }
    pub fn flush(&self) {}
    pub fn close(&self) {}
}
impl std::io::Write for JavaStringWriter {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.buf.borrow_mut().push_str(&String::from_utf8_lossy(buf));
        Ok(buf.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}
impl std::fmt::Display for JavaStringWriter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.buf.borrow())
    }
}

// ===========================================================================
// Factory free-fns — disambiguate the arity-colliding concrete ctors. The
// write-side wrappers all build the carrier so abstract-typed assignments share
// one Rust type; carriers impl `std::io::Write` so these compose.
// ===========================================================================

// --- byte-stream carriers ---
pub fn java_output_stream<W: IntoBoxedWrite>(inner: W) -> JavaOutputStream {
    JavaOutputStream::new_1(inner)
}
pub fn java_file_output_stream<P: ToString>(path: P) -> JavaOutputStream {
    JavaOutputStream::new_1(JavaFileOutputStream::new_1(path))
}
pub fn java_file_output_stream_append<P: ToString>(path: P, append: bool) -> JavaOutputStream {
    JavaOutputStream::new_1(JavaFileOutputStream::new_2(path, append))
}
pub fn java_buffered_output_stream<W: IntoBoxedWrite>(inner: W) -> JavaOutputStream {
    JavaOutputStream::new_1(std::io::BufWriter::new(inner.into_boxed_write()))
}
pub fn java_buffered_output_stream_sized<W: IntoBoxedWrite>(
    inner: W,
    size: i32,
) -> JavaOutputStream {
    JavaOutputStream::new_1(std::io::BufWriter::with_capacity(
        size.max(0) as usize,
        inner.into_boxed_write(),
    ))
}

// --- char-writer carriers ---
pub fn java_writer<W: IntoBoxedWrite>(inner: W) -> JavaWriter {
    JavaWriter::new_1(inner)
}
pub fn java_output_stream_writer<W: IntoBoxedWrite>(out: W) -> JavaWriter {
    JavaWriter::new_1(out)
}
pub fn java_output_stream_writer_charset<W: IntoBoxedWrite, C: ToString>(
    out: W,
    _charset: C,
) -> JavaWriter {
    JavaWriter::new_1(out)
}
pub fn java_buffered_writer<W: IntoBoxedWrite>(w: W) -> JavaWriter {
    JavaWriter::new_1(std::io::BufWriter::new(w.into_boxed_write()))
}
pub fn java_buffered_writer_sized<W: IntoBoxedWrite>(w: W, size: i32) -> JavaWriter {
    JavaWriter::new_1(std::io::BufWriter::with_capacity(
        size.max(0) as usize,
        w.into_boxed_write(),
    ))
}
pub fn java_file_writer<P: ToString>(path: P) -> JavaWriter {
    JavaWriter::new_1(JavaFileOutputStream::new_1(path))
}
pub fn java_file_writer_append<P: ToString>(path: P, append: bool) -> JavaWriter {
    JavaWriter::new_1(JavaFileOutputStream::new_2(path, append))
}
pub fn java_print_writer<W: IntoBoxedWrite>(out: W) -> JavaWriter {
    JavaWriter::new_1(out)
}
pub fn java_print_writer_path<P: ToString>(path: P) -> JavaWriter {
    JavaWriter::new_1(JavaFileOutputStream::new_1(path))
}
pub fn java_print_stream<W: IntoBoxedWrite>(out: W) -> JavaWriter {
    JavaWriter::new_1(out)
}
pub fn java_print_stream_path<P: ToString>(path: P) -> JavaWriter {
    JavaWriter::new_1(JavaFileOutputStream::new_1(path))
}

#[cfg(test)]
mod io_write_tests {
    use super::*;

    #[test]
    fn string_writer_write_and_to_string() {
        // `&self` (interior mutability) — never declared `mut`, matching how a
        // field/static of this type is reached.
        let w = JavaStringWriter::new();
        w.write("hello");
        w.append(", ");
        w.write('!');
        w.println_1("world");
        assert_eq!(w.to_string(), "hello, !world\n");
        assert_eq!(w.get_buffer(), w.to_string());
    }

    #[test]
    fn byte_array_output_stream_write_and_to_byte_array() {
        let o = JavaByteArrayOutputStream::new();
        o.write(72); // 'H'
        o.write_1(&[105i8]); // "i"
        o.write_3(&[33i8, 63i8, 64i8], 0, 2); // "!?"
        assert_eq!(o.size(), 4);
        assert_eq!(o.to_byte_array(), vec![72i8, 105, 33, 63]);
        assert_eq!(o.to_string(), "Hi!?");
        o.reset();
        assert_eq!(o.size(), 0);
    }

    #[test]
    fn println_into_a_buffer() {
        // own-typed buffer
        let s = JavaStringWriter::new();
        s.println_1(42);
        s.print_1("x");
        assert_eq!(s.to_string(), "42\nx");
        // carrier over an std byte sink; `println_1` into it.
        let w = JavaWriter::new_1(Vec::<u8>::new());
        w.println_1("line");
        w.flush();
        assert!(!w.check_error());
        // a clone shares the same sink (Rc) — type-checks AND keeps writing.
        let w2 = w.clone();
        w2.print_1("more");
    }
}
