//! jar-to-symbols — read dependency JAR(s) and emit a Java→Rust symbol map.
//!
//! A `.class` file carries ground-truth signatures (exact parameter/return
//! types, static-ness, throws) and — when the library ships nullability
//! annotations (`@Nullable`/`@CheckForNull`, CLASS retention) — real
//! nullability. We extract these into the same JSON the translator consumes via
//! `--link`, so referencing such a dependency emits precise types instead of
//! guessed stubs.
//!
//! What a JAR cannot give: Rust ownership (`&`/`&mut`) — Java is uniformly
//! by-reference — so borrowing stays heuristic (`&` for non-primitive params).
//! Overloads (same name, different params) collapse to one entry, since the map
//! keys methods by their bare Java name.
//!
//! Usage: jar-to-symbols <a.jar> [<b.jar> ...] [-o map.json]
//!
//! It warns about every type referenced in a signature that is not covered by
//! the provided JARs (and is not a JDK type), so you can add the missing JAR and
//! make the translation precise.

use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::fs::File;
use std::io::Read;

use cafebabe::attributes::{Annotation, AttributeData};
use cafebabe::descriptors::{FieldDescriptor, FieldType, ReturnDescriptor};
use cafebabe::{ClassFile, FieldInfo, MethodInfo};

use java2rust_rs::dump::{escape_rust_keyword, map_type_name};
use java2rust_rs::naming::camel_to_snake_case;
use java2rust_rs::symbol_map::{FieldSym, MethodSym, ParamSym, SymbolMap, TypeSym};

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut jars = Vec::new();
    let mut out: Option<String> = None;
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "-o" | "--output" => {
                i += 1;
                out = args.get(i).cloned();
            }
            other => jars.push(other.to_string()),
        }
        i += 1;
    }
    if jars.is_empty() {
        eprintln!("usage: jar-to-symbols <a.jar> [<b.jar> ...] [-o map.json]");
        std::process::exit(2);
    }

    // Read every .class from every jar first, so the "defined types" set spans
    // all provided jars before we look for unresolved references.
    let mut classes: Vec<Vec<u8>> = Vec::new();
    for jar in &jars {
        match read_classes(jar) {
            Ok(mut cs) => classes.append(&mut cs),
            Err(e) => {
                eprintln!("error reading {jar}: {e}");
                std::process::exit(1);
            }
        }
    }

    let mut map = SymbolMap::default();
    let mut defined: HashSet<String> = HashSet::new();
    let mut referenced: BTreeSet<String> = BTreeSet::new();
    let mut overloads = 0usize;

    for bytes in &classes {
        let class = match cafebabe::parse_class(bytes) {
            Ok(c) => c,
            Err(e) => {
                eprintln!("warning: skipping unparseable class: {e}");
                continue;
            }
        };
        process_class(&class, &mut map, &mut defined, &mut referenced, &mut overloads);
    }

    // Warn about referenced types we have no information about.
    let missing: Vec<&String> = referenced
        .iter()
        .filter(|fqn| !defined.contains(*fqn) && !is_jdk_fqn(fqn))
        .collect();
    eprintln!(
        "jar-to-symbols: {} types, {} classes from {} jar(s){}",
        map.types.len(),
        classes.len(),
        jars.len(),
        if overloads > 0 { format!(", {overloads} overloads collapsed") } else { String::new() }
    );
    if !missing.is_empty() {
        let mut by_pkg: BTreeMap<String, usize> = BTreeMap::new();
        for m in &missing {
            let pkg = m.rsplit_once('.').map(|(p, _)| p.to_string()).unwrap_or_default();
            *by_pkg.entry(pkg).or_default() += 1;
        }
        eprintln!(
            "\nWARNING: {} referenced type(s) are not covered by the provided JAR(s).",
            missing.len()
        );
        eprintln!("Provide the JAR(s) for these packages so translation can link precisely:");
        for (pkg, n) in &by_pkg {
            eprintln!("  {pkg}  ({n})");
        }
        eprintln!("Sample types: {}", missing.iter().take(8).map(|s| s.as_str()).collect::<Vec<_>>().join(", "));
    }

    let json = serde_json::to_string_pretty(&map).expect("serialize map");
    match out {
        Some(path) => {
            std::fs::write(&path, json).expect("write map");
            eprintln!("wrote {path}");
        }
        None => println!("{json}"),
    }
}

/// Read all `.class` entries (raw bytes) from a jar (zip).
fn read_classes(path: &str) -> Result<Vec<Vec<u8>>, String> {
    let file = File::open(path).map_err(|e| e.to_string())?;
    let mut zip = zip::ZipArchive::new(file).map_err(|e| e.to_string())?;
    let mut out = Vec::new();
    for i in 0..zip.len() {
        let mut entry = zip.by_index(i).map_err(|e| e.to_string())?;
        if !entry.name().ends_with(".class") {
            continue;
        }
        let mut buf = Vec::with_capacity(entry.size() as usize);
        entry.read_to_end(&mut buf).map_err(|e| e.to_string())?;
        out.push(buf);
    }
    Ok(out)
}

fn process_class(
    class: &ClassFile,
    map: &mut SymbolMap,
    defined: &mut HashSet<String>,
    referenced: &mut BTreeSet<String>,
    overloads: &mut usize,
) {
    // Skip non-public and synthetic classes — not part of the linkable API.
    use cafebabe::ClassAccessFlags as CF;
    if !class.access_flags.contains(CF::PUBLIC)
        || class.access_flags.contains(CF::SYNTHETIC)
        || class.access_flags.contains(CF::ANNOTATION)
    {
        return;
    }
    let fqn = binary_to_fqn(&class.this_class);
    defined.insert(fqn.clone());

    let kind = if class.access_flags.contains(CF::INTERFACE) {
        "trait"
    } else if class.access_flags.contains(CF::ENUM) {
        "enum"
    } else {
        "struct"
    };

    let mut t = TypeSym {
        rust_path: binary_to_rust_path(&class.this_class),
        kind: kind.to_string(),
        parent: class.super_class.as_ref().map(|s| binary_to_fqn(s)),
        interfaces: class.interfaces.iter().map(|i| binary_to_fqn(i)).collect(),
        generic: false,
        generic_params: Vec::new(),
        fields: BTreeMap::new(),
        static_fields: BTreeMap::new(),
        methods: BTreeMap::new(),
    };

    if let Some(sup) = &class.super_class {
        note_ref(referenced, sup);
    }
    for iface in &class.interfaces {
        note_ref(referenced, iface);
    }

    for f in &class.fields {
        process_field(f, &fqn, &mut t, referenced);
    }
    for m in &class.methods {
        process_method(m, &fqn, &mut t, referenced, overloads);
    }

    map.types.insert(fqn, t);
}

fn process_field(f: &FieldInfo, _fqn: &str, t: &mut TypeSym, referenced: &mut BTreeSet<String>) {
    use cafebabe::FieldAccessFlags as FF;
    if !(f.access_flags.contains(FF::PUBLIC) || f.access_flags.contains(FF::PROTECTED)) {
        return;
    }
    if f.access_flags.contains(FF::SYNTHETIC) {
        return;
    }
    note_field_ref(referenced, &f.descriptor);
    let java = f.name.to_string();
    let nullable = annotations_nullable(field_annotations(&f.attributes));
    // Prefer the generic Signature attribute; fall back to the erased descriptor.
    let rust_type = signature_attr(&f.attributes)
        .and_then(parse_field_signature)
        .unwrap_or_else(|| descriptor_to_rust(&f.descriptor));
    t.fields.insert(
        java.clone(),
        FieldSym {
            rust: rust_ident(&java),
            rust_type,
            nullable: nullable.unwrap_or(false),
        },
    );
}

fn process_method(
    m: &MethodInfo,
    fqn: &str,
    t: &mut TypeSym,
    referenced: &mut BTreeSet<String>,
    overloads: &mut usize,
) {
    use cafebabe::MethodAccessFlags as MF;
    if !(m.access_flags.contains(MF::PUBLIC) || m.access_flags.contains(MF::PROTECTED)) {
        return;
    }
    if m.access_flags.contains(MF::SYNTHETIC) || m.access_flags.contains(MF::BRIDGE) {
        return;
    }
    let java = m.name.to_string();
    if java == "<clinit>" {
        return;
    }
    let is_static = m.access_flags.contains(MF::STATIC);
    let is_ctor = java == "<init>";

    for p in &m.descriptor.parameters {
        note_field_ref(referenced, p);
    }
    if let ReturnDescriptor::Return(fd) = &m.descriptor.return_type {
        note_field_ref(referenced, fd);
    }

    // Generic signature (types only); aligned to the descriptor by arity, since
    // the Signature attribute may omit synthetic parameters.
    let sig = signature_attr(&m.attributes).and_then(parse_method_signature);
    let sig_params: Option<&Vec<String>> = sig.as_ref().and_then(|(p, _)| {
        if p.len() == m.descriptor.parameters.len() { Some(p) } else { None }
    });

    let param_anns = parameter_annotations(&m.attributes);
    let params: Vec<ParamSym> = m
        .descriptor
        .parameters
        .iter()
        .enumerate()
        .map(|(i, p)| {
            let by_ref = !is_primitive(p); // ownership stays heuristic (erased)
            let base = sig_params
                .map(|sp| sp[i].clone())
                .unwrap_or_else(|| descriptor_to_rust(p));
            ParamSym {
                rust_type: maybe_ref(by_ref, base),
                by_ref,
                mutable: false,
                nullable: param_anns.get(i).copied().flatten().unwrap_or(false),
            }
        })
        .collect();

    let (ret, ret_nullable) = match &m.descriptor.return_type {
        ReturnDescriptor::Void => (None, false),
        ReturnDescriptor::Return(fd) => {
            let base = match sig.as_ref() {
                Some((_, SigRet::Type(t))) => t.clone(),
                _ => descriptor_to_rust(fd),
            };
            (Some(base), annotations_nullable(method_annotations(&m.attributes)).unwrap_or(false))
        }
    };

    let rust = if is_ctor { "new".to_string() } else { rust_ident(&java) };
    let receiver = if is_static || is_ctor { "none" } else { "ref" };
    let sig = MethodSym {
        rust_path: format!("{}::{}", binary_pkg_path(fqn), rust),
        rust,
        receiver: receiver.to_string(),
        ret: if is_ctor { Some(simple_rust_name(fqn)) } else { ret },
        ret_nullable,
        // External (already-compiled) dep methods are real Rust signatures, not
        // our `throws`->`Result` translation, so they are never auto-unwrapped.
        throws: false,
        params,
    };

    use std::collections::btree_map::Entry;
    match t.methods.entry(java) {
        Entry::Vacant(e) => {
            e.insert(sig);
        }
        Entry::Occupied(mut e) => {
            // Overload: keep the richer (more-parameter) signature.
            *overloads += 1;
            if sig.params.len() > e.get().params.len() {
                e.insert(sig);
            }
        }
    }
}

// ---- generic signature parsing (JVMS §4.7.9.1) ----
//
// The erased `descriptor` loses generics; the optional `Signature` attribute
// preserves them (`List<String>`, `Map<K,V>`, type variables, wildcards). We
// parse it into the same Rust type strings the descriptor path produces, but
// with type arguments. Falls back to the descriptor on any parse failure or
// arity mismatch (the Signature attribute can omit synthetic parameters).

enum SigRet {
    Void,
    Type(String),
}

struct SigParser<'a> {
    s: &'a [u8],
    i: usize,
}

impl<'a> SigParser<'a> {
    fn new(s: &'a str) -> Self {
        SigParser { s: s.as_bytes(), i: 0 }
    }
    fn peek(&self) -> Option<u8> {
        self.s.get(self.i).copied()
    }
    fn bump(&mut self) -> Option<u8> {
        let c = self.peek();
        if c.is_some() {
            self.i += 1;
        }
        c
    }
    fn eat(&mut self, c: u8) -> Option<()> {
        if self.peek() == Some(c) {
            self.i += 1;
            Some(())
        } else {
            None
        }
    }

    /// Skip a leading `<TypeParameter+>` block (class/method type parameters) by
    /// balancing angle brackets — their bounds can nest arbitrarily.
    fn skip_type_params(&mut self) {
        if self.peek() != Some(b'<') {
            return;
        }
        let mut depth = 0;
        while let Some(c) = self.bump() {
            match c {
                b'<' => depth += 1,
                b'>' => {
                    depth -= 1;
                    if depth == 0 {
                        break;
                    }
                }
                _ => {}
            }
        }
    }

    fn parse_method(&mut self) -> Option<(Vec<String>, SigRet)> {
        self.skip_type_params();
        self.eat(b'(')?;
        let mut params = Vec::new();
        while self.peek() != Some(b')') {
            params.push(self.parse_type()?);
        }
        self.eat(b')')?;
        let ret = if self.peek() == Some(b'V') {
            self.bump();
            SigRet::Void
        } else {
            SigRet::Type(self.parse_type()?)
        };
        // Throws (`^...`) are ignored.
        Some((params, ret))
    }

    fn parse_type(&mut self) -> Option<String> {
        match self.peek()? {
            b'[' => {
                self.bump();
                Some(format!("Vec<{}>", self.parse_type()?))
            }
            b'T' => {
                self.bump();
                let name = self.read_until(&[b';']);
                self.eat(b';')?;
                Some(name) // a type variable renders as its name
            }
            b'L' => self.parse_class_type(),
            b'B' => self.base(b'B', "i8"),
            b'C' => self.base(b'C', "char"),
            b'D' => self.base(b'D', "f64"),
            b'F' => self.base(b'F', "f32"),
            b'I' => self.base(b'I', "i32"),
            b'J' => self.base(b'J', "i64"),
            b'S' => self.base(b'S', "i16"),
            b'Z' => self.base(b'Z', "bool"),
            _ => None,
        }
    }

    fn base(&mut self, c: u8, rust: &str) -> Option<String> {
        self.eat(c)?;
        Some(rust.to_string())
    }

    fn parse_class_type(&mut self) -> Option<String> {
        self.eat(b'L')?;
        // First SimpleClassTypeSignature carries the package (via `/`).
        let mut name = self.read_until(&[b'<', b';', b'.']);
        let mut args = if self.peek() == Some(b'<') { self.parse_type_args()? } else { Vec::new() };
        // Nested-class suffixes `.Inner<...>` — the innermost names the type.
        while self.peek() == Some(b'.') {
            self.bump();
            name = self.read_until(&[b'<', b';', b'.']);
            args = if self.peek() == Some(b'<') { self.parse_type_args()? } else { Vec::new() };
        }
        self.eat(b';')?;
        let simple = name.rsplit(['/', '$']).next().unwrap_or(&name);
        let base = map_type_name(simple).to_string();
        if args.is_empty() {
            Some(base)
        } else {
            Some(format!("{base}<{}>", args.join(", ")))
        }
    }

    fn parse_type_args(&mut self) -> Option<Vec<String>> {
        self.eat(b'<')?;
        let mut out = Vec::new();
        while self.peek() != Some(b'>') {
            match self.peek()? {
                b'*' => {
                    self.bump();
                    out.push("_".to_string()); // unbounded wildcard `?`
                }
                b'+' | b'-' => {
                    self.bump(); // `? extends`/`? super`: render the bound
                    out.push(self.parse_type()?);
                }
                _ => out.push(self.parse_type()?),
            }
        }
        self.eat(b'>')?;
        Some(out)
    }

    fn read_until(&mut self, stops: &[u8]) -> String {
        let start = self.i;
        while let Some(c) = self.peek() {
            if stops.contains(&c) {
                break;
            }
            self.i += 1;
        }
        String::from_utf8_lossy(&self.s[start..self.i]).into_owned()
    }
}

fn parse_method_signature(sig: &str) -> Option<(Vec<String>, SigRet)> {
    SigParser::new(sig).parse_method()
}

fn parse_field_signature(sig: &str) -> Option<String> {
    SigParser::new(sig).parse_type()
}

/// The raw `Signature` attribute string, if present.
fn signature_attr<'a>(attrs: &'a [cafebabe::attributes::AttributeInfo<'a>]) -> Option<&'a str> {
    attrs.iter().find_map(|a| match &a.data {
        AttributeData::Signature(s) => Some(s.as_ref()),
        _ => None,
    })
}

// ---- type mapping ----

fn descriptor_to_rust(fd: &FieldDescriptor) -> String {
    let mut s = field_type_to_rust(&fd.field_type);
    for _ in 0..fd.dimensions {
        s = format!("Vec<{s}>");
    }
    s
}

fn field_type_to_rust(ft: &FieldType) -> String {
    match ft {
        FieldType::Byte => "i8".into(),
        FieldType::Char => "char".into(),
        FieldType::Double => "f64".into(),
        FieldType::Float => "f32".into(),
        FieldType::Integer => "i32".into(),
        FieldType::Long => "i64".into(),
        FieldType::Short => "i16".into(),
        FieldType::Boolean => "bool".into(),
        // `java/lang/String` -> simple name -> the translator's mapping.
        FieldType::Object(cn) => {
            let simple = cn.rsplit('/').next().unwrap_or(cn).rsplit('$').next().unwrap_or(cn);
            map_type_name(simple).to_string()
        }
    }
}

fn is_primitive(fd: &FieldDescriptor) -> bool {
    fd.dimensions == 0 && !matches!(fd.field_type, FieldType::Object(_))
}

fn maybe_ref(by_ref: bool, ty: String) -> String {
    if by_ref { format!("&{ty}") } else { ty }
}

// ---- naming ----

fn rust_ident(java: &str) -> String {
    escape_rust_keyword(camel_to_snake_case(java))
}

/// `java/lang/String` (or `Ljava/lang/String;`) -> `java.lang.String`.
fn binary_to_fqn(binary: &str) -> String {
    binary.replace('/', ".").replace('$', ".")
}

/// `org/json/JSONObject` -> `org::json::JSONObject` (package as module path).
fn binary_to_rust_path(binary: &str) -> String {
    binary.replace('/', "::").replace('$', "_")
}

/// FQN `org.json.JSONObject` -> module path `org::json::JSONObject`.
fn binary_pkg_path(fqn: &str) -> String {
    fqn.replace('.', "::")
}

fn simple_rust_name(fqn: &str) -> String {
    fqn.rsplit('.').next().unwrap_or(fqn).to_string()
}

// ---- references / JDK ----

fn note_ref(referenced: &mut BTreeSet<String>, binary: &str) {
    referenced.insert(binary_to_fqn(binary));
}

fn note_field_ref(referenced: &mut BTreeSet<String>, fd: &FieldDescriptor) {
    if let FieldType::Object(cn) = &fd.field_type {
        referenced.insert(binary_to_fqn(cn));
    }
}

/// JDK types are always available (not something a user provides a JAR for).
fn is_jdk_fqn(fqn: &str) -> bool {
    fqn.starts_with("java.") || fqn.starts_with("javax.") || fqn.starts_with("jdk.") || fqn.starts_with("sun.")
}

// ---- annotations (nullability) ----

fn field_annotations<'a>(attrs: &'a [cafebabe::attributes::AttributeInfo<'a>]) -> Vec<&'a Annotation<'a>> {
    method_annotations(attrs)
}

fn method_annotations<'a>(attrs: &'a [cafebabe::attributes::AttributeInfo<'a>]) -> Vec<&'a Annotation<'a>> {
    let mut out = Vec::new();
    for a in attrs {
        match &a.data {
            AttributeData::RuntimeVisibleAnnotations(v)
            | AttributeData::RuntimeInvisibleAnnotations(v) => out.extend(v.iter()),
            _ => {}
        }
    }
    out
}

/// Per-parameter nullability (index matches the descriptor's parameter order).
fn parameter_annotations(attrs: &[cafebabe::attributes::AttributeInfo]) -> Vec<Option<bool>> {
    for a in attrs {
        match &a.data {
            AttributeData::RuntimeVisibleParameterAnnotations(v)
            | AttributeData::RuntimeInvisibleParameterAnnotations(v) => {
                return v
                    .iter()
                    .map(|pa| annotations_nullable(pa.annotations.iter().collect()))
                    .collect();
            }
            _ => {}
        }
    }
    Vec::new()
}

/// `Some(true)` if a `@Nullable`-style annotation is present, `Some(false)` for
/// an explicit `@NotNull`/`@Nonnull`, `None` if unannotated.
fn annotations_nullable(anns: Vec<&Annotation>) -> Option<bool> {
    for a in anns {
        if let FieldType::Object(cn) = &a.type_descriptor.field_type {
            let simple = cn.rsplit('/').next().unwrap_or(cn);
            let lower = simple.to_ascii_lowercase();
            if lower.contains("nullable") || lower.contains("checkfornull") {
                return Some(true);
            }
            if lower == "nonnull" || lower == "notnull" {
                return Some(false);
            }
        }
    }
    None
}
