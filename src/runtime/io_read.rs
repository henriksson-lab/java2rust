// Java input/reader I/O stack -> concrete Rust runtime types.
//
// Two carrier types unify each family so that Java's abstract supertypes and
// concrete subtypes lower to the SAME Rust type (annotations, values, and
// composition all agree):
//   * `JavaInputStream`  — byte streams  (`InputStream`/`FileInputStream`/
//     `BufferedInputStream`/`ByteArrayInputStream`/`DataInputStream`).
//   * `JavaReader`       — char readers  (`Reader`/`BufferedReader`/
//     `FileReader`/`InputStreamReader`/`StringReader`/`LineNumberReader`).
//
// Both carriers `impl std::io::Read` (delegating to a boxed inner) so a
// reader can wrap a stream and a buffered wrapper can wrap either. Free
// factory fns (`java_file_input_stream`, `java_buffered_reader`, …) build a
// carrier from a concrete source; the translator's `visit_object_creation`
// special-cases each Java IO `new X(..)` to the matching factory fn, sidestepping
// the arity collision between e.g. `FileInputStream(path)` and
// `BufferedInputStream(stream)`.
//
// Interior mutability: a Java field / `static final` of these types lowers to a
// value reached via `&Self`, so every advancing method takes `&self` and uses a
// `RefCell` cursor (an `&mut self` reader would fail with E0596 at field/static
// call sites).

use std::cell::RefCell;
use std::io::{BufRead, BufReader, Cursor, Read};
use std::rc::Rc;

// =====================================================================
// Byte streams: JavaInputStream carrier
// =====================================================================

/// Carrier for every Java byte `InputStream` (and concrete subtypes). Holds a
/// boxed `Read` behind an `Rc<RefCell<..>>`:
///   * `RefCell` -> advancing methods take `&self` (a Java field/static of this
///     type is reached via `&Self`, so `&mut self` would E0596 at call sites).
///   * `Rc`      -> the carrier is `Clone` and shares ONE underlying stream
///     cursor across clones. That matches Java reference semantics (a Java
///     stream is a reference; "copying" the variable shares the stream) and lets
///     these types be used as struct fields / nullable locals like every other
///     mapped value type (the translator emits `.clone()` freely).
#[derive(Clone)]
pub struct JavaInputStream {
    inner: Rc<RefCell<Box<dyn Read>>>,
}

impl JavaInputStream {
    /// Wrap any `Read` source (used for composition / abstract assignment).
    pub fn new_1<R: Read + 'static>(inner: R) -> Self {
        JavaInputStream { inner: Rc::new(RefCell::new(Box::new(inner))) }
    }
    fn from_box(inner: Box<dyn Read>) -> Self {
        JavaInputStream { inner: Rc::new(RefCell::new(inner)) }
    }

    /// Java `read()` -> next byte as 0..=255, or -1 at EOF.
    pub fn read_byte(&self) -> i32 {
        let mut b = [0u8; 1];
        match self.inner.borrow_mut().read(&mut b) {
            Ok(0) | Err(_) => -1,
            Ok(_) => b[0] as i32,
        }
    }
    /// Java `read(byte[], off, len)` -> number of bytes read, or -1 at EOF.
    pub fn read(&self, buf: &mut [u8], off: i32, len: i32) -> i32 {
        let off = off.max(0) as usize;
        let len = len.max(0) as usize;
        let end = (off + len).min(buf.len());
        if off >= end {
            return 0;
        }
        match self.inner.borrow_mut().read(&mut buf[off..end]) {
            Ok(0) => -1,
            Ok(n) => n as i32,
            Err(_) => -1,
        }
    }
    /// Java `read(byte[])` -> fill the whole buffer; -1 at EOF.
    pub fn read_1(&self, buf: &mut [u8]) -> i32 {
        let len = buf.len() as i32;
        self.read(buf, 0, len)
    }
    /// Read all remaining bytes (helper; not a strict JDK method but handy).
    pub fn read_all_bytes(&self) -> Vec<u8> {
        let mut v = Vec::new();
        let _ = self.inner.borrow_mut().read_to_end(&mut v);
        v
    }
    pub fn available(&self) -> i32 {
        0
    }
    pub fn skip(&self, n: i64) -> i64 {
        let mut left = n.max(0) as u64;
        let mut buf = [0u8; 4096];
        let mut skipped: u64 = 0;
        let mut guard = self.inner.borrow_mut();
        while left > 0 {
            let want = left.min(buf.len() as u64) as usize;
            match guard.read(&mut buf[..want]) {
                Ok(0) | Err(_) => break,
                Ok(n) => {
                    skipped += n as u64;
                    left -= n as u64;
                }
            }
        }
        skipped as i64
    }
    pub fn close(&self) {}
    pub fn mark(&self, _read_limit: i32) {}
    pub fn reset(&self) {}
    pub fn mark_supported(&self) -> bool {
        false
    }
}

impl Read for JavaInputStream {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        self.inner.borrow_mut().read(buf)
    }
}

// `&JavaInputStream` is also `Read` (so a carrier reached via `&self` composes
// into another carrier without moving out of the field).
impl Read for &JavaInputStream {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        self.inner.borrow_mut().read(buf)
    }
}

// Field/value-type plumbing: the translator treats mapped types like plain
// values (struct fields, `Default::default()`, `==`, map keys), so the carrier
// must satisfy the same bounds the other runtime value types (`JavaFile`, …) do.
// Default = an empty stream; identity is by `Rc` pointer.
impl Default for JavaInputStream {
    fn default() -> Self {
        JavaInputStream::from_box(Box::new(Cursor::new(Vec::<u8>::new())))
    }
}
impl PartialEq for JavaInputStream {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.inner, &other.inner)
    }
}
impl Eq for JavaInputStream {}
impl std::hash::Hash for JavaInputStream {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        (Rc::as_ptr(&self.inner) as *const () as usize).hash(state);
    }
}

// =====================================================================
// Char readers: JavaReader carrier
// =====================================================================

/// Carrier for every Java char `Reader` (and concrete subtypes). Internally a
/// buffered reader over a boxed byte source (UTF-8), behind an `Rc<RefCell<..>>`
/// (see `JavaInputStream` for the rationale: `&self` methods + `Clone` sharing
/// one cursor + usable as a value-typed field).
#[derive(Clone)]
pub struct JavaReader {
    inner: Rc<RefCell<BufReader<Box<dyn Read>>>>,
    /// Line counter for LineNumberReader semantics (best effort). Shared across
    /// clones so the count survives a `.clone()`-then-read.
    line_number: Rc<std::cell::Cell<i32>>,
}

impl JavaReader {
    /// Wrap any `Read` byte source as a char reader (UTF-8).
    pub fn new_1<R: Read + 'static>(inner: R) -> Self {
        JavaReader {
            inner: Rc::new(RefCell::new(BufReader::new(Box::new(inner) as Box<dyn Read>))),
            line_number: Rc::new(std::cell::Cell::new(0)),
        }
    }
    fn from_box(inner: Box<dyn Read>, size: usize) -> Self {
        JavaReader {
            inner: Rc::new(RefCell::new(BufReader::with_capacity(size, inner))),
            line_number: Rc::new(std::cell::Cell::new(0)),
        }
    }

    /// Java `readLine()` -> the next line WITHOUT the trailing `\n`/`\r\n`, or
    /// `None` at end of stream. (The translator lowers
    /// `while ((line = in.readLine()) != null)` to `while let Some(line) = ...`.)
    pub fn read_line(&self) -> Option<String> {
        let mut s = String::new();
        let n = self.inner.borrow_mut().read_line(&mut s).unwrap_or(0);
        if n == 0 {
            return None;
        }
        // strip trailing newline (\n or \r\n)
        if s.ends_with('\n') {
            s.pop();
            if s.ends_with('\r') {
                s.pop();
            }
        }
        self.line_number.set(self.line_number.get() + 1);
        Some(s)
    }
    /// Java `read()` -> next char as its code point, or -1 at EOF.
    pub fn read(&self) -> i32 {
        let mut b = [0u8; 1];
        match self.inner.borrow_mut().read(&mut b) {
            Ok(0) | Err(_) => -1,
            Ok(_) => b[0] as i32,
        }
    }
    pub fn ready(&self) -> bool {
        self.inner.borrow_mut().fill_buf().map(|b| !b.is_empty()).unwrap_or(false)
    }
    pub fn skip(&self, n: i64) -> i64 {
        let mut left = n.max(0) as u64;
        let mut buf = [0u8; 4096];
        let mut skipped: u64 = 0;
        let mut guard = self.inner.borrow_mut();
        while left > 0 {
            let want = left.min(buf.len() as u64) as usize;
            match guard.read(&mut buf[..want]) {
                Ok(0) | Err(_) => break,
                Ok(n) => {
                    skipped += n as u64;
                    left -= n as u64;
                }
            }
        }
        skipped as i64
    }
    /// LineNumberReader: current line number (best effort).
    pub fn get_line_number(&self) -> i32 {
        self.line_number.get()
    }
    pub fn set_line_number(&self, n: i32) {
        self.line_number.set(n);
    }
    pub fn close(&self) {}
    pub fn mark(&self, _read_limit: i32) {}
    pub fn reset(&self) {}
    pub fn mark_supported(&self) -> bool {
        false
    }
}

impl Read for JavaReader {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        self.inner.borrow_mut().read(buf)
    }
}
impl Read for &JavaReader {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        self.inner.borrow_mut().read(buf)
    }
}

impl Default for JavaReader {
    fn default() -> Self {
        JavaReader::new_1(Cursor::new(Vec::<u8>::new()))
    }
}
impl PartialEq for JavaReader {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.inner, &other.inner)
    }
}
impl Eq for JavaReader {}
impl std::hash::Hash for JavaReader {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        (Rc::as_ptr(&self.inner) as *const () as usize).hash(state);
    }
}

// =====================================================================
// Factory free fns — one per concrete Java IO type.
//
// The translator emits these for `new <Type>(..)` (the bespoke IO-ctor handler
// in visit_object_creation), sidestepping the arity collision that would break
// `<carrier>::new_N`. Each returns the family carrier.
// =====================================================================

// ---- byte streams -> JavaInputStream ----

/// Accepts a Java `byte[]` in either Rust representation: `Vec<u8>`/`&[u8]` or
/// the signed `Vec<i8>`/`&[i8]` (Java `byte` is signed, so the translator lowers
/// `byte[]` to `Vec<i8>`). Yields the raw `u8` bytes.
pub trait JavaBytes {
    fn java_bytes(&self) -> Vec<u8>;
}
impl JavaBytes for [u8] {
    fn java_bytes(&self) -> Vec<u8> {
        self.to_vec()
    }
}
impl JavaBytes for Vec<u8> {
    fn java_bytes(&self) -> Vec<u8> {
        self.clone()
    }
}
impl JavaBytes for [i8] {
    fn java_bytes(&self) -> Vec<u8> {
        self.iter().map(|&b| b as u8).collect()
    }
}
impl JavaBytes for Vec<i8> {
    fn java_bytes(&self) -> Vec<u8> {
        self.iter().map(|&b| b as u8).collect()
    }
}
impl<T: JavaBytes + ?Sized> JavaBytes for &T {
    fn java_bytes(&self) -> Vec<u8> {
        (**self).java_bytes()
    }
}

/// Argument adapter for the reader/stream factories: anything `Read` (the
/// blanket case — `File`, `Cursor`, `JavaInputStream`, `&JavaInputStream`)
/// plus the doubly-borrowed carriers (`&&JavaInputStream`/`&&JavaReader`),
/// which arise when an already-borrowed field is passed by reference and are
/// not themselves `Read`. Mirrors `IntoBoxedWrite` on the writer side.
pub trait IntoBoxedRead {
    fn into_boxed_read(self) -> Box<dyn Read>;
}
impl<R: Read + 'static> IntoBoxedRead for R {
    fn into_boxed_read(self) -> Box<dyn Read> { Box::new(self) }
}
impl IntoBoxedRead for &&JavaInputStream {
    fn into_boxed_read(self) -> Box<dyn Read> { Box::new((**self).clone()) }
}
impl IntoBoxedRead for &&JavaReader {
    fn into_boxed_read(self) -> Box<dyn Read> { Box::new((**self).clone()) }
}

/// `new FileInputStream(path)` / `new FileInputStream(file)`.
pub fn java_file_input_stream<P: ToString>(path: P) -> JavaInputStream {
    match std::fs::File::open(path.to_string()) {
        Ok(f) => JavaInputStream::from_box(Box::new(f)),
        // Match Java's exception-as-empty fallback rather than panicking.
        Err(_) => JavaInputStream::from_box(Box::new(Cursor::new(Vec::<u8>::new()))),
    }
}
/// `new ByteArrayInputStream(bytes)`.
pub fn java_byte_array_input_stream<B: JavaBytes>(bytes: B) -> JavaInputStream {
    JavaInputStream::from_box(Box::new(Cursor::new(bytes.java_bytes())))
}
/// `new ByteArrayInputStream(bytes, off, len)`.
pub fn java_byte_array_input_stream_3<B: JavaBytes>(bytes: B, off: i32, len: i32) -> JavaInputStream {
    let b = bytes.java_bytes();
    let off = (off.max(0) as usize).min(b.len());
    let end = (off + len.max(0) as usize).min(b.len());
    JavaInputStream::from_box(Box::new(Cursor::new(b[off..end].to_vec())))
}
/// `new BufferedInputStream(in)` (buffering is implicit in the boxed read).
pub fn java_buffered_input_stream<R: IntoBoxedRead>(inner: R) -> JavaInputStream {
    JavaInputStream::from_box(Box::new(BufReader::new(inner.into_boxed_read())))
}
/// `new BufferedInputStream(in, size)`.
pub fn java_buffered_input_stream_2<R: IntoBoxedRead>(inner: R, size: i32) -> JavaInputStream {
    JavaInputStream::from_box(Box::new(BufReader::with_capacity(size.max(1) as usize, inner.into_boxed_read())))
}
/// `new InputStream(in)` / `new DataInputStream(in)` — pass-through wrap.
pub fn java_input_stream<R: IntoBoxedRead>(inner: R) -> JavaInputStream {
    JavaInputStream::new_1(inner.into_boxed_read())
}

// ---- char readers -> JavaReader ----

/// `new BufferedReader(reader)`.
pub fn java_buffered_reader<R: IntoBoxedRead>(inner: R) -> JavaReader {
    JavaReader::new_1(inner.into_boxed_read())
}
/// `new BufferedReader(reader, size)`.
pub fn java_buffered_reader_2<R: IntoBoxedRead>(inner: R, size: i32) -> JavaReader {
    JavaReader::from_box(Box::new(inner.into_boxed_read()), size.max(1) as usize)
}
/// `new InputStreamReader(in)` (UTF-8: just wrap the byte source as chars).
pub fn java_input_stream_reader<R: IntoBoxedRead>(inner: R) -> JavaReader {
    JavaReader::new_1(inner.into_boxed_read())
}
/// `new InputStreamReader(in, charset)` — charset ignored (UTF-8 assumed).
pub fn java_input_stream_reader_2<R: IntoBoxedRead, C: ToString>(inner: R, _charset: C) -> JavaReader {
    JavaReader::new_1(inner.into_boxed_read())
}
/// `new FileReader(path)` / `new FileReader(file)`.
pub fn java_file_reader<P: ToString>(path: P) -> JavaReader {
    match std::fs::File::open(path.to_string()) {
        Ok(f) => JavaReader::new_1(f),
        Err(_) => JavaReader::new_1(Cursor::new(Vec::<u8>::new())),
    }
}
/// `new StringReader(s)`.
pub fn java_string_reader<S: ToString>(s: S) -> JavaReader {
    JavaReader::new_1(Cursor::new(s.to_string().into_bytes()))
}
/// `new LineNumberReader(reader)`.
pub fn java_line_number_reader<R: Read + 'static>(inner: R) -> JavaReader {
    JavaReader::new_1(inner)
}
/// `new Reader(...)` placeholder carrier wrap.
pub fn java_reader<R: Read + 'static>(inner: R) -> JavaReader {
    JavaReader::new_1(inner)
}

#[cfg(test)]
mod io_read_tests {
    use super::*;

    #[test]
    fn string_reader_buffered_read_line() {
        // StringReader -> BufferedReader -> read_line() yields lines then None.
        let sr = java_string_reader("alpha\nbeta\r\ngamma");
        let r = java_buffered_reader(sr);
        assert_eq!(r.read_line(), Some("alpha".to_string()));
        assert_eq!(r.read_line(), Some("beta".to_string()));
        assert_eq!(r.read_line(), Some("gamma".to_string()));
        assert_eq!(r.read_line(), None);
        assert_eq!(r.read_line(), None);
    }

    #[test]
    fn byte_array_input_stream_read() {
        let s = java_byte_array_input_stream(vec![10u8, 20, 30]);
        assert_eq!(s.read_byte(), 10);
        assert_eq!(s.read_byte(), 20);
        assert_eq!(s.read_byte(), 30);
        assert_eq!(s.read_byte(), -1);
    }

    #[test]
    fn byte_array_input_stream_offset() {
        let s = java_byte_array_input_stream_3(vec![1u8, 2, 3, 4, 5], 1, 3);
        let mut buf = [0u8; 8];
        assert_eq!(s.read(&mut buf, 0, 8), 3);
        assert_eq!(&buf[..3], &[2, 3, 4]);
    }

    #[test]
    fn input_stream_to_reader_composition() {
        // InputStreamReader over a ByteArrayInputStream, then BufferedReader.
        let bais = java_byte_array_input_stream(b"one\ntwo".to_vec());
        let isr = java_input_stream_reader(bais);
        let r = java_buffered_reader(isr);
        assert_eq!(r.read_line(), Some("one".to_string()));
        assert_eq!(r.read_line(), Some("two".to_string()));
        assert_eq!(r.read_line(), None);
    }

    #[test]
    fn line_number_reader_counts() {
        let sr = java_string_reader("a\nb\nc");
        let r = java_line_number_reader(sr);
        let _ = r.read_line();
        let _ = r.read_line();
        assert_eq!(r.get_line_number(), 2);
    }
}
