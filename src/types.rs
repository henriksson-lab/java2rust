//! Unified type representation and resolver.
//!
//! A single source of truth for "what is the type of this expression / type
//! node", replacing the scattered, partial, on-demand type-derivers in the
//! dumper (`expr_num_type`, `recv_type_name`, `array_access_rust_type`,
//! `expr_is_char`, …). The dumper queries [`TypeResolver::type_of`] and reads
//! the answer through the [`Type`] query methods.
//!
//! Design notes:
//!   - [`Type`] carries a [`Type::Var`] variant for *unification variables* even
//!     though Tier 1 (ground resolution) never creates one — so the later
//!     constraint-solving tier is an extension, not a rewrite.
//!   - Nullability is **not** modelled here: `Type` is the underlying type, and
//!     `Option`-wrapping stays driven by the `nullability` pass. (`Type::Opt`
//!     exists only for an explicitly `Optional<T>`-typed source.)
//!   - Anything not determinable resolves to [`Type::Unknown`]; callers fall
//!     back to their existing heuristic rather than guessing.

use std::cell::RefCell;
use std::collections::HashMap;

use crate::ast::{Arena, BinaryOp, Node, NodeId, PrimitiveKind, UnaryOp};
use crate::id_tracker::IdTracker;
use crate::symbol_map::{LinkIndex, TypeSym};

/// A Rust primitive scalar.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Prim {
    I8,
    I16,
    I32,
    I64,
    Usize,
    F32,
    F64,
    Bool,
    Char,
}

/// The receiver category used by the stdlib method rewrites.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Category {
    String,
    List,
    Map,
    Set,
    Option,
}

/// A resolved type. `Unknown` is the bottom (un-inferrable / stub placeholder);
/// `Var` is a unification variable (Tier 2 only).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Type {
    Prim(Prim),
    Str,
    Vec(Box<Type>),
    Map(Box<Type>, Box<Type>),
    Set(Box<Type>),
    Opt(Box<Type>),
    /// A user/stub struct or enum, with any generic arguments.
    Named { path: String, args: Vec<Type> },
    /// A `Box<dyn Trait>` owned trait object.
    TraitObj(String),
    /// A type parameter (`T`, `O`, …).
    Param(String),
    /// A unification variable (unused until Tier 2).
    Var(u32),
    Unknown,
}

impl Type {
    /// The Rust scalar-numeric type string (`i32`/`f64`/…), or `None` if not a
    /// numeric scalar. Mirrors the old `expr_num_type` output.
    pub fn numeric_rust(&self) -> Option<&'static str> {
        match self {
            Type::Prim(p) => match p {
                Prim::I8 => Some("i8"),
                Prim::I16 => Some("i16"),
                Prim::I32 => Some("i32"),
                Prim::I64 => Some("i64"),
                Prim::F32 => Some("f32"),
                Prim::F64 => Some("f64"),
                // `usize`/`bool`/`char` are not numeric-promotable operands.
                Prim::Usize | Prim::Bool | Prim::Char => None,
            },
            _ => None,
        }
    }

    /// The receiver category for stdlib method rewrites (`recv_category`).
    pub fn category(&self) -> Option<Category> {
        match self {
            Type::Str => Some(Category::String),
            Type::Vec(_) => Some(Category::List),
            Type::Map(_, _) => Some(Category::Map),
            Type::Set(_) => Some(Category::Set),
            Type::Opt(_) => Some(Category::Option),
            _ => None,
        }
    }

    pub fn is_char(&self) -> bool {
        matches!(self, Type::Prim(Prim::Char))
    }

    /// A `Vec<char>` (a Java `char[]`).
    pub fn is_char_vec(&self) -> bool {
        matches!(self, Type::Vec(inner) if inner.is_char())
    }

    pub fn is_box_dyn(&self) -> bool {
        matches!(self, Type::TraitObj(_))
    }

    /// Numeric-promotion rank (`i8` < … < `i64` < `f32` < `f64`), or `None` if
    /// not a promotable numeric.
    fn numeric_rank(&self) -> Option<u8> {
        match self {
            Type::Prim(Prim::I8) => Some(1),
            Type::Prim(Prim::I16) => Some(2),
            Type::Prim(Prim::I32) => Some(3),
            Type::Prim(Prim::I64) => Some(4),
            Type::Prim(Prim::F32) => Some(5),
            Type::Prim(Prim::F64) => Some(6),
            _ => None,
        }
    }

    /// The `Map` value type (`V` of `Map<K,V>`), if this is a map.
    pub fn map_value(&self) -> Option<&Type> {
        match self {
            Type::Map(_, v) => Some(v),
            _ => None,
        }
    }

    /// The element type of a `Vec`/`Set`, if this is one.
    pub fn elem(&self) -> Option<&Type> {
        match self {
            Type::Vec(e) | Type::Set(e) => Some(e),
            _ => None,
        }
    }
}

/// A type's recorded return type for `name#arity` (falling back to the bare
/// `name`), if any.
fn lookup_method_ret(t: &TypeSym, name: &str, arity: usize) -> Option<String> {
    t.methods
        .get(&format!("{name}#{arity}"))
        .or_else(|| t.methods.get(name))
        .and_then(|m| m.ret.clone())
}

/// Java's promoted (wider) type for a numeric binary op; the numeric side when
/// only one is numeric; else `Unknown`.
fn promote(l: &Type, r: &Type) -> Type {
    // Both operands must be numeric to yield a numeric result — matching the
    // long-standing `expr_num_type` behaviour (a single unknown operand bails to
    // `Unknown`, so the surrounding code keeps its conservative coercion).
    match (l.numeric_rank(), r.numeric_rank()) {
        (Some(a), Some(b)) => {
            if a >= b {
                l.clone()
            } else {
                r.clone()
            }
        }
        _ => Type::Unknown,
    }
}

/// `Math` functions whose result is always `double` (excludes `abs`/`round`/
/// `min`/`max`/`signum`, whose return follows the argument type).
fn is_float_math(name: &str) -> bool {
    matches!(
        name,
        "sqrt" | "cbrt" | "log" | "log10" | "log1p" | "exp" | "expm1" | "pow" | "sin" | "cos"
            | "tan" | "asin" | "acos" | "atan" | "atan2" | "sinh" | "cosh" | "tanh" | "hypot"
            | "toRadians" | "toDegrees" | "ceil" | "floor" | "rint"
    )
}

/// Parse a *Java* type string (as recorded for a field: `Map<K, V>`,
/// `String[]`, `Double`, …) into a [`Type`], recovering element/value types.
pub fn parse_java_type(s: &str) -> Type {
    let s = s.trim();
    if let Some(inner) = s.strip_suffix("[]") {
        return Type::Vec(Box::new(parse_java_type(inner)));
    }
    if s.ends_with('>') {
        if let Some(lt) = s.find('<') {
            let base = s[..lt].trim();
            let args = split_top_level(&s[lt + 1..s.len() - 1]);
            let a = |i: usize| Box::new(parse_java_type(args.get(i).copied().unwrap_or("")));
            return match base {
                "Map" | "HashMap" | "LinkedHashMap" | "TreeMap" | "SortedMap" | "NavigableMap"
                | "ConcurrentHashMap" => Type::Map(a(0), a(1)),
                "List" | "ArrayList" | "LinkedList" | "Vector" | "Stack" | "Collection"
                | "Queue" | "Deque" | "ArrayDeque" | "Iterable" => Type::Vec(a(0)),
                "Set" | "HashSet" | "LinkedHashSet" | "TreeSet" | "SortedSet" | "NavigableSet" => {
                    Type::Set(a(0))
                }
                "Optional" => Type::Opt(a(0)),
                _ => Type::Named {
                    path: base.to_string(),
                    args: args.iter().map(|a| parse_java_type(a)).collect(),
                },
            };
        }
    }
    java_simple_to_type(s)
}

/// Map a Java *simple* type name (no generics) to a `Type` — used for a
/// cross-class field whose symbol-map entry stored only the simple name.
fn java_simple_to_type(simple: &str) -> Type {
    match simple {
        "String" | "CharSequence" | "StringBuilder" | "StringBuffer" => Type::Str,
        "List" | "ArrayList" | "LinkedList" | "Vector" | "Stack" | "Collection" | "Queue"
        | "Deque" | "ArrayDeque" | "Iterable" => Type::Vec(Box::new(Type::Unknown)),
        "Map" | "HashMap" | "LinkedHashMap" | "TreeMap" | "SortedMap" | "NavigableMap"
        | "ConcurrentHashMap" => Type::Map(Box::new(Type::Unknown), Box::new(Type::Unknown)),
        "Set" | "HashSet" | "LinkedHashSet" | "TreeSet" | "SortedSet" | "NavigableSet" => {
            Type::Set(Box::new(Type::Unknown))
        }
        "Optional" => Type::Opt(Box::new(Type::Unknown)),
        "Integer" => Type::Prim(Prim::I32),
        "Long" => Type::Prim(Prim::I64),
        "Short" => Type::Prim(Prim::I16),
        "Byte" => Type::Prim(Prim::I8),
        "Double" => Type::Prim(Prim::F64),
        "Float" => Type::Prim(Prim::F32),
        "Boolean" => Type::Prim(Prim::Bool),
        "Character" => Type::Prim(Prim::Char),
        "int" => Type::Prim(Prim::I32),
        "long" => Type::Prim(Prim::I64),
        "double" => Type::Prim(Prim::F64),
        "float" => Type::Prim(Prim::F32),
        "boolean" => Type::Prim(Prim::Bool),
        "char" => Type::Prim(Prim::Char),
        "" => Type::Unknown,
        _ => Type::Named { path: simple.to_string(), args: Vec::new() },
    }
}

/// Parse a *rendered Rust type string* (as stored in the symbol map's
/// `MethodSym::ret` / `ParamSym::rust_type`) back into a [`Type`]. Handles the
/// shapes the translator emits; anything else becomes `Named`/`Unknown`.
pub fn parse_rust_type(s: &str) -> Type {
    let s = s.trim();
    match s {
        "i8" => Type::Prim(Prim::I8),
        "i16" => Type::Prim(Prim::I16),
        "i32" => Type::Prim(Prim::I32),
        "i64" => Type::Prim(Prim::I64),
        "usize" => Type::Prim(Prim::Usize),
        "f32" => Type::Prim(Prim::F32),
        "f64" => Type::Prim(Prim::F64),
        "bool" => Type::Prim(Prim::Bool),
        "char" => Type::Prim(Prim::Char),
        "String" | "&str" | "str" | "&'static str" => Type::Str,
        "Unknown" | "" => Type::Unknown,
        _ => {
            let strip = |pfx: &str| s.strip_prefix(pfx).and_then(|x| x.strip_suffix('>'));
            if let Some(inner) = strip("Vec<") {
                Type::Vec(Box::new(parse_rust_type(inner)))
            } else if let Some(inner) = strip("Option<") {
                Type::Opt(Box::new(parse_rust_type(inner)))
            } else if let Some(inner) = strip("Box<") {
                if inner.trim_start().starts_with("dyn ") {
                    Type::TraitObj(inner.trim_start()[4..].trim().to_string())
                } else {
                    parse_rust_type(inner)
                }
            } else if let Some(inner) =
                strip("std::collections::HashMap<").or_else(|| strip("HashMap<"))
            {
                let parts = split_top_level(inner);
                Type::Map(
                    Box::new(parse_rust_type(parts.first().copied().unwrap_or(""))),
                    Box::new(parse_rust_type(parts.get(1).copied().unwrap_or(""))),
                )
            } else if let Some(inner) =
                strip("std::collections::HashSet<").or_else(|| strip("HashSet<"))
            {
                Type::Set(Box::new(parse_rust_type(inner)))
            } else {
                // A path (`crate::…::Name`) or bare name -> the simple name.
                let simple = s.split('<').next().unwrap_or(s);
                let simple = simple.rsplit("::").next().unwrap_or(simple).trim();
                Type::Named { path: simple.to_string(), args: Vec::new() }
            }
        }
    }
}

/// Split generic-argument text at top-level commas (ignoring nested `<…>`).
fn split_top_level(s: &str) -> Vec<&str> {
    let mut depth = 0i32;
    let mut start = 0usize;
    let mut out = Vec::new();
    for (i, c) in s.char_indices() {
        match c {
            '<' | '(' => depth += 1,
            '>' | ')' => depth -= 1,
            ',' if depth == 0 => {
                out.push(s[start..i].trim());
                start = i + 1;
            }
            _ => {}
        }
    }
    out.push(s[start..].trim());
    out
}

fn prim_of(kind: PrimitiveKind) -> Prim {
    match kind {
        PrimitiveKind::Boolean => Prim::Bool,
        PrimitiveKind::Char => Prim::Char,
        PrimitiveKind::Byte => Prim::I8,
        PrimitiveKind::Short => Prim::I16,
        PrimitiveKind::Int => Prim::I32,
        PrimitiveKind::Long => Prim::I64,
        PrimitiveKind::Float => Prim::F32,
        PrimitiveKind::Double => Prim::F64,
    }
}

pub struct TypeResolver<'a> {
    arena: &'a Arena,
    id: &'a IdTracker,
    link: &'a LinkIndex,
    /// FQN of the class being translated, for resolving `this.m()` / bare self
    /// method-call return types against the symbol map.
    current_class: Option<String>,
    memo: RefCell<HashMap<NodeId, Type>>,
}

impl<'a> TypeResolver<'a> {
    pub fn new(
        arena: &'a Arena,
        id: &'a IdTracker,
        link: &'a LinkIndex,
        current_class: Option<String>,
    ) -> Self {
        TypeResolver { arena, id, link, current_class, memo: RefCell::new(HashMap::new()) }
    }

    /// The type of a *type node* (`int`, `Map<K,V>`, `String[]`, `Foo<T>`).
    pub fn type_of_node(&self, typ: NodeId) -> Type {
        match self.arena.kind(typ) {
            Node::PrimitiveType { kind } => Type::Prim(prim_of(*kind)),
            Node::ReferenceType { typ, array_count } => {
                let inner = self.type_of_node(*typ);
                // Each array dimension wraps in a `Vec`.
                (0..*array_count).fold(inner, |t, _| Type::Vec(Box::new(t)))
            }
            Node::ClassOrInterfaceType { name, type_args, .. } => {
                let simple = name.rsplit('.').next().unwrap_or(name);
                let arg = |i: usize| {
                    type_args.get(i).map(|&a| self.type_of_node(a)).unwrap_or(Type::Unknown)
                };
                match simple {
                    "String" | "CharSequence" | "StringBuilder" | "StringBuffer" => Type::Str,
                    "List" | "ArrayList" | "LinkedList" | "Vector" | "Stack" | "Collection"
                    | "Queue" | "Deque" | "ArrayDeque" | "Iterable" => Type::Vec(Box::new(arg(0))),
                    "Map" | "HashMap" | "LinkedHashMap" | "TreeMap" | "SortedMap" | "NavigableMap"
                    | "ConcurrentHashMap" => Type::Map(Box::new(arg(0)), Box::new(arg(1))),
                    "Set" | "HashSet" | "LinkedHashSet" | "TreeSet" | "SortedSet"
                    | "NavigableSet" => Type::Set(Box::new(arg(0))),
                    "Optional" => Type::Opt(Box::new(arg(0))),
                    "Integer" => Type::Prim(Prim::I32),
                    "Long" => Type::Prim(Prim::I64),
                    "Short" => Type::Prim(Prim::I16),
                    "Byte" => Type::Prim(Prim::I8),
                    "Double" => Type::Prim(Prim::F64),
                    "Float" => Type::Prim(Prim::F32),
                    "Boolean" => Type::Prim(Prim::Bool),
                    "Character" => Type::Prim(Prim::Char),
                    _ => Type::Named {
                        path: simple.to_string(),
                        args: type_args.iter().map(|&a| self.type_of_node(a)).collect(),
                    },
                }
            }
            _ => Type::Unknown,
        }
    }

    /// The type of an *expression*. Phase 0 resolves the easy leaves (literals,
    /// names/fields via their declaration); everything else is `Unknown` and is
    /// filled in by later phases.
    pub fn type_of(&self, expr: NodeId) -> Type {
        if let Some(t) = self.memo.borrow().get(&expr) {
            return t.clone();
        }
        let t = self.compute_type_of(expr);
        self.memo.borrow_mut().insert(expr, t.clone());
        t
    }

    fn compute_type_of(&self, expr: NodeId) -> Type {
        match self.arena.kind(expr) {
            Node::IntegerLiteralExpr { .. } => Type::Prim(Prim::I32),
            Node::LongLiteralExpr { .. } => Type::Prim(Prim::I64),
            Node::DoubleLiteralExpr { .. } => Type::Prim(Prim::F64),
            Node::CharLiteralExpr { .. } => Type::Prim(Prim::Char),
            Node::BooleanLiteralExpr { .. } => Type::Prim(Prim::Bool),
            Node::StringLiteralExpr { .. } => Type::Str,
            Node::EnclosedExpr { inner: Some(i) } => self.type_of(*i),
            Node::CastExpr { typ, .. } => self.type_of_node(*typ),
            Node::UnaryExpr { expr, op } => match op {
                UnaryOp::Not => Type::Prim(Prim::Bool),
                _ => self.type_of(*expr),
            },
            Node::BinaryExpr { left, op, right } => self.binary_type(*left, *op, *right),
            Node::ConditionalExpr { then_expr, else_expr, .. } => {
                let t = self.type_of(*then_expr);
                if t == Type::Unknown { self.type_of(*else_expr) } else { t }
            }
            // Java array indexing `a[i]` -> the element type of `a`.
            Node::ArrayAccessExpr { name, .. } => {
                self.type_of(*name).elem().cloned().unwrap_or(Type::Unknown)
            }
            Node::ObjectCreationExpr { typ, .. } => self.type_of_node(*typ),
            Node::MethodCallExpr { scope, name, args, .. } => {
                self.method_call_type(*scope, name, args)
            }
            Node::NameExpr { name } => self
                .decl_type_node(name, expr)
                .map(|t| self.type_of_node(t))
                .unwrap_or(Type::Unknown),
            Node::FieldAccessExpr { scope, field, .. } => self
                .decl_type_node(field, expr)
                .map(|t| self.type_of_node(t))
                // A cross-class static field (`Other.field`): the symbol map
                // stores its Java simple type in `FieldSym::rust_type`.
                .unwrap_or_else(|| {
                    self.static_field_type(*scope, field).unwrap_or(Type::Unknown)
                }),
            _ => Type::Unknown,
        }
    }

    /// Result type of an arithmetic/comparison/string `+` binary expression.
    fn binary_type(&self, left: NodeId, op: BinaryOp, right: NodeId) -> Type {
        use BinaryOp::*;
        match op {
            Or | And | Equals | NotEquals | Less | Greater | LessEquals | GreaterEquals => {
                Type::Prim(Prim::Bool)
            }
            LShift | RSignedShift | RUnsignedShift => self.type_of(left),
            Plus | Minus | Times | Divide | Remainder | BinOr | BinAnd | Xor => {
                let (l, r) = (self.type_of(left), self.type_of(right));
                // Java string concatenation: `String + anything` is a `String`.
                if op == Plus && (l == Type::Str || r == Type::Str) {
                    return Type::Str;
                }
                promote(&l, &r)
            }
        }
    }

    /// Result type of `recv.name(args)` — the structural stdlib reach plus
    /// resolved project/linked method returns.
    fn method_call_type(&self, scope: Option<NodeId>, name: &str, args: &[NodeId]) -> Type {
        let recv = scope.map(|s| self.type_of(s));
        // Math.x(..) on inherently-float functions.
        if let Some(s) = scope {
            if matches!(self.arena.kind(s), Node::NameExpr { name } if name == "Math")
                && is_float_math(name)
            {
                return Type::Prim(Prim::F64);
            }
        }
        match (name, args.len()) {
            // Collection/Optional element access.
            ("get", 1) => match &recv {
                Some(Type::Map(_, v)) => return (**v).clone(),
                Some(Type::Vec(e)) | Some(Type::Set(e)) => return (**e).clone(),
                _ => {}
            },
            ("getOrDefault", 2) => {
                if let Some(Type::Map(_, v)) = &recv {
                    return (**v).clone();
                }
            }
            ("unwrap", 0) | ("orElse", 1) | ("orElseGet", 1) => match &recv {
                Some(Type::Opt(t)) => return (**t).clone(),
                Some(t) => return t.clone(),
                None => {}
            },
            // `Optional.get()` (0-arg) -> the wrapped type.
            ("get", 0) => {
                if let Some(Type::Opt(t)) = &recv {
                    return (**t).clone();
                }
            }
            // Identity-ish chain methods preserve the receiver type.
            ("clone", 0) | ("cloned", 0) | ("clone_java", 0) => {
                if let Some(t) = &recv {
                    return t.clone();
                }
            }
            // Boxed-number unboxing.
            ("doubleValue", 0) => return Type::Prim(Prim::F64),
            ("floatValue", 0) => return Type::Prim(Prim::F32),
            ("intValue", 0) => return Type::Prim(Prim::I32),
            ("longValue", 0) => return Type::Prim(Prim::I64),
            ("shortValue", 0) => return Type::Prim(Prim::I16),
            ("byteValue", 0) => return Type::Prim(Prim::I8),
            ("size", 0) | ("length", 0) => return Type::Prim(Prim::I32),
            ("charAt", 1) => return Type::Prim(Prim::Char),
            ("toCharArray", 0) => return Type::Vec(Box::new(Type::Prim(Prim::Char))),
            ("indexOf", _) | ("lastIndexOf", _) | ("compareTo", 1) | ("hashCode", 0) => {
                return Type::Prim(Prim::I32)
            }
            // String-returning String methods (on a String or unknown receiver).
            ("substring", _) | ("trim", 0) | ("toLowerCase", 0) | ("toUpperCase", 0)
            | ("replace", 2) | ("strip", 0) | ("concat", 1) | ("toString", 0)
                if matches!(recv, Some(Type::Str) | None) =>
            {
                return Type::Str
            }
            _ => {}
        }
        // A project/linked method: look up its recorded return type.
        if let Some(Type::Named { path, .. }) = &recv {
            if let Some(t) = self.resolve_type_sym(path) {
                if let Some(ret) = lookup_method_ret(t, name, args.len()) {
                    return parse_rust_type(&ret);
                }
            }
        }
        // A self-method call (`this.m()` or bare `m()`): resolve in the current
        // class and its ancestors (mirrors the dumper's `resolve_self_callee`).
        let is_self = match scope {
            None => true,
            Some(s) => matches!(self.arena.kind(s), Node::ThisExpr { .. }),
        };
        if is_self {
            if let Some(ret) = self.resolve_self_method_ret(name, args.len()) {
                return parse_rust_type(&ret);
            }
        }
        Type::Unknown
    }

    /// Return type of a self/inherited method `name#arity`, walking the current
    /// class's parent chain in the symbol map.
    fn resolve_self_method_ret(&self, name: &str, arity: usize) -> Option<String> {
        let mut t = self.link.lookup(self.current_class.as_deref()?);
        // Cycle guard: a loaded dependency map (which bypasses
        // `break_parent_cycles`) could still carry a looping `parent` chain.
        let mut seen: Vec<&str> = Vec::new();
        while let Some(ty) = t {
            if let Some(ret) = lookup_method_ret(ty, name, arity) {
                return Some(ret);
            }
            match ty.parent.as_deref() {
                Some(p) if !seen.contains(&p) => {
                    seen.push(p);
                    t = self.link.lookup(p);
                }
                _ => break,
            }
        }
        None
    }

    /// The type of a cross-class static field `Class.field` from the symbol map
    /// (its `FieldSym::rust_type` holds the field's Java simple type name).
    fn static_field_type(&self, scope: NodeId, field: &str) -> Option<Type> {
        let Node::NameExpr { name: cls } = self.arena.kind(scope) else {
            return None;
        };
        let t = self.resolve_type_sym(cls)?;
        let f = t.static_fields.get(field).or_else(|| t.fields.get(field))?;
        (!f.rust_type.is_empty()).then(|| parse_java_type(&f.rust_type))
    }

    /// Resolve a Java type name to its symbol-map entry (using this unit's
    /// imports + package), mirroring the dumper's resolution.
    fn resolve_type_sym(&self, name: &str) -> Option<&'a TypeSym> {
        if self.link.is_empty() {
            return None;
        }
        let mut explicit = Vec::new();
        let mut wildcard = Vec::new();
        for i in &self.id.imports {
            if i.static_import {
                continue;
            }
            if i.wildcard_import {
                wildcard.push(i.import_string.clone());
            } else {
                explicit.push(i.import_string.clone());
            }
        }
        self.link.resolve(name, &explicit, &wildcard, self.id.package_name.as_deref())
    }

    /// The declared type node of the variable/field/param a name resolves to.
    fn decl_type_node(&self, name: &str, at: NodeId) -> Option<NodeId> {
        let (_, decl) = self.id.find_declaration_node_for(self.arena, name, at)?;
        let parent = self.arena.parent(decl)?;
        let grand = self.arena.parent(parent);
        match self.arena.kind(parent) {
            Node::Parameter { typ, .. } => *typ,
            _ => match grand.map(|g| self.arena.kind(g)) {
                Some(Node::FieldDeclaration { typ, .. })
                | Some(Node::VariableDeclarationExpr { typ, .. }) => Some(*typ),
                _ => None,
            },
        }
    }
}
