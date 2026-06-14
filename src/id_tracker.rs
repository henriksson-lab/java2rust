//! Port of `IdTracker`, `Block`, and `IdTrackerVisitor`.

use std::collections::{HashMap, HashSet};

use crate::ast::{Arena, JClass, Node, NodeId, Pos};

/// Inspired by `de.aschoerk.java2rust.TypeDescription`, but `is_primitive` is
/// read directly from the AST type node rather than via JVM reflection — so the
/// borrow decision is consistent and independent of the classpath.
#[derive(Debug, Clone, Copy)]
pub struct TypeDescription {
    pub array_count: i32,
    pub clazz: JClass,
    pub is_primitive: bool,
}

/// Mirrors `de.aschoerk.java2rust.Import`.
#[derive(Debug, Clone)]
pub struct Import {
    pub import_string: String,
    pub static_import: bool,
    pub wildcard_import: bool,
}

pub const FICTIONAL_LINE_SIZE: i64 = 10_000_000;

/// Mirrors `de.aschoerk.java2rust.Block`.
#[derive(Debug)]
pub struct Block {
    pub parent_block: Option<usize>,
    pub children: Vec<usize>,
    pub node: NodeId,
    pub begin: Pos,
    pub end: Pos,
    pub changes: HashMap<String, Vec<NodeId>>,
    pub declarations: HashMap<String, (Option<TypeDescription>, NodeId)>,
    pub usages: HashMap<String, Vec<NodeId>>,
}

impl Block {
    fn new(node: NodeId, begin: Pos, end: Pos) -> Self {
        Block {
            parent_block: None,
            children: Vec::new(),
            node,
            begin,
            end,
            changes: HashMap::new(),
            declarations: HashMap::new(),
            usages: HashMap::new(),
        }
    }

    /// Mirrors `Block.contains(Node)` using begin/end positions.
    fn contains_pos(&self, b: Pos, e: Pos) -> bool {
        if b.line < self.begin.line {
            return false;
        }
        if b.line == self.begin.line && b.column < self.begin.column {
            return false;
        }
        if e.line > self.end.line {
            return false;
        }
        if e.line == self.end.line {
            return e.column <= self.end.column;
        }
        true
    }

    fn size(&self) -> i64 {
        (self.end.line as i64 - self.begin.line as i64) * FICTIONAL_LINE_SIZE
            + (self.end.column as i64 + FICTIONAL_LINE_SIZE - self.begin.column as i64)
    }
}

/// Mirrors `de.aschoerk.java2rust.IdTracker`.
#[derive(Debug, Default)]
pub struct IdTracker {
    pub try_count: i32,
    pub types: HashMap<NodeId, JClass>,
    pub package_name: Option<String>,
    pub has_throws: HashSet<String>,
    pub current_method: Option<String>,
    pub imports: Vec<Import>,
    pub blocks: Vec<Block>,
    pub current_blocks: Vec<usize>,
    pub in_constructor: bool,
}

impl IdTracker {
    pub fn new() -> Self {
        IdTracker::default()
    }

    // ---- throws / method ----

    pub fn has_throws_name(&self, name: &str) -> bool {
        self.has_throws.contains(name)
    }

    pub fn has_throws(&self) -> bool {
        match &self.current_method {
            Some(m) => self.has_throws.contains(m),
            None => false,
        }
    }

    pub fn set_current_method(&mut self, name: Option<String>) {
        self.current_method = name;
    }

    pub fn set_has_throws(&mut self, name: &str) {
        self.has_throws.insert(name.to_string());
    }

    // ---- in constructor ----

    pub fn set_in_constructor(&mut self, v: bool) {
        self.in_constructor = v;
    }

    pub fn is_in_constructor(&self) -> bool {
        self.in_constructor
    }

    // ---- try count ----

    pub fn increment_and_get_try_count(&mut self) -> i32 {
        self.try_count += 1;
        self.try_count
    }

    pub fn decrement_try_count(&mut self) {
        self.try_count -= 1;
    }

    // ---- blocks ----

    fn top(&self) -> Option<usize> {
        self.current_blocks.last().copied()
    }

    pub fn push_block(&mut self, arena: &Arena, n: NodeId) {
        let begin = arena.begin(n);
        let end = arena.end(n);
        let mut block = Block::new(n, begin, end);
        let idx = self.blocks.len();
        if let Some(parent) = self.top() {
            block.parent_block = Some(parent);
            self.blocks.push(block);
            self.blocks[parent].children.push(idx);
        } else {
            self.blocks.push(block);
        }
        self.current_blocks.push(idx);
    }

    pub fn pop_block(&mut self) {
        self.current_blocks.pop();
    }

    pub fn add_change(&mut self, name: &str, n: NodeId) {
        if let Some(b) = self.top() {
            self.blocks[b].changes.entry(name.to_string()).or_default().push(n);
        }
    }

    pub fn add_usage(&mut self, name: &str, n: NodeId) {
        if let Some(b) = self.top() {
            self.blocks[b].usages.entry(name.to_string()).or_default().push(n);
        }
    }

    pub fn add_declaration(&mut self, name: &str, descr: (Option<TypeDescription>, NodeId)) {
        if let Some(b) = self.top() {
            self.blocks[b].declarations.insert(name.to_string(), descr);
        }
    }

    fn find_inner_most_block(&self, arena: &Arena, n: NodeId) -> Option<usize> {
        let b = arena.begin(n);
        let e = arena.end(n);
        self.blocks
            .iter()
            .enumerate()
            .filter(|(_, blk)| blk.contains_pos(b, e))
            .min_by_key(|(_, blk)| blk.size())
            .map(|(i, _)| i)
    }

    pub fn find_declaration_node_for(
        &self,
        arena: &Arena,
        name: &str,
        n: NodeId,
    ) -> Option<(Option<TypeDescription>, NodeId)> {
        let mut block = self.find_inner_most_block(arena, n);
        loop {
            match block {
                Some(b) => {
                    if let Some(descr) = self.blocks[b].declarations.get(name) {
                        return Some(*descr);
                    }
                    block = self.blocks[b].parent_block;
                }
                None => return None,
            }
        }
    }

    fn is_changed_in_single_block(&self, name: &str, b: usize) -> bool {
        self.blocks[b].changes.contains_key(name)
    }

    fn is_declared_in_single_block(&self, name: &str, b: usize) -> bool {
        self.blocks[b].declarations.contains_key(name)
    }

    fn is_changed_in_children_of_block(&self, name: &str, bp: usize) -> bool {
        self.blocks[bp].children.iter().any(|&child| {
            !self.is_declared_in_single_block(name, child)
                && (self.is_changed_in_single_block(name, child)
                    || self.is_changed_in_children_of_block(name, child))
        })
    }

    pub fn is_changed(&self, arena: &Arena, name: &str, n: NodeId) -> bool {
        match self.find_inner_most_block(arena, n) {
            Some(b) => {
                self.is_changed_in_single_block(name, b)
                    || self.is_changed_in_children_of_block(name, b)
            }
            None => false,
        }
    }

    // ---- types ----

    pub fn put_type(&mut self, n: NodeId, clazz: JClass) {
        match self.get_type(n) {
            None => {
                self.types.insert(n, clazz);
            }
            Some(existing) => {
                if clazz.is_primitive()
                    && Self::is_discrete_class(existing)
                    && Self::is_float_class(clazz)
                {
                    self.types.insert(n, clazz);
                }
            }
        }
    }

    pub fn get_type(&self, n: NodeId) -> Option<JClass> {
        self.types.get(&n).copied()
    }

    pub fn is_float_class(clazz: JClass) -> bool {
        matches!(
            clazz,
            JClass::FloatType | JClass::DoubleType | JClass::FloatClass | JClass::DoubleClass
        )
    }

    pub fn is_discrete_class(clazz: JClass) -> bool {
        matches!(
            clazz,
            JClass::IntType
                | JClass::LongType
                | JClass::ByteType
                | JClass::ShortType
                | JClass::IntegerClass
                | JClass::LongClass
                | JClass::ByteClass
                | JClass::ShortClass
        )
    }

    pub fn is_float_node(&self, n: Option<NodeId>) -> bool {
        match n {
            Some(id) => self.get_type(id).map(Self::is_float_class).unwrap_or(false),
            None => false,
        }
    }
}

// ===================== IdTrackerVisitor =====================

/// Mirrors `IdTrackerVisitor.visit(CompilationUnit, IdTracker)`.
pub fn run(arena: &Arena, root: NodeId, t: &mut IdTracker) {
    let mut v = IdVisitor {
        arena,
        in_assign_target: false,
    };
    v.visit(root, t);
}

struct IdVisitor<'a> {
    arena: &'a Arena,
    in_assign_target: bool,
}

impl<'a> IdVisitor<'a> {
    /// Default traversal: visit each child (mirrors `super.visit`).
    fn visit_children(&mut self, id: NodeId, t: &mut IdTracker) {
        for c in self.arena.children(id) {
            self.visit(c, t);
        }
    }

    fn visit(&mut self, id: NodeId, t: &mut IdTracker) {
        use Node::*;
        match self.arena.kind(id).clone() {
            CompilationUnit {
                package,
                imports,
                types,
            } => {
                if let Some(p) = package {
                    if let Node::PackageDeclaration { name } = self.arena.kind(p) {
                        t.package_name = Some(self.qualified_name(*name));
                    }
                    self.visit(p, t);
                }
                for i in imports {
                    if let Node::ImportDeclaration {
                        name,
                        is_static,
                        is_asterisk,
                    } = self.arena.kind(i)
                    {
                        t.imports.push(Import {
                            import_string: self.qualified_name(*name),
                            static_import: *is_static,
                            wildcard_import: *is_asterisk,
                        });
                    }
                    self.visit(i, t);
                }
                for ty in types {
                    self.visit(ty, t);
                }
            }
            ClassOrInterfaceDeclaration { name, .. } => {
                t.push_block(self.arena, id);
                t.add_declaration(&name, (None, id));
                self.visit_children(id, t);
                t.pop_block();
            }
            EnumDeclaration { name, .. } => {
                t.push_block(self.arena, id);
                t.add_declaration(&name, (None, id));
                self.visit_children(id, t);
                t.pop_block();
            }
            AssignExpr { target, value, .. } => {
                self.in_assign_target = true;
                self.visit(target, t);
                self.in_assign_target = false;
                self.visit(value, t);
            }
            UnaryExpr { expr, op } => {
                use crate::ast::UnaryOp::*;
                self.in_assign_target = matches!(
                    op,
                    PosIncrement | PosDecrement | PreIncrement | PreDecrement
                );
                self.visit(expr, t);
                self.in_assign_target = false;
            }
            BlockStmt { .. } | ForStmt { .. } | ForeachStmt { .. } | CatchClause { .. } => {
                t.push_block(self.arena, id);
                self.visit_children(id, t);
                t.pop_block();
            }
            MethodCallExpr { scope, name, .. } => {
                t.add_usage(&name, id);
                // A mutating method call on a variable means it needs `let mut`.
                if let Some(s) = scope {
                    if let Node::NameExpr { name: recv } = self.arena.kind(s) {
                        if is_mutating_method(&name) {
                            let recv = recv.clone();
                            t.add_change(&recv, s);
                        }
                    }
                }
                self.visit_children(id, t);
            }
            MethodDeclaration { name, throws, .. } => {
                t.add_declaration(&name, (None, id));
                if !throws.is_empty() {
                    t.set_has_throws(&name);
                }
                t.push_block(self.arena, id);
                self.visit_children(id, t);
                t.pop_block();
            }
            ConstructorDeclaration { .. } => {
                t.push_block(self.arena, id);
                self.visit_children(id, t);
                t.pop_block();
            }
            NameExpr { name } => {
                if self.in_assign_target {
                    t.add_change(&name, id);
                } else {
                    t.add_usage(&name, id);
                }
            }
            // JavaParser models the field of a FieldAccessExpr as a child NameExpr,
            // so `this.x = ...` records a change to `x` (driving `let mut`).
            FieldAccessExpr { scope, field, .. } => {
                self.visit(scope, t);
                if self.in_assign_target {
                    t.add_change(&field, id);
                } else {
                    t.add_usage(&field, id);
                }
            }
            QualifiedNameExpr { name, .. } => {
                if self.in_assign_target {
                    t.add_change(&name, id);
                }
                self.visit_children(id, t);
            }
            VariableDeclarationExpr { typ, .. } => {
                if let Some(td) = self.type_description(t, typ) {
                    if IdTracker::is_float_class(td.clazz) {
                        t.put_type(id, td.clazz);
                        if td.array_count > 0 {
                            // child(1).child(1) is the initializer (JavaParser layout)
                            let kids = self.arena.children(id);
                            if let Some(&first_var) = kids.get(1) {
                                let vkids = self.arena.children(first_var);
                                if let Some(&init) = vkids.get(1) {
                                    if !matches!(self.arena.kind(init), Node::MethodCallExpr { .. }) {
                                        for child in self.arena.children(init) {
                                            t.put_type(child, JClass::DoubleType);
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                self.visit_children(id, t);
            }
            VariableDeclaratorId { name } => {
                let clazz = self.type_of_var_id(t, id);
                t.add_declaration(&name, (clazz, id));
                self.visit_children(id, t);
            }
            _ => self.visit_children(id, t),
        }
    }

    fn qualified_name(&self, id: NodeId) -> String {
        match self.arena.kind(id) {
            Node::NameExpr { name } => name.clone(),
            Node::QualifiedNameExpr { qualifier, name } => {
                format!("{}.{}", self.qualified_name(*qualifier), name)
            }
            _ => String::new(),
        }
    }

    // ---- type resolution (IdTrackerVisitor type helpers) ----

    fn name_of_type(&self, t: NodeId) -> Option<String> {
        match self.arena.kind(t) {
            Node::ReferenceType { typ, .. } => self.name_of_type(*typ),
            Node::ClassOrInterfaceType { name, .. } => Some(name.clone()),
            _ => None,
        }
    }

    fn type_description(&self, t: &IdTracker, typ: NodeId) -> Option<TypeDescription> {
        let name = self.name_of_type(typ);
        let clazz = identify_class(t, name.as_deref());
        match self.arena.kind(typ) {
            Node::ReferenceType { typ: inner, array_count } => {
                let inner_prim = potential_primitive(self.arena.kind(*inner));
                Some(TypeDescription {
                    array_count: *array_count,
                    clazz: clazz.or(inner_prim).unwrap_or(JClass::Other),
                    is_primitive: inner_prim.is_some(),
                })
            }
            Node::PrimitiveType { .. } => Some(TypeDescription {
                array_count: 0,
                clazz: potential_primitive(self.arena.kind(typ)).unwrap_or(JClass::Other),
                is_primitive: true,
            }),
            Node::ClassOrInterfaceType { .. } => Some(TypeDescription {
                array_count: 0,
                clazz: clazz.unwrap_or(JClass::Other),
                is_primitive: false,
            }),
            // VoidType / UnknownType / etc.: no useful description.
            _ => None,
        }
    }

    fn type_of_var_id(&self, t: &IdTracker, id: NodeId) -> Option<TypeDescription> {
        let parent = self.arena.parent(id)?;
        let grand = self.arena.parent(parent);
        let typ = match self.arena.kind(parent) {
            Node::Parameter { typ, .. } => *typ,
            _ => match grand.map(|g| self.arena.kind(g)) {
                Some(Node::FieldDeclaration { typ, .. }) => Some(*typ),
                Some(Node::VariableDeclarationExpr { typ, .. }) => Some(*typ),
                _ => None,
            },
        };
        typ.and_then(|t2| self.type_description(t, t2))
    }
}

/// Collection/StringBuilder methods that mutate their receiver (so the
/// receiver variable needs `let mut`).
pub fn is_mutating_method(name: &str) -> bool {
    matches!(
        name,
        "add" | "addAll" | "set" | "put" | "putAll" | "remove" | "removeAll" | "removeIf"
            | "clear" | "push" | "pop" | "insert" | "sort" | "append" | "addFirst" | "addLast"
    )
}

/// Mirrors `IdTrackerVisitor.getPotentialPrimitiveType`.
fn potential_primitive(kind: &Node) -> Option<JClass> {
    use crate::ast::PrimitiveKind::*;
    if let Node::PrimitiveType { kind } = kind {
        Some(match kind {
            Byte => JClass::ByteType,
            Short => JClass::ShortType,
            Int => JClass::IntType,
            Long => JClass::LongType,
            Float => JClass::FloatType,
            Double => JClass::DoubleType,
            Char => JClass::CharType,
            Boolean => JClass::BooleanType,
        })
    } else if let Node::VoidType = kind {
        Some(JClass::VoidType)
    } else {
        None
    }
}

/// Mirrors `IdTrackerVisitor.identifyaClass` (reflection over imports + java.lang).
fn identify_class(t: &IdTracker, name: Option<&str>) -> Option<JClass> {
    let name = name?;
    // imports
    for i in &t.imports {
        if !i.static_import {
            if i.wildcard_import {
                // A wildcard import only resolves a name if that class actually
                // exists in the package. We cannot reflect, so we resolve only
                // well-known JDK simple names — avoiding spuriously resolving
                // project types under `import java.util.*` etc.
                if wildcard_pkg_has(&i.import_string, name) {
                    return Some(JClass::Other);
                }
            } else if i.import_string.ends_with(&format!(".{name}")) {
                if let Some(c) = for_name(&i.import_string) {
                    return Some(c);
                }
            }
        }
    }
    if let Some(c) = for_name(&format!("java.lang.{name}")) {
        return Some(c);
    }
    if let Some(pkg) = &t.package_name {
        if let Some(c) = for_name(&format!("{pkg}.{name}")) {
            return Some(c);
        }
    }
    None
}

/// Emulates `Class.forName` for the classes the converter actually distinguishes.
fn for_name(fqn: &str) -> Option<JClass> {
    let simple = fqn.rsplit('.').next().unwrap_or(fqn);
    let java_lang = fqn
        .strip_prefix("java.lang.")
        .map_or(false, |rest| !rest.contains('.'));
    if java_lang {
        return match simple {
            "String" => Some(JClass::StringClass),
            "Double" => Some(JClass::DoubleClass),
            "Float" => Some(JClass::FloatClass),
            "Integer" => Some(JClass::IntegerClass),
            "Long" => Some(JClass::LongClass),
            "Short" => Some(JClass::ShortClass),
            "Byte" => Some(JClass::ByteClass),
            "Character" => Some(JClass::CharacterClass),
            "Boolean" => Some(JClass::BooleanClass),
            // Only real java.lang classes resolve via Class.forName("java.lang."+name).
            n if JAVA_LANG.contains(&n) => Some(JClass::Other),
            _ => None,
        };
    }
    // Other JDK packages are always on the classpath; approximate Class.forName
    // success for them (project classes like htsjdk.* are NOT on the converter's
    // classpath, so they fail to resolve — matching the jar).
    if fqn.starts_with("java.") || fqn.starts_with("javax.") {
        return Some(JClass::Other);
    }
    None
}

/// Does a wildcard import of `pkg` resolve the simple class `name`? We match the
/// name against the known classes of that specific package (a `java.util.*`
/// import must not resolve `Path`, which lives in `java.nio.file`).
fn wildcard_pkg_has(pkg: &str, name: &str) -> bool {
    match pkg {
        "java.util" => JAVA_UTIL.contains(&name),
        "java.io" => JAVA_IO.contains(&name),
        "java.nio.file" => JAVA_NIO_FILE.contains(&name),
        "java.nio" | "java.nio.channels" => {
            matches!(name, "ByteBuffer" | "CharBuffer" | "IntBuffer" | "LongBuffer" | "Buffer")
        }
        _ => false,
    }
}

/// Is `name` a simple name of a JDK class the tool knows about (java.lang /
/// java.util / java.io / java.nio.file)? Used to avoid stubbing stdlib types
/// that map to themselves (e.g. `String`, `Exception`).
pub fn is_known_jdk_type(name: &str) -> bool {
    JAVA_LANG.contains(&name)
        || JAVA_UTIL.contains(&name)
        || JAVA_IO.contains(&name)
        || JAVA_NIO_FILE.contains(&name)
}

const JAVA_UTIL: &[&str] = &[
    "List", "ArrayList", "LinkedList", "Map", "HashMap", "LinkedHashMap", "TreeMap", "SortedMap",
    "NavigableMap", "AbstractMap", "Set", "HashSet", "LinkedHashSet", "TreeSet", "SortedSet",
    "NavigableSet", "Collection", "AbstractCollection", "AbstractList", "Collections", "Arrays",
    "Iterator", "ListIterator", "Comparator", "Optional", "OptionalInt", "OptionalLong",
    "OptionalDouble", "Queue", "Deque", "ArrayDeque", "PriorityQueue", "Stack", "Vector",
    "Enumeration", "Properties", "Date", "Calendar", "GregorianCalendar", "TimeZone", "Locale",
    "Random", "UUID", "Objects", "Scanner", "StringTokenizer", "BitSet", "EnumSet", "EnumMap",
    "NoSuchElementException", "ConcurrentModificationException", "Spliterator", "Formatter",
    "IdentityHashMap", "WeakHashMap", "AbstractSet", "Map.Entry",
];

const JAVA_IO: &[&str] = &[
    "InputStream", "OutputStream", "Reader", "Writer", "BufferedReader", "BufferedWriter",
    "BufferedInputStream", "BufferedOutputStream", "ByteArrayInputStream", "ByteArrayOutputStream",
    "DataInputStream", "DataOutputStream", "DataInput", "DataOutput", "File", "FileInputStream",
    "FileOutputStream", "FileReader", "FileWriter", "InputStreamReader", "OutputStreamWriter",
    "PrintStream", "PrintWriter", "IOException", "FileNotFoundException", "UncheckedIOException",
    "Closeable", "Flushable", "Serializable", "Externalizable", "EOFException",
    "RandomAccessFile", "StringWriter", "StringReader", "FilterInputStream", "FilterOutputStream",
    "ObjectInputStream", "ObjectOutputStream", "InterruptedIOException", "PushbackInputStream",
];

const JAVA_NIO_FILE: &[&str] = &[
    "Path", "Paths", "Files", "FileSystem", "FileSystems", "OpenOption", "StandardOpenOption",
    "FileVisitor", "SimpleFileVisitor", "FileVisitResult", "DirectoryStream", "LinkOption",
    "CopyOption", "StandardCopyOption", "PathMatcher", "WatchService", "FileStore",
    "NoSuchFileException", "FileAlreadyExistsException",
];

/// Subset of public `java.lang` class simple names the corpus may reference.

/// Subset of public `java.lang` class simple names the corpus may reference.
const JAVA_LANG: &[&str] = &[
    // core
    "Object", "Class", "System", "Math", "StrictMath", "Number", "Enum", "Record", "Void",
    "Runtime", "Process", "ProcessBuilder", "Thread", "ThreadGroup", "ThreadLocal", "Package",
    "Compiler", "ClassLoader", "SecurityManager", "Runtime",
    // wrappers handled specially above are excluded here
    // char/string
    "CharSequence", "String", "StringBuilder", "StringBuffer", "Character", "Boolean",
    // interfaces
    "Runnable", "Comparable", "Iterable", "Cloneable", "AutoCloseable", "Appendable", "Readable",
    // throwables
    "Throwable", "Error", "Exception", "RuntimeException",
    "ArithmeticException", "ArrayIndexOutOfBoundsException", "ArrayStoreException",
    "ClassCastException", "ClassNotFoundException", "CloneNotSupportedException",
    "EnumConstantNotPresentException", "IllegalAccessException", "IllegalArgumentException",
    "IllegalMonitorStateException", "IllegalStateException", "IllegalThreadStateException",
    "IndexOutOfBoundsException", "InstantiationException", "InterruptedException",
    "NegativeArraySizeException", "NoSuchFieldException", "NoSuchMethodException",
    "NullPointerException", "NumberFormatException", "ReflectiveOperationException",
    "SecurityException", "StringIndexOutOfBoundsException", "TypeNotPresentException",
    "UnsupportedOperationException",
    "AbstractMethodError", "AssertionError", "BootstrapMethodError", "ClassCircularityError",
    "ClassFormatError", "ExceptionInInitializerError", "IllegalAccessError",
    "IncompatibleClassChangeError", "InstantiationError", "InternalError", "LinkageError",
    "NoClassDefFoundError", "NoSuchFieldError", "NoSuchMethodError", "OutOfMemoryError",
    "StackOverflowError", "ThreadDeath", "UnknownError", "UnsatisfiedLinkError",
    "UnsupportedClassVersionError", "VerifyError", "VirtualMachineError",
];
