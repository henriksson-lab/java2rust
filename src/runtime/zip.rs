// `java.util.zip` -> real (de)compression backed by the `flate2` crate.
//
// GZIP/Inflater STREAMS compose with the I/O carriers in `io_read.rs`/
// `io_write.rs`: `new GZIPInputStream(in)` routes (visit_object_creation) to the
// `java_gzip_input_stream` factory, which wraps the inner reader in
// `flate2::read::GzDecoder` and yields a `JavaInputStream` carrier — so
// `new BufferedInputStream(new GZIPInputStream(f))` composes like the rest of the
// read stack. (This clears the known vcf residual where `GZIPInputStream` was a
// named stub that didn't impl `Read`.)
//
// `Inflater`/`Deflater`/`CRC32` are OWN-typed runtime structs (mapped by
// map_type_name to `crate::java_runtime::JavaInflater`/`JavaDeflater`/`JavaCRC32`).
// They are stateful, so — like every runtime value type here — they use interior
// mutability (`RefCell`/`Cell`) and take `&self` (a Java field / `static final` of
// one of these is reached via `&Self`; an `&mut self` method would E0596 at those
// call sites). They `#[derive(Clone)]` (sharing state via `Rc`) and hand-roll the
// trivial `PartialEq`/`Eq`/`Hash` so they satisfy the same value-type plumbing as
// the opaque stub they replace.

// NOTE: this fragment is concat!'d AFTER io_read.rs/io_write.rs into one
// `java_runtime.rs` file, so `Rc`/`RefCell`/`Read` are already imported there;
// to avoid duplicate-import (E0252) in both the emitted file and the
// `java_runtime_compiles` include! test, refer to std types by FULL path and
// import only what no earlier fragment brings in.
use std::cell::Cell;

// =====================================================================
// GZIP / Inflater STREAM factories -> JavaInputStream carrier
// =====================================================================

/// `new GZIPInputStream(in)` / `new GZIPInputStream(in, size)`: wrap the inner
/// reader in a gzip decoder and yield the byte-stream carrier so it composes with
/// `BufferedInputStream`/`InputStreamReader`/… Accepts the same `IntoBoxedRead`
/// argument family as the other read factories (a `Read` source or a borrowed
/// carrier).
pub fn java_gzip_input_stream<R: IntoBoxedRead>(inner: R) -> JavaInputStream {
    JavaInputStream::new_1(flate2::read::GzDecoder::new(inner.into_boxed_read()))
}
pub fn java_gzip_input_stream_2<R: IntoBoxedRead>(inner: R, _size: i32) -> JavaInputStream {
    JavaInputStream::new_1(flate2::read::GzDecoder::new(inner.into_boxed_read()))
}

/// `new InflaterInputStream(in)` / `(in, inflater)` / `(in, inflater, size)`:
/// zlib/raw-deflate decode of the inner reader. The optional `Inflater` arg only
/// carries the `nowrap` flag in Java; honour it (raw deflate vs zlib) when
/// present, else assume zlib.
pub fn java_inflater_input_stream<R: IntoBoxedRead>(inner: R) -> JavaInputStream {
    JavaInputStream::new_1(flate2::read::ZlibDecoder::new(inner.into_boxed_read()))
}
pub fn java_inflater_input_stream_2<R: IntoBoxedRead>(inner: R, inflater: JavaInflater) -> JavaInputStream {
    if inflater.nowrap.get() {
        JavaInputStream::new_1(flate2::read::DeflateDecoder::new(inner.into_boxed_read()))
    } else {
        JavaInputStream::new_1(flate2::read::ZlibDecoder::new(inner.into_boxed_read()))
    }
}
pub fn java_inflater_input_stream_3<R: IntoBoxedRead>(inner: R, inflater: JavaInflater, _size: i32) -> JavaInputStream {
    java_inflater_input_stream_2(inner, inflater)
}

// =====================================================================
// GZIP / Deflater OUTPUT factories -> JavaOutputStream carrier
// =====================================================================

/// `new GZIPOutputStream(out)` / `(out, size)`: gzip-compress everything written
/// and yield the byte-sink carrier. `flate2::write::GzEncoder` finishes on drop.
pub fn java_gzip_output_stream<W: IntoBoxedWrite>(out: W) -> JavaOutputStream {
    JavaOutputStream::new_1(flate2::write::GzEncoder::new(
        out.into_boxed_write(),
        flate2::Compression::default(),
    ))
}
pub fn java_gzip_output_stream_2<W: IntoBoxedWrite>(out: W, _size: i32) -> JavaOutputStream {
    java_gzip_output_stream(out)
}

/// `new DeflaterOutputStream(out)` / `(out, deflater)`: zlib-compress.
pub fn java_deflater_output_stream<W: IntoBoxedWrite>(out: W) -> JavaOutputStream {
    JavaOutputStream::new_1(flate2::write::ZlibEncoder::new(
        out.into_boxed_write(),
        flate2::Compression::default(),
    ))
}
pub fn java_deflater_output_stream_2<W: IntoBoxedWrite>(out: W, _deflater: JavaDeflater) -> JavaOutputStream {
    java_deflater_output_stream(out)
}

// =====================================================================
// CRC32 (pure-std hand-roll, no crate)
// =====================================================================

/// `java.util.zip.CRC32` — IEEE CRC-32 (the same polynomial Java uses). State is
/// the running checksum; `getValue()` returns it as an unsigned value in an `i64`
/// (Java's `long`, 0..=0xFFFFFFFF).
#[derive(Clone)]
pub struct JavaCRC32 {
    crc: Rc<Cell<u32>>,
}
impl JavaCRC32 {
    pub fn new() -> Self {
        JavaCRC32 { crc: Rc::new(Cell::new(0)) }
    }
    pub fn reset(&self) {
        self.crc.set(0);
    }
    /// `update(int b)` — one byte (low 8 bits).
    pub fn update(&self, b: i32) {
        self.update_bytes(&[b as u8]);
    }
    /// `update(byte[] b)`.
    pub fn update_1<B: JavaBytes>(&self, b: B) {
        self.update_bytes(&b.java_bytes());
    }
    /// `update(byte[] b, int off, int len)`.
    pub fn update_3<B: JavaBytes>(&self, b: B, off: i32, len: i32) {
        let bytes = b.java_bytes();
        let off = (off.max(0) as usize).min(bytes.len());
        let end = (off + len.max(0) as usize).min(bytes.len());
        self.update_bytes(&bytes[off..end]);
    }
    fn update_bytes(&self, bytes: &[u8]) {
        let mut crc = !self.crc.get();
        for &byte in bytes {
            crc ^= byte as u32;
            for _ in 0..8 {
                let mask = (crc & 1).wrapping_neg();
                crc = (crc >> 1) ^ (0xEDB8_8320 & mask);
            }
        }
        self.crc.set(!crc);
    }
    pub fn get_value(&self) -> i64 {
        self.crc.get() as i64
    }
}
impl Default for JavaCRC32 {
    fn default() -> Self {
        Self::new()
    }
}
value_eq_hash!(JavaCRC32, crc.get());

// =====================================================================
// Inflater (decompressor) — own-typed runtime struct
// =====================================================================

/// `java.util.zip.Inflater`. Mostly used to carry the `nowrap` flag into an
/// `InflaterInputStream`, but also supports a best-effort streaming
/// `setInput`/`inflate` over `flate2`.
#[derive(Clone)]
pub struct JavaInflater {
    nowrap: Rc<Cell<bool>>,
    input: Rc<RefCell<Vec<u8>>>,
    finished: Rc<Cell<bool>>,
}
impl JavaInflater {
    pub fn new() -> Self {
        Self::new_with(false)
    }
    /// `new Inflater(boolean nowrap)`.
    pub fn new_1(nowrap: bool) -> Self {
        Self::new_with(nowrap)
    }
    fn new_with(nowrap: bool) -> Self {
        JavaInflater {
            nowrap: Rc::new(Cell::new(nowrap)),
            input: Rc::new(RefCell::new(Vec::new())),
            finished: Rc::new(Cell::new(false)),
        }
    }
    pub fn set_input<B: JavaBytes>(&self, b: B) {
        *self.input.borrow_mut() = b.java_bytes();
    }
    pub fn set_input_3<B: JavaBytes>(&self, b: B, off: i32, len: i32) {
        let bytes = b.java_bytes();
        let off = (off.max(0) as usize).min(bytes.len());
        let end = (off + len.max(0) as usize).min(bytes.len());
        *self.input.borrow_mut() = bytes[off..end].to_vec();
    }
    pub fn needs_input(&self) -> bool {
        self.input.borrow().is_empty()
    }
    pub fn finished(&self) -> bool {
        self.finished.get()
    }
    pub fn reset(&self) {
        self.input.borrow_mut().clear();
        self.finished.set(false);
    }
    pub fn end(&self) {}
    /// `inflate(byte[] out)` -> number of decompressed bytes written.
    pub fn inflate(&self, out: &mut [i8]) -> i32 {
        let input = std::mem::take(&mut *self.input.borrow_mut());
        let mut buf = Vec::with_capacity(out.len());
        let mut dec = flate2::read::DeflateDecoder::new(&input[..]).take(out.len() as u64);
        let n = dec.read_to_end(&mut buf).unwrap_or(0);
        let n = n.min(out.len());
        for (i, &byte) in buf[..n].iter().enumerate() {
            out[i] = byte as i8;
        }
        self.finished.set(true);
        n as i32
    }
}
impl Default for JavaInflater {
    fn default() -> Self {
        Self::new()
    }
}
noop_eq_hash!(JavaInflater);

// =====================================================================
// Deflater (compressor) — own-typed runtime struct
// =====================================================================

/// `java.util.zip.Deflater`. Streaming compress via `flate2::Compress`.
#[derive(Clone)]
pub struct JavaDeflater {
    nowrap: Rc<Cell<bool>>,
    level: Rc<Cell<i32>>,
    input: Rc<RefCell<Vec<u8>>>,
    comp: Rc<RefCell<flate2::Compress>>,
    finish: Rc<Cell<bool>>,
}
impl JavaDeflater {
    // Java compression-level / strategy / flush constants.
    pub const DEFAULT_COMPRESSION: i32 = -1;
    pub const NO_COMPRESSION: i32 = 0;
    pub const BEST_SPEED: i32 = 1;
    pub const BEST_COMPRESSION: i32 = 9;
    pub const DEFAULT_STRATEGY: i32 = 0;
    pub const FILTERED: i32 = 1;
    pub const HUFFMAN_ONLY: i32 = 2;
    pub const DEFLATED: i32 = 8;
    pub const NO_FLUSH: i32 = 0;
    pub const SYNC_FLUSH: i32 = 2;
    pub const FULL_FLUSH: i32 = 3;

    pub fn new() -> Self {
        Self::build(Self::DEFAULT_COMPRESSION, false)
    }
    /// `new Deflater(int level)`.
    pub fn new_1(level: i32) -> Self {
        Self::build(level, false)
    }
    /// `new Deflater(int level, boolean nowrap)`.
    pub fn new_2(level: i32, nowrap: bool) -> Self {
        Self::build(level, nowrap)
    }
    fn build(level: i32, nowrap: bool) -> Self {
        let lvl = if level < 0 { 6u32 } else { (level as u32).min(9) };
        JavaDeflater {
            nowrap: Rc::new(Cell::new(nowrap)),
            level: Rc::new(Cell::new(level)),
            input: Rc::new(RefCell::new(Vec::new())),
            comp: Rc::new(RefCell::new(flate2::Compress::new(
                flate2::Compression::new(lvl),
                !nowrap,
            ))),
            finish: Rc::new(Cell::new(false)),
        }
    }
    pub fn set_level(&self, level: i32) {
        self.level.set(level);
        let lvl = if level < 0 { 6u32 } else { (level as u32).min(9) };
        *self.comp.borrow_mut() =
            flate2::Compress::new(flate2::Compression::new(lvl), !self.nowrap.get());
    }
    pub fn set_strategy(&self, _strategy: i32) {}
    pub fn set_input<B: JavaBytes>(&self, b: B) {
        *self.input.borrow_mut() = b.java_bytes();
    }
    pub fn set_input_3<B: JavaBytes>(&self, b: B, off: i32, len: i32) {
        let bytes = b.java_bytes();
        let off = (off.max(0) as usize).min(bytes.len());
        let end = (off + len.max(0) as usize).min(bytes.len());
        *self.input.borrow_mut() = bytes[off..end].to_vec();
    }
    pub fn set_dictionary<B: JavaBytes>(&self, _b: B) {}
    pub fn set_dictionary_3<B: JavaBytes>(&self, _b: B, _off: i32, _len: i32) {}
    pub fn needs_input(&self) -> bool {
        self.input.borrow().is_empty()
    }
    pub fn finish(&self) {
        self.finish.set(true);
    }
    pub fn finished(&self) -> bool {
        self.finish.get() && self.input.borrow().is_empty()
    }
    pub fn end(&self) {}
    pub fn reset(&self) {
        self.input.borrow_mut().clear();
        self.finish.set(false);
        let lvl = {
            let l = self.level.get();
            if l < 0 { 6u32 } else { (l as u32).min(9) }
        };
        *self.comp.borrow_mut() =
            flate2::Compress::new(flate2::Compression::new(lvl), !self.nowrap.get());
    }
    /// `deflate(byte[] out)` / `(out, off, len)` / `(out, off, len, flush)` ->
    /// number of compressed bytes written into `out`. Drains the pending input.
    pub fn deflate(&self, out: &mut [i8]) -> i32 {
        let len = out.len() as i32;
        self.deflate_4(out, 0, len, Self::NO_FLUSH)
    }
    pub fn deflate_3(&self, out: &mut [i8], off: i32, len: i32) -> i32 {
        self.deflate_4(out, off, len, Self::NO_FLUSH)
    }
    pub fn deflate_4(&self, out: &mut [i8], off: i32, len: i32, flush: i32) -> i32 {
        use flate2::FlushCompress;
        let off = (off.max(0) as usize).min(out.len());
        let end = (off + len.max(0) as usize).min(out.len());
        if off >= end {
            return 0;
        }
        let input = std::mem::take(&mut *self.input.borrow_mut());
        let mut tmp = vec![0u8; end - off];
        let mut comp = self.comp.borrow_mut();
        let fl = if self.finish.get() {
            FlushCompress::Finish
        } else if flush == Self::SYNC_FLUSH {
            FlushCompress::Sync
        } else if flush == Self::FULL_FLUSH {
            FlushCompress::Full
        } else {
            FlushCompress::None
        };
        let before_out = comp.total_out();
        let _ = comp.compress(&input, &mut tmp, fl);
        let produced = ((comp.total_out() - before_out) as usize).min(tmp.len());
        for (i, &byte) in tmp[..produced].iter().enumerate() {
            out[off + i] = byte as i8;
        }
        produced as i32
    }
}
impl Default for JavaDeflater {
    fn default() -> Self {
        Self::new()
    }
}
noop_eq_hash!(JavaDeflater);

#[cfg(test)]
mod zip_tests {
    use super::*;

    #[test]
    fn crc32_known_value() {
        // CRC-32 of "123456789" is 0xCBF43926.
        let c = JavaCRC32::new();
        c.update_1(b"123456789".to_vec());
        assert_eq!(c.get_value(), 0xCBF4_3926);
        c.reset();
        assert_eq!(c.get_value(), 0);
    }

    #[test]
    fn crc32_byte_and_range() {
        let c = JavaCRC32::new();
        for &b in b"123456789" {
            c.update(b as i32);
        }
        assert_eq!(c.get_value(), 0xCBF4_3926);
        let c2 = JavaCRC32::new();
        c2.update_3(
            b"xx123456789yy".iter().map(|&b| b as i8).collect::<Vec<i8>>(),
            2,
            9,
        );
        assert_eq!(c2.get_value(), 0xCBF4_3926);
    }

    #[test]
    fn gzip_input_stream_decodes() {
        use std::io::Write;
        // Produce a real gzip member, then decode it through the input-stream
        // factory (the GZIPInputStream path the corpora exercise).
        let mut enc = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
        enc.write_all(b"hello gzip world").unwrap();
        let compressed = enc.finish().unwrap();
        let src = java_byte_array_input_stream(compressed);
        let dec = java_gzip_input_stream(src);
        assert_eq!(
            String::from_utf8(dec.read_all_bytes()).unwrap(),
            "hello gzip world"
        );
        // And it composes with the buffered carrier.
        let mut enc2 = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
        enc2.write_all(b"line one\nline two").unwrap();
        let c2 = enc2.finish().unwrap();
        let r = java_buffered_reader(java_input_stream_reader(java_gzip_input_stream(
            java_byte_array_input_stream(c2),
        )));
        assert_eq!(r.read_line(), Some("line one".to_string()));
        assert_eq!(r.read_line(), Some("line two".to_string()));
    }

    #[test]
    fn deflate_inflate_round_trip() {
        let d = JavaDeflater::new_2(JavaDeflater::BEST_COMPRESSION, true);
        d.set_input(b"abcabcabcabcabc".to_vec());
        d.finish();
        let mut out = vec![0i8; 256];
        let n = d.deflate(&mut out) as usize;
        assert!(n > 0);
        let inf = JavaInflater::new_1(true);
        inf.set_input(out[..n].to_vec());
        let mut back = vec![0i8; 256];
        let m = inf.inflate(&mut back) as usize;
        let s: Vec<u8> = back[..m].iter().map(|&b| b as u8).collect();
        assert_eq!(String::from_utf8(s).unwrap(), "abcabcabcabcabc");
    }
}
