# Java stdlib → Rust runtime: implementation checklist

The worklist for replacing opaque stdlib **stubs** with real Rust. Built by a 6-agent
audit over all 12 corpora (2026-06). Companion to memory `stdlib-stub-implementation`
and `TODO.md §4.2b`.

## What a "stub" is, and the rule
Unresolved JDK types are auto-emitted into `stub_<pkg>.rs` at each crate root as
`pub struct X {}` + `pub fn m(args) -> Ret { unimplemented!() }` (or `pub type X = Unknown`).
They **compile** (do nothing at runtime). The `stub_<pkg>.rs` files list the exact
called-method surface. **Mapping a type to a real runtime type MUST cover its full
method surface** (every box below) or previously-compiling stub calls regress to
E0599/E0061. Method names are snake_cased; `new`/`new_2` = ctor arities; `Unknown`
return = translator-inferred-any (a *capable* type — a concrete return may mismatch
call sites; measure).

## Proven recipe to add a runtime type (validated on `java.io.File`, errors −7)
1. One arm in `map_type_name` (`dump.rs ~7897`): `"X" => "crate::java_runtime::JavaX"`.
   This maps the annotation + ctor, flips `receiver_is_user_type`→false (methods emit by
   snake-case), AND suppresses the stub (`missing_type_key`).
2. Add `JavaX` to the runtime (see code-structure plan). NON-generic struct (annotations
   carry no generics). Return `Option<T>` not `Result` (no auto-`.unwrap()` for runtime
   types; fits the `as_readline_assign` read-loop lowering). Ctor arity-suffix is handled
   (`dump.rs ~6450`: `crate::java_runtime::` base + ≥2 args → `::new_2`).
3. Bound ctor/path/string args by `ToString` (not `AsRef<Path>`) — matches the stub's
   permissiveness; `Unknown` is `Display`.
4. Build → re-translate (clones over `/tmp/audit-*` only) → all-12 errors → KEEP iff no
   per-corpus regression.

## Vehicles
- **[runtime-type]** stateful → a `crate::java_runtime` struct.
- **[template]** stateless static/instance util → one-line rule in `src/stdlib.rs`.
- **[alias]** maps to an existing type (e.g. `Hashtable`→`HashMap`).
- **[drop/no-op]** semantics irrelevant in Rust (e.g. `WeakReference`, `Locale`).
- **[needs-crate: X]** requires a Cargo dependency — see the decision below.
- **[hard/defer]** no clean equivalent (engines, reflection, threads).

---

## DECISION (2026-06): well-known crates are ALLOWED
The user approved adding well-known Cargo crates to the generated crates where they unblock
a family. So the `[needs-crate]` families below are IN scope (not deferred for dependency
reasons) — though still ordered after the cheap pure-std tiers by effort. Prefer a single
well-maintained crate per family (regex, flate2, url, ureq/reqwest, zip, encoding_rs) and
add it to the generated `Cargo.toml` template only when that family's runtime is emitted.
Reference of which family needs which crate:
- **regex** (`java.util.regex.Pattern`/`Matcher`, ~4/12) — unblocks the most-used non-IO
  family. Java regex ≠ Rust `regex` in edge cases (no lookahead/backrefs).
- **flate2** (all of `java.util.zip`: GZIP/Deflater/Inflater, jsoup+trim+vcf).
- **url** + **reqwest**/**ureq** (`java.net.URL`/`HttpURLConnection`, jsoup HTTP).
- **zip** (`java.util.jar.JarFile`, jaligner).
- **encoding_rs** (`java.nio.charset` non-UTF-8; UTF-8 path is pure-std).

Pure-std-only deliverables if you decline crates: everything below NOT marked
`[needs-crate]` — which is the large majority (all of java.io/util/lang/text/awt-geom).
`CRC32`, `URLEncoder/Decoder` are small enough to hand-roll pure-std.

---

## Recommended implementation order (tiers)

**Tier 0 — code structure first** (one-time; see plan at bottom): move the runtime out of
the `crate_layout.rs` string literal into `src/runtime/*.rs` fragments. Do before bulk impl.
**✅ DONE (2026-06):** `src/runtime/{header,iter,io_file}.rs` + `concat!(include_str!())` in
`crate_layout.rs` + `#[cfg(test)] mod java_runtime_compiles` (include! compile-check). Emitted
`java_runtime.rs` byte-equivalent; trim unchanged (187); 92 tests/golden/compilecheck/0-warn.

**Tier 1 — cheap templates & aliases (E, no crates, biggest unblock-per-effort):**
`java.util.Arrays` (7/12), `java.util.Collections` (5/12), `Objects.hash`/`hashCode`,
`Map.Entry`→tuple, `Locale`→unit consts, `System` statics (exit/currentTimeMillis/
arraycopy/nanoTime/getProperty/gc), `java.util.logging.Logger`→eprintln, and the
`[alias]` maps (`Hashtable`/`IdentityHashMap`/`WeakHashMap`/`EnumMap`→`HashMap`,
`Properties`→`HashMap<String,String>`), plus constant/exception aliases (OutputKeys,
XPathConstants, `*Exception`→throws/String channel).

**Tier 2 — easy runtime-type leaves (E, pure-std):**
io byte/string leaves (`FileInputStream`/`FileOutputStream`→`std::fs::File`,
`ByteArrayInput/OutputStream`→`Cursor<Vec<u8>>`, `StringReader`/`StringWriter`), atomics
(`AtomicBoolean`/`Integer`/`Long`→`std::sync::atomic`), `StringTokenizer`, `BitSet`.

**Tier 3 — the file-I/O stack (M-H, pure-std; the user's stated priority):**
First settle the abstract-supertype design: `InputStream`/`OutputStream`/`Reader`/`Writer`
→ `Box<dyn std::io::Read/Write>` (Rust has NO subtyping), and every wrapper ctor accepts
`impl Read`/`impl Write` so `new BufferedReader(new InputStreamReader(new FileInputStream(f)))`
composes. Then `BufferedReader` (8/12, `read_line()->Option<String>`),
`InputStreamReader`/`OutputStreamWriter` (charset decode), `BufferedWriter`,
`Buffered{Input,Output}Stream`, `PrintStream` (route to stdout/stderr; `println` overloaded).

**Tier 4 — correctness-critical & first-class values (M-H):**
`java.util.Random` (must bit-match JDK 48-bit LCG incl. `nextGaussian` cache — jahmm/jhlabs
reproducibility); `DecimalFormat`/`NumberFormat` (pattern→`format!`); `Comparator`,
`java.util.function.*`, `Stream`/`Collector` (all need boxed-closure / iterator-lowering —
shared machinery); jts 2D-geometry (`GeneralPath`/`PathIterator`/`Point2D`/`Rectangle2D`
+ field-less `Color`/`Rectangle`/`Line2D`/`Ellipse2D` aliases need real FIELDS).

**Tier 5 — defer (H, dependency/engine-bound, low corpus count):**
zip (`[needs-crate: flate2]`), net/HTTP (`[needs-crate: url/reqwest]`), jar
(`[needs-crate: zip]`), `java.nio` buffers+charset+channels, `StreamTokenizer`,
`Object{Input,Output}Stream` serialization, the executor/thread-pool stack (trim only),
XML engines (Transformer/XSLT, XPath, StAX), font glyph tessellation, TLS, reflection.

> Out of scope: the `java.awt.image` raster cluster (BufferedImage/Raster/Kernel/Graphics2D)
> does NOT appear as java.* stubs — jhlabs resolves those against its own jar-recovered
> `com.jhlabs.*` types (app code). Only jts's `java.awt.geom.*` is real stdlib here.

---

## Checklist by package

### DONE
- [x] `java.io.File` → `JavaFile` (real, PathBuf-backed)
- [x] `StringBuilder`/`StringBuffer`/`CharSequence` → `String`
- [x] `Iterator`/`ListIterator` → `JavaIter`; `Optional` → `Option`; boxed numerics
- [x] String/Map/Set/List/Character/Objects(subset)/Integer/Long/Double stateless methods (`stdlib.rs`)
- [x] `Math.*`; `System.out/err.println` direct; `getClass().getName()`

### java.io — abstract supertypes [runtime-type, H] (→ `Box<dyn Read/Write>`)
- [ ] `InputStream`: available->i32, close, mark, mark_supported->bool, read()->i32, read(b)->i32, read(b,off,len)->i32, reset, skip(n)->i64  *(EOF: Java -1 vs Rust Ok(0))*
- [ ] `OutputStream`: close, flush, write(b,off,len)
- [ ] `Reader` (char-oriented, needs decode): close, read(buf,off,len)->i32
- [ ] `Writer` (char-oriented): append(c), flush, write(s)

### java.io — reader/writer wrappers [runtime-type]
- [ ] `BufferedReader` (8/12, M): new(r)/new_2(r,size), read_line()->Option<String>, read()->i32, ready()->bool, mark, reset, close
- [ ] `BufferedWriter` (M): new(w)/new_2(w,size), append(c), flush, write(s), close
- [ ] `InputStreamReader` (H, charset): new(in)/new_2(in,charset)
- [ ] `OutputStreamWriter` (H, charset): new(out)/new_2(out,charset)
- [ ] `FileReader` (E): new(file_or_name)
- [ ] `FileWriter` (E): new(file_or_name), write(s), close
- [ ] `StringReader` (E): new(s), close
- [ ] `StringWriter` (E): new(), to_string()->String   *(simplest in family)*
- [ ] `LineNumberReader` (M): new(r), read_line()->Option<String>

### java.io — byte streams [runtime-type, mostly E]
- [ ] `FileInputStream` (E): new(file_or_name) → `std::fs::File`
- [ ] `FileOutputStream` (E): new(file_or_name) → `File::create`
- [ ] `BufferedInputStream` (E): new(in)/new_2(in,size)
- [ ] `BufferedOutputStream` (E): new_2(out,size), close
- [ ] `ByteArrayInputStream` (E): new_3(buf,off,len) → `Cursor<Vec<u8>>`
- [ ] `ByteArrayOutputStream` (E): new(), reset, to_byte_array()->Vec<i8>, to_string()->String
- [ ] `PushbackInputStream` (M): new_2(in,size), read()->i32, unread(b), close
- [ ] `FilterInputStream` (M): field `in`: InputStream
- [ ] `PrintStream` (6/12, M): new(out), print(x), println(), println(x), printf(fmt,...), flush, check_error()->bool, close

### java.io — tokenizer / serialization [hard]
- [ ] `StreamTokenizer` (H): fields sval:Option<String>, ttype:i32, nval:f64; consts TT_EOF/TT_EOL/TT_WORD/TT_NUMBER; new(r), comment_char, eol_is_significant, lineno()->i32, next_token()->i32, ordinary_char, push_back, reset_syntax, whitespace_chars(lo,hi), word_chars(lo,hi)
- [ ] `ObjectInputStream`/`ObjectOutputStream` (H): new(stream), read_object/write_object  *(no Rust analog; serde or error)*

### java.io — exceptions [alias → error/String channel, E]
- [ ] `IOException` (10/12), `FileNotFoundException`, `UnsupportedEncodingException`, `ObjectStreamException`, `FileDescriptor`

### java.nio [runtime-type, mostly H — defer]
- [ ] `ByteBuffer` (H): allocate/wrap(static), array()->Vec<i8>, array_offset, capacity, compact, flip, has_array, has_remaining, limit, position, put, remaining  *(position/limit cursor model)*
- [ ] `CharBuffer` (H): wrap/wrap_3(static), has_remaining, position, slice
- [ ] `Buffer` (abstract base)
- [ ] `charset.Charset` (H, `[needs-crate: encoding_rs]` for non-UTF8): for_name/is_supported(static), can_encode, name()->String, new_decoder, new_encoder
- [ ] `charset.CharsetDecoder`/`CharsetEncoder`/`CoderResult`/`CodingErrorAction`/`StandardCharsets` (consts)
- [ ] `channels.Channels` (M): new_input_stream(static); `channels.SeekableByteChannel` (M): position
- [ ] `file.Files` (M): new_byte_channel/new_input_stream(static) → `std::fs`
- [ ] `file.Path` (M): get_file_name, to_absolute_path, to_uri  *(reuse JavaFile/PathBuf)*

### java.util — data structures [runtime-type]
- [x] `Random` → `JavaRandom` *(done 2026-06; JDK-bit-exact LCG, unit-tested vs known JDK values; uses `Cell` interior mutability so `next_*` take `&self` — works on `const LazyLock` static-final RNGs. ctor `new`/`new_seeded`; `nextInt(bound)`→`next_int_bound` via try_emit_known_method. jhlabs −14, vcf −5.)*
- [x] `BitSet` → `JavaBitSet` *(done; `Vec<u64>` words, `BitIndex` trait for char/int index args; arity-mangled `set`/`get`/`clear`/`flip` via try_emit_known_method; ctor special-case 1-arg→new_2.)*
- [x] `StringTokenizer` → `JavaStringTokenizer` *(done; eager VecDeque tokenizer; `nextToken`/`nextElement` added to `is_mutating_method`. jaligner −1.)*
- [ ] `PriorityQueue` (M): new()/new_1(cmp), add/offer, peek/poll, remove, is_empty, size  *(min-heap; invert vs BinaryHeap)*

### java.util — aliases [alias → HashMap/HashSet, E]
- [ ] `Hashtable`, `IdentityHashMap`, `WeakHashMap`, `EnumMap` → `HashMap`
- [ ] `EnumSet` (M) → `HashSet`; `of`/`add`/`contains`/`size` easy; `all_of`/`complement_of` need enum-variant iteration
- [ ] `Properties` → `HashMap<String,String>`: get_property(k)/(k,default), set_property, put_all, load(stream)  *(load = .properties parser)*

### java.util — utility classes [template]
- [ ] `Arrays` (7/12, E-M): sort/sort(4-arg), as_list, fill/fill(4-arg), copy_of, copy_of_range, binary_search(miss=-(ins)-1), equals, hash_code
- [ ] `Collections` (5/12, E-M): sort(cmp), reverse, min, reverse_order, unmodifiable_list/map(identity), empty_list/empty_set, singleton_list
- [ ] `Objects` (E-M): hash(varargs fold), hash_code  *(equals/requireNonNull/toString already done)*
- [ ] `Map.Entry` (E): get_key/get_value → tuple .0/.1
- [ ] `Locale` (E): consts ENGLISH/ROOT/US → unit value

### java.util — first-class values [runtime-type / boxed closures, M-H]
- [ ] `Comparator` (M-H): comparing_int(static), compare(a,b), comparing/reversed/then_comparing/natural_order  *(Box<dyn Fn(&T,&T)->Ordering>)*
- [ ] `function.*` (M-H): Function.apply, BiConsumer.accept, Consumer.accept, Predicate.test, Supplier.get, UnaryOperator.apply  *(ideally lower lambdas → Rust closures)*
- [ ] `stream.Stream` (M-H): of(static), map, collect, +filter/forEach/count/toArray/...  *(extend bespoke iterator-lowering; prioritize Collectors.toList/toSet/joining)*
- [ ] `stream.Collector`/`StreamSupport`/`Spliterator(s)` (H, defer)

### java.util.concurrent [runtime-type]
- [ ] `atomic.AtomicBoolean` (3/12, E): new()/new_1(v), get->bool, set, compare_and_set
- [ ] `atomic.AtomicInteger` (E): new_1(v), increment_and_get->i32, get/set/get_and_increment/add_and_get/compare_and_set
- [ ] `atomic.AtomicLong` (E): new(), get->i64, set, add_and_get, increment_and_get
- [ ] `locks.ReentrantLock` (E-M): lock/unlock → Mutex or no-op
- [ ] **executor stack (H, defer — trim only):** Future.get, ThreadPoolExecutor(6-arg ctor)/submit/shutdown/await_termination, ArrayBlockingQueue(offer/poll/put/take/peek/remaining_capacity), Executors.default_thread_factory, TimeUnit consts, ThreadFactory/Execution/TimeoutException aliases

### java.util.regex [needs-crate: regex, M]
- [ ] `Pattern`: compile(re)/compile(re,flags), matcher(in)->Matcher, split(in)->Vec<String>, to_string, const CASE_INSENSITIVE
- [ ] `Matcher` (stateful): find()->bool, group(n)->String, matches()->bool, replace_all(repl)->String

### java.text [runtime-type, M]
- [ ] `DecimalFormat` (4/12): new(pat)/new(pat,symbols), format(num)->String, apply_pattern, set_decimal_separator_always_shown, set_maximum_fraction_digits
- [ ] `DecimalFormatSymbols` (E): new(), set_decimal_separator
- [ ] `NumberFormat` (M): get_instance()/get_instance(locale)(static), format, set_maximum_fraction_digits  *(share DecimalFormat)*

### java.net [needs-crate: url/reqwest mostly, mixed]
- [ ] `URLEncoder`/`URLDecoder` (E, hand-roll pure-std): encode/decode(s,charset)
- [ ] `URL` (5/12, H): new(spec)/new(ctx,spec)/new(proto,host,port,file), open_stream/open_connection, get_file/get_path/get_host/get_port/get_protocol/get_query/get_ref/get_user_info, to_uri  *(parsing=url crate; network=reqwest)*
- [ ] `URI` (M, url crate): new(7-arg), to_ascii_string
- [ ] `HttpURLConnection` (H, reqwest): connect/disconnect, set/get_request_method, add_request_property, set_do_output, timeouts, get_input/error/output_stream, get_response_code/message, get_content_length/type, get_header_field(i)/key(i), get_url
- [ ] small value types (E): `Proxy`(new,type), `InetSocketAddress`(create_unresolved), `PasswordAuthentication`(new), `IDN`(to_ascii [needs-crate: idna]), `JarURLConnection`(get_jar_file), `CookieManager`; aliases Authenticator/CookieStore/MalformedURLException

### java.util.zip [needs-crate: flate2, M-H — defer]
- [ ] `GZIPInputStream`(new(in)), `GZIPOutputStream`, `Inflater`(new(nowrap)), `InflaterInputStream`(new_2)
- [ ] `Deflater` (H): new(level,nowrap), deflate(b,off,len,flush), set_input, set_dictionary, needs_input, finish, consts
- [ ] `CRC32` (E, hand-roll pure-std): new, reset, update(b,off,len), get_value()->i64

### java.util.jar [needs-crate: zip — defer]
- [ ] `JarFile`(entries/get_manifest), `JarEntry`(get_name/is_directory), `Manifest`(new(in)/get_main_attributes), `Attributes`(get_value)  *(Manifest parse = pure-std)*

### java.lang.ref [drop/no-op, E]
- [ ] `WeakReference`/`SoftReference`: new(referent), get()->value  *(transparent wrapper)*

### java.lang.reflect [hard/defer]
- [ ] `Method`(invoke), `Constructor`(new_instance) — H, no Rust analog
- [ ] `Array` (M, template): new_instance(type,len), get(arr,i), get_length(arr)

### java.awt.geom (jts) [runtime-type, M-H] — stubs lack FIELDS (field access dangles)
- [ ] `Color`/`Rectangle`/`Line2D`/`Ellipse2D`: add real FIELDS (currently `= Unknown`)
- [ ] `GeneralPath` (H): consts WIND_EVEN_ODD, new()/new_1/new_2, append, close_path, contains, get_bounds/get_bounds2_d, get_path_iterator, intersects, line_to, move_to  *(several wrongly &self → &mut self)*
- [ ] `PathIterator` (M): consts SEG_*, current_segment, is_done, next
- [ ] `Point2D` (M): get_x/get_y/set_location + x,y fields
- [ ] `Rectangle2D` (M): add + x,y,width,height fields
- [ ] `AffineTransform`(get_scale_instance), `Font`(consts+new+create_glyph_vector+get_size), `Shape`(trait), `FontRenderContext`(new), `GlyphVector` (H defer)

### javax.xml.* + org.xml.sax [mostly H/defer; constants E]
- [ ] **Easy consts/aliases:** `OutputKeys.*`, `XPathConstants.NODESET`, `XMLInputFactory` property strings → real `&str`/enum consts; `*Exception` (ParserConfiguration/XMLStream/SAX/SAXParse) → throws/String channel
- [ ] **Medium factory/builder wiring (M):** DocumentBuilderFactory/DocumentBuilder, SAXParserFactory/SAXParser, StreamResult/DOMSource, InputSource/AttributesImpl, Attributes, ErrorHandler(trait), ContentHandler/DefaultHandler(traits)  *(depends on org.w3c.dom sibling stub + a backing parser)*
- [ ] **Defer (H, engines):** Transformer.transform (XSLT), XPathFactory/XPathExpression.evaluate, XMLInputFactory/XMLStreamReader (StAX)

### java.lang.System statics
- [x] `System.exit(code)` → `std::process::exit((code) as i32)` *(done 2026-06; 43 sites in jaligner/varscan)*
- [x] `System.currentTimeMillis()` / `nanoTime()` → `SystemTime::now()` epoch millis/nanos
- [x] `System.gc()` → `()` no-op; `getProperty(k)`/`(k,default)` → `std::env::var` best-effort
- [ ] `System.arraycopy(src,sp,dst,dp,len)` → slice `clone_from_slice` (4-arg template; not yet)

### misc catch-all
- [ ] `java.util.logging.Logger` (E, template): get_logger(static), info/warning/severe/fine/log → eprintln/log crate
- [ ] `javax.net.ssl.*` (H, defer): HttpsURLConnection/SSLContext/SSLSocketFactory (TLS)
- [ ] `javax.swing.tree.TreeNode` (E, alias/trait)

---

## Code structure plan (Tier 0 — do first)
Current: runtime is a single `const JAVA_RUNTIME: &str = "\…"` in `crate_layout.rs:1073`
(hand-escaped `\"`, no rustfmt, no compile check). Won't scale.

**Adopt option (a): real `src/runtime/*.rs` fragments assembled via `concat!(include_str!)`.**
Keeps the flat `crate::java_runtime::Type` module → **zero changes** to the mod-tree
generator (`gen_mod_file`/`merge_colliding_modules`) and to `dump.rs` consumer paths.

Steps:
1. `src/runtime/header.rs` = exactly `//! java2rust runtime support.` + `#![allow(dead_code)]`
   (inner attrs MUST lead the emitted file → header concat'd first; no other fragment may
   have inner attrs).
2. `src/runtime/iter.rs` (move `JavaIter`), `src/runtime/io_file.rs` (move `JavaFile`,
   un-escaping the `\"`). Then `io_read.rs`, `io_write.rs`, `util.rs`, `text.rs`,
   `random.rs`, … as implemented. Each fragment = item-level-valid Rust (rustfmt-able),
   trailing newline, no inner attrs.
3. `crate_layout.rs`: `const JAVA_RUNTIME: &str = concat!(include_str!("runtime/header.rs"),
   include_str!("runtime/iter.rs"), include_str!("runtime/io_file.rs"), …);`
   (`include_str!` resolves relative to `crate_layout.rs`). `finish_crate`'s
   `std::fs::write(out_root.join("java_runtime.rs"), JAVA_RUNTIME)` is unchanged.
4. Add a `#[cfg(test)] mod java_runtime_compiles { #![allow(dead_code)] include!("runtime/iter.rs"); include!("runtime/io_file.rs"); … }`
   so the translator's own `cargo test`/`build` type-checks the runtime (today nothing
   does — a broken runtime is only caught when a corpus builds). Keep the `concat!` and
   `include!` lists in sync. Note generated crates are edition 2021 — stay in that subset.
5. Verify: `cargo test` (new include! test) + `tools/jts_check.sh` (emitted `java_runtime.rs`
   byte-equivalent modulo rustfmt; corpus errors unchanged).
6. Each future type: drop `runtime/<area>.rs`, add one `include_str!` + one `include!` line,
   and (if mapped) one `map_type_name` arm (~`dump.rs:7897`). No generator changes ever.
