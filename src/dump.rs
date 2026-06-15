//! Port of `RustDumpVisitor` and its `SourcePrinter`.

use crate::ast::{
    Arena, AssignOp, BinaryOp, JClass, Node, NodeId, PrimitiveKind, UnaryOp,
};
use crate::id_tracker::IdTracker;
use crate::modifiers;
use crate::naming::camel_to_snake_case;

/// The `arg: Object` threaded through JavaParser's visitor: absent, or a Type node.
pub type Arg = Option<NodeId>;

/// Port of `RustDumpVisitor.SourcePrinter`.
pub struct SourcePrinter {
    indentation: String,
    buf: String,
    level: usize,
    indented: bool,
    marks: Vec<usize>,
}

impl SourcePrinter {
    pub fn new(indentation: &str) -> Self {
        SourcePrinter {
            indentation: indentation.to_string(),
            buf: String::new(),
            level: 0,
            indented: false,
            marks: Vec::new(),
        }
    }
    pub fn indent(&mut self) {
        self.level += 1;
    }
    pub fn unindent(&mut self) {
        self.level -= 1;
    }
    fn make_indent(&mut self) {
        for _ in 0..self.level {
            self.buf.push_str(&self.indentation);
        }
    }
    pub fn print(&mut self, arg: &str) {
        if !self.indented {
            self.make_indent();
            self.indented = true;
        }
        self.buf.push_str(arg);
    }
    pub fn print_ln_s(&mut self, arg: &str) {
        self.print(arg);
        self.print_ln();
    }
    pub fn print_ln(&mut self) {
        self.buf.push('\n');
        self.indented = false;
    }
    pub fn get_source(&self) -> String {
        self.buf.clone()
    }
    pub fn push(&mut self) -> usize {
        self.marks.push(self.buf.len());
        self.marks.len()
    }
    pub fn get_mark(&self, mark: usize) -> String {
        self.buf[self.marks[mark - 1]..].to_string()
    }
    pub fn pop(&mut self) {
        let start = self.marks[self.marks.len() - 1];
        self.buf.truncate(start);
        self.marks.pop();
    }
    pub fn drop(&mut self) {
        self.marks.pop();
    }
}

/// Member filter used by `printMembers`.
#[derive(Clone, Copy)]
enum Filter {
    /// Methods/constructors/initializers — excludes fields and nested types.
    Method,
}

pub struct RustDumpVisitor<'a> {
    pub printer: SourcePrinter,
    arena: &'a Arena,
    id: &'a mut IdTracker,
    /// Mirrors the original's `commentOut` flag (always false in this build, so
    /// the `/* private */`-style emissions it guards never fire).
    #[allow(dead_code)]
    comment_out: bool,
    print_comments: bool,
    /// Instance field names (Java spelling) of the enclosing class, used to
    /// decide whether a method needs `&mut self`.
    class_field_names: std::collections::HashSet<String>,
    /// Declarations inferred nullable (emit `Option<T>`); see `nullability`.
    nullable: &'a std::collections::HashSet<NodeId>,
    /// When true, the value being emitted feeds an `Option<T>` slot, so a
    /// nullable read is kept as-is (no `.unwrap()`).
    expect_option: bool,
    /// When true, string literals are emitted raw (`"x"`, not `"x".to_string()`)
    /// — used for `match` patterns.
    raw_string: bool,
    /// Symbol maps of previously-translated dependencies; used to resolve
    /// referenced types to their real Rust paths.
    link: &'a crate::symbol_map::LinkIndex,
    /// Java names of the current method's parameters/locals used as receivers of
    /// a linked `&mut self` (refmut) call. Parameters get `&mut T`; locals get
    /// `let mut`.
    mut_borrow_params: std::collections::HashSet<String>,
    /// When true, unresolved external symbols are recorded into `stubs`.
    emit_stubs: bool,
    /// When true (crate mode), a resolved dependency path that isn't already
    /// crate-/std-relative is prefixed with `crate::` (the deps are generated as
    /// crate modules).
    crate_mode: bool,
    /// Transiently set by a parameter already emitting `&`: a trait type renders
    /// as the unsized `dyn Trait` (so the param is `&dyn Trait`) instead of the
    /// owned `Box<dyn Trait>` used in field/return/local positions.
    trait_dyn_ref: bool,
    /// Transiently set while printing a type-parameter bound (`T: Trait`): a
    /// trait there is a bound, emitted bare (no `dyn`/`Box`).
    trait_bound_pos: bool,
    /// True while emitting a `static` method body: a bare self-call there can't
    /// use `self` (it must be a static `Self::` call).
    in_static_method: bool,
    /// Crate path of the enum being matched in the current `switch`, if its
    /// variants cover the case labels — used to qualify bare labels as
    /// `Enum::Label` in match patterns.
    switch_enum_path: Option<String>,
    /// FQNs of types defined elsewhere in the same translated tree, so their
    /// cross-file references are not recorded as missing externals.
    known_types: Option<&'a std::collections::HashSet<String>>,
    /// Collected stub signatures for unresolved external symbols.
    stubs: std::cell::RefCell<crate::stubs::StubCollector>,
    /// True while emitting trait body items, where Rust forbids `pub`.
    in_trait: bool,
    /// FQN of the class currently being emitted, for inherited-member resolution
    /// against the linked project map.
    current_class_fqn: Option<String>,
    /// When emitting a non-static inner class, the outer class's FQN: outer
    /// instance members are reached via a synthesized `__outer` field
    /// (`self.__outer.borrow().<member>`), and the inner struct carries that
    /// field. `None` for top-level / static-nested classes.
    enclosing_class_fqn: Option<String>,
    /// The enclosing (outer) class's type-parameter nodes, so the inner can
    /// re-declare them and type its `__outer` field (`Rc<RefCell<Outer<…>>>`).
    enclosing_class_params: Vec<NodeId>,
    /// Names of the current class's non-static inner classes: at a `new Inner(…)`
    /// site the enclosing instance is threaded in as the synthesized `__outer`
    /// first argument.
    current_inner_classes: std::collections::HashSet<String>,
    /// Type-parameter nodes of the enclosing class(es), so a hoisted inner class
    /// can re-declare the outer params it references.
    enclosing_type_params: Vec<NodeId>,
    /// Counter for naming generated anonymous-class structs.
    anon_counter: u32,
    /// Java names of the enclosing locals/params captured by the anonymous class
    /// currently being emitted — references to them become `self.<field>`.
    anon_captures: std::collections::HashSet<String>,
    /// Java names of locals hoisted above the current `switch` (declared in one
    /// case, used in another — Java cases share a scope, Rust match arms don't):
    /// their in-case declaration becomes a plain assignment.
    hoisted_switch_vars: std::collections::HashSet<String>,
    /// If the current class extends an *external* (stub) type: its (FQN, Rust
    /// name), so a bare inherited-field read resolves to `self.base.<field>` and
    /// the field is recorded on the parent's stub.
    current_external_base: Option<(String, String)>,
    /// Names of the current `impl` block's type parameters, so a method that
    /// re-declares one (a Java static generic method shadowing the class param)
    /// drops it (Rust forbids the shadow — E0403).
    impl_param_names: Vec<String>,
    /// Overload disambiguation for the current type: method/constructor NodeId →
    /// its emitted Rust name (only present for members in an overloaded group).
    overload_name: std::collections::HashMap<NodeId, String>,
    /// Per base name, the overloaded members as (arity, emitted-name), so a
    /// self-call can pick the arity-matching overload.
    overload_by_arity: std::collections::HashMap<String, Vec<(usize, String)>>,
}

impl<'a> RustDumpVisitor<'a> {
    pub fn new(
        print_comments: bool,
        arena: &'a Arena,
        id: &'a mut IdTracker,
        nullable: &'a std::collections::HashSet<NodeId>,
        link: &'a crate::symbol_map::LinkIndex,
    ) -> Self {
        RustDumpVisitor {
            printer: SourcePrinter::new("    "),
            arena,
            id,
            comment_out: false,
            print_comments,
            class_field_names: std::collections::HashSet::new(),
            nullable,
            expect_option: false,
            raw_string: false,
            link,
            mut_borrow_params: std::collections::HashSet::new(),
            emit_stubs: false,
            crate_mode: false,
            trait_dyn_ref: false,
            trait_bound_pos: false,
            in_static_method: false,
            switch_enum_path: None,
            known_types: None,
            stubs: std::cell::RefCell::new(crate::stubs::StubCollector::default()),
            in_trait: false,
            current_class_fqn: None,
            enclosing_class_fqn: None,
            enclosing_class_params: Vec::new(),
            current_inner_classes: std::collections::HashSet::new(),
            enclosing_type_params: Vec::new(),
            anon_counter: 0,
            anon_captures: std::collections::HashSet::new(),
            hoisted_switch_vars: std::collections::HashSet::new(),
            impl_param_names: Vec::new(),
            current_external_base: None,
            overload_name: std::collections::HashMap::new(),
            overload_by_arity: std::collections::HashMap::new(),
        }
    }

    /// If `member` is an inherited instance field (not declared in the current
    /// class but in an ancestor), the access path through `base` fields, e.g.
    /// `self.base.base.rust_name`. Uses the linked project map's parent links.
    fn inherited_field(&self, member: &str) -> Option<String> {
        let mut t = self.link.lookup(self.current_class_fqn.as_deref()?)?;
        let mut bases = String::new();
        while let Some(parent) = t.parent.as_deref() {
            bases.push_str("base.");
            let pt = self.link.lookup(parent)?;
            if let Some(f) = pt.fields.get(member) {
                return Some(format!("{}.{bases}{}", self.self_receiver(), f.rust));
            }
            t = pt;
        }
        None
    }

    /// The crate path of the enum whose variant set contains every one of these
    /// `switch` case labels (bare names) — so a `match` on the enum can qualify
    /// them as `Enum::Label`. Matching the *whole* label set disambiguates even
    /// when individual variant names recur across enums.
    fn enum_path_for_labels(&self, labels: &[String]) -> Option<String> {
        if self.link.is_empty() || labels.is_empty() {
            return None;
        }
        for (_fqn, t) in self.link.iter() {
            if t.kind == "enum" && labels.iter().all(|l| t.static_fields.contains_key(l)) {
                return Some(self.crate_relativize(&t.rust_path));
            }
        }
        None
    }

    /// An inherited *static* constant (a `static final` in an ancestor): resolved
    /// as an associated const on the declaring type, `<ParentPath>::NAME`.
    fn inherited_static_const(&self, member: &str) -> Option<String> {
        let mut t = self.link.lookup(self.current_class_fqn.as_deref()?)?;
        while let Some(parent) = t.parent.as_deref() {
            let pt = self.link.lookup(parent)?;
            if let Some(f) = pt.static_fields.get(member) {
                return Some(format!("{}::{}", self.crate_relativize(&pt.rust_path), f.rust));
            }
            t = pt;
        }
        None
    }

    /// A bare name that's an instance field of the enclosing class or one of its
    /// ancestors (accessed from a non-static inner class) ->
    /// `self.__outer.borrow().[base.]*<field>`.
    fn enclosing_field(&self, name: &str) -> Option<String> {
        let mut t = self.link.lookup(self.enclosing_class_fqn.as_deref()?)?;
        let mut bases = String::new();
        loop {
            if let Some(f) = t.fields.get(name) {
                return Some(format!(
                    "{}.__outer.borrow().{bases}{}",
                    self.self_receiver(),
                    f.rust
                ));
            }
            let parent = t.parent.as_deref()?;
            t = self.link.lookup(parent)?;
            bases.push_str("base.");
        }
    }

    /// Java names of enclosing locals/params referenced inside an anonymous-class
    /// `body` but declared outside it (`anon_id`) — the free variables it must
    /// capture as fields.
    fn collect_anon_captures(&self, body: &[NodeId], anon_id: NodeId) -> Vec<String> {
        let mut out = Vec::new();
        let mut seen = std::collections::HashSet::new();
        let mut stack: Vec<NodeId> = body.to_vec();
        while let Some(n) = stack.pop() {
            if let Node::NameExpr { name } = self.arena.kind(n) {
                if !seen.contains(name) {
                    if let Some((_, decl)) = self.id.find_declaration_node_for(self.arena, name, n) {
                        if self.is_local_or_param(decl) && self.is_strict_descendant(decl, anon_id) == false {
                            seen.insert(name.clone());
                            out.push(name.clone());
                        }
                    }
                }
            }
            for c in self.arena.children(n) {
                stack.push(c);
            }
        }
        out
    }

    /// Is `decl` a method parameter or a local variable (not a field/type)?
    fn is_local_or_param(&self, decl: NodeId) -> bool {
        let Some(parent) = self.arena.parent(decl) else { return false };
        if matches!(self.arena.kind(parent), Node::Parameter { .. }) {
            return true;
        }
        if matches!(self.arena.kind(decl), Node::VariableDeclaratorId { .. }) {
            if let Some(g) = self.arena.parent(parent) {
                return matches!(self.arena.kind(g), Node::VariableDeclarationExpr { .. });
            }
        }
        false
    }

    /// Is `ancestor` a strict ancestor of `node`?
    fn is_strict_descendant(&self, mut node: NodeId, ancestor: NodeId) -> bool {
        while let Some(p) = self.arena.parent(node) {
            if p == ancestor {
                return true;
            }
            node = p;
        }
        false
    }

    /// The `Rc<RefCell<Outer<…>>>` type of a capturing inner class's `__outer`
    /// field/constructor param, if we're in such an inner class.
    fn enclosing_outer_type(&self) -> Option<String> {
        let t = self.link.lookup(self.enclosing_class_fqn.as_deref()?)?;
        let path = self.crate_relativize(&t.rust_path);
        let names: Vec<String> =
            self.enclosing_class_params.iter().filter_map(|&p| self.type_param_name(p)).collect();
        let args = if names.is_empty() { String::new() } else { format!("<{}>", names.join(", ")) };
        Some(format!("std::rc::Rc<std::cell::RefCell<{path}{args}>>"))
    }

    /// Is `name` an instance method of the enclosing class or an ancestor
    /// (reached from a non-static inner class via `self.__outer.borrow().m()`)?
    fn enclosing_method(&self, name: &str) -> bool {
        let mut t = match self.enclosing_class_fqn.as_deref().and_then(|f| self.link.lookup(f)) {
            Some(t) => t,
            None => return false,
        };
        loop {
            if t.methods.contains_key(name) {
                return true;
            }
            match t.parent.as_deref().and_then(|p| self.link.lookup(p)) {
                Some(p) => t = p,
                None => return false,
            }
        }
    }

    /// The receiver name for an instance member: `__self` in a constructor body
    /// (where `self` doesn't exist yet), `self` elsewhere.
    fn self_receiver(&self) -> &'static str {
        if self.id.is_in_constructor() {
            "__self"
        } else {
            "self"
        }
    }

    /// The Rust path of the current class's direct superclass, if known.
    fn current_parent_rust_path(&self) -> Option<String> {
        let t = self.link.lookup(self.current_class_fqn.as_deref()?)?;
        let p = t.parent.as_deref()?;
        Some(self.link.lookup(p)?.rust_path.clone())
    }

    /// Is `member` an inherited instance method (in an ancestor)? Such calls use
    /// `self.method()` and dispatch through `Deref`.
    fn inherited_method(&self, member: &str) -> bool {
        let Some(mut t) = self.current_class_fqn.as_deref().and_then(|f| self.link.lookup(f)) else {
            return false;
        };
        while let Some(parent) = t.parent.as_deref() {
            let Some(pt) = self.link.lookup(parent) else { return false };
            if pt.methods.contains_key(member) {
                return true;
            }
            t = pt;
        }
        false
    }

    /// Compute overload-disambiguated names for a type's members. Java permits
    /// same-name/different-param methods (and multiple constructors); Rust does
    /// not, so an overloaded group keeps its first member's name and suffixes the
    /// rest by arity (`foo`, `foo_2`, …). Self-calls resolve by arity; calls from
    /// other files fall back to the base (first) name, which always exists.
    fn compute_overloads(&mut self, members: &[NodeId]) {
        use std::collections::{HashMap, HashSet};
        let mut groups: HashMap<String, Vec<(NodeId, usize)>> = HashMap::new();
        for &m in members {
            match self.arena.kind(m) {
                Node::MethodDeclaration { name, parameters, .. } => {
                    let base = self.to_snake_if_necessary(name);
                    groups.entry(base).or_default().push((m, parameters.len()));
                }
                Node::ConstructorDeclaration { parameters, .. } => {
                    groups.entry("new".to_string()).or_default().push((m, parameters.len()));
                }
                _ => {}
            }
        }
        self.overload_name.clear();
        self.overload_by_arity.clear();
        for (base, list) in groups {
            if list.len() <= 1 {
                continue;
            }
            let mut used: HashSet<String> = HashSet::new();
            used.insert(base.clone());
            let mut by_arity: Vec<(usize, String)> = Vec::new();
            for (i, &(node, arity)) in list.iter().enumerate() {
                let mangled = if i == 0 {
                    base.clone()
                } else {
                    let mut cand = format!("{base}_{arity}");
                    let mut k = 2;
                    while used.contains(&cand) {
                        cand = format!("{base}_{arity}_{k}");
                        k += 1;
                    }
                    cand
                };
                used.insert(mangled.clone());
                self.overload_name.insert(node, mangled.clone());
                by_arity.push((arity, mangled));
            }
            self.overload_by_arity.insert(base, by_arity);
        }
    }

    /// The emitted Rust name for a method/constructor declaration node (its
    /// overload-mangled name, or the plain base name if not overloaded).
    fn decl_emitted_name(&self, node: NodeId, base: &str) -> String {
        self.overload_name.get(&node).cloned().unwrap_or_else(|| base.to_string())
    }

    /// The name to emit for a call: for a self-call to an overloaded method, the
    /// arity-matching overload; otherwise the base name.
    fn call_emitted_name(&self, scope: Option<NodeId>, name: &str, arity: usize) -> String {
        let base = self.to_snake_if_necessary(name);
        if scope.is_none() {
            if let Some(list) = self.overload_by_arity.get(&base) {
                if let Some((_, m)) = list.iter().find(|(a, _)| *a == arity) {
                    return m.clone();
                }
            }
        }
        base
    }

    /// Enable recording of unresolved external symbols into the stub collector,
    /// suppressing those defined elsewhere in the same tree (`known_types`).
    pub fn set_stub_collection(
        &mut self,
        emit_stubs: bool,
        known_types: &'a std::collections::HashSet<String>,
    ) {
        self.emit_stubs = emit_stubs;
        self.known_types = Some(known_types);
    }

    /// Enable crate mode: linked dependency paths are made `crate::`-relative
    /// (the deps are generated as crate modules).
    pub fn set_crate_mode(&mut self, crate_mode: bool) {
        self.crate_mode = crate_mode;
    }

    /// Take the collected stubs (call after `visit`).
    pub fn take_stubs(self) -> crate::stubs::StubCollector {
        self.stubs.into_inner()
    }

    /// Scan a method body for parameters/locals used as the receiver of a linked
    /// `refmut` call; their names need a `&mut` borrow in the caller's signature.
    fn collect_mut_borrow_params(&self, body: NodeId) -> std::collections::HashSet<String> {
        let mut out = std::collections::HashSet::new();
        if self.link.is_empty() {
            return out;
        }
        let mut stack = vec![body];
        while let Some(n) = stack.pop() {
            if let Node::MethodCallExpr { scope: Some(s), name, .. } = self.arena.kind(n) {
                if let Node::NameExpr { name: recv } = self.arena.kind(*s) {
                    if let Some(m) = self.resolve_linked_callee(Some(*s), name) {
                        if m.receiver == "refmut" {
                            out.insert(recv.clone());
                        }
                    }
                }
            }
            for c in self.arena.children(n) {
                stack.push(c);
            }
        }
        out
    }

    /// This file's non-static imports, split into (explicit `a.b.C`, wildcard
    /// packages `a.b`), for FQN reconstruction against the linked maps.
    fn link_candidates(&self) -> (Vec<String>, Vec<String>) {
        let mut explicit: Vec<String> = Vec::new();
        let mut wildcard: Vec<String> = Vec::new();
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
        (explicit, wildcard)
    }

    /// A bare name matching a static import resolves to the owning type's path
    /// (`Class::NAME`). Explicit (`import static a.b.C.NAME`) names the member
    /// directly; wildcard (`import static a.b.C.*`) requires the owner to
    /// actually declare a field `name` (so it can't swallow unrelated names).
    fn static_import_path(&self, name: &str) -> Option<String> {
        if self.link.is_empty() {
            return None;
        }
        let suffix = format!(".{name}");
        for imp in &self.id.imports {
            if !imp.static_import {
                continue;
            }
            if imp.wildcard_import {
                // A wildcard static import pulls in constants/static methods.
                // The owner declares few instance fields in the map, so trust it
                // for a constant-shaped name (all-caps), else require the field.
                let require = !is_const_name(name);
                if let Some(p) = self.static_member_path(&imp.import_string, name, require) {
                    return Some(p);
                }
            } else if let Some(owner_fqn) = imp.import_string.strip_suffix(&suffix) {
                if let Some(p) = self.static_member_path(owner_fqn, name, false) {
                    return Some(p);
                }
            }
        }
        None
    }

    /// Path to a static member `name` of the linked type `owner_fqn`, using the
    /// field's recorded Rust name when known. With `require_field`, returns
    /// `None` unless the type declares that field.
    fn static_member_path(&self, owner_fqn: &str, name: &str, require_field: bool) -> Option<String> {
        let t = self.link.lookup(owner_fqn)?;
        let path = self.crate_relativize(&t.rust_path);
        if let Some(f) = t.fields.get(name) {
            return Some(format!("{path}::{}", f.rust));
        }
        if require_field {
            return None;
        }
        Some(format!("{path}::{}", self.to_snake_if_necessary(name)))
    }

    /// Resolve a Java simple type name to a linked dependency type, if any.
    fn resolve_type_sym(&self, name: &str) -> Option<&'a crate::symbol_map::TypeSym> {
        if self.link.is_empty() {
            return None;
        }
        let (explicit, wildcard) = self.link_candidates();
        let link: &'a crate::symbol_map::LinkIndex = self.link;
        link.resolve(name, &explicit, &wildcard, self.id.package_name.as_deref())
    }

    /// Resolve a Java type name to the Rust type to emit. Consults the linked
    /// dependency maps first (using this file's imports + package to rebuild the
    /// FQN); falls back to the built-in stdlib mapping otherwise.
    fn resolve_type_name(&self, name: &str) -> String {
        // Resolve against the link map (project self-map + dependency maps), which
        // is import/package-driven, so a bare name can't shadow to an unrelated
        // dependency type (the unqualified fallback was removed).
        if let Some(t) = self.resolve_type_sym(name) {
            return self.crate_relativize(&t.rust_path);
        }
        let mapped = map_type_name(name).replace('$', "_");
        if let Some(key) = self.missing_type_key(name) {
            self.stubs.borrow_mut().note_type(&key, &mapped);
        }
        mapped
    }

    /// In crate mode, a linked *dependency* path (e.g. `org::json::JSONObject`,
    /// recovered from a jar) isn't crate- or std-relative, so a bare `org::…`
    /// reference won't resolve inside the assembled crate. The deps are emitted
    /// as crate modules (see `generate_dep_modules`), so prefix such paths with
    /// `crate::`. Project (`crate::…`) and stdlib (`std::`/`core::`/`alloc::`)
    /// paths are left untouched.
    fn crate_relativize(&self, path: &str) -> String {
        if !self.crate_mode || !path.contains("::") {
            return path.to_string();
        }
        let head = path.split("::").next().unwrap_or("");
        if matches!(head, "std" | "core" | "alloc") {
            return path.to_string();
        }
        // Escape path segments that are Rust keywords (Java package `impl`,
        // `type`, …) or contain `$` (synthetic/nested names) — segments that are
        // valid Java identifiers but invalid as a bare Rust path element.
        let escaped = sanitize_path_segments(path);
        if head == "crate" {
            escaped
        } else {
            format!("crate::{escaped}")
        }
    }

    // ---- stub collection (unresolved external symbols) ----

    /// Candidate FQNs for a referenced simple type name, via this file's imports
    /// and package (most-specific first).
    fn type_candidates(&self, name: &str) -> Vec<String> {
        let simple = name.rsplit('.').next().unwrap_or(name);
        let (explicit, wildcard) = self.link_candidates();
        let mut out = Vec::new();
        let suffix = format!(".{simple}");
        for imp in &explicit {
            if imp.ends_with(&suffix) {
                out.push(imp.clone());
            }
        }
        if let Some(pkg) = self.id.package_name.as_deref() {
            out.push(format!("{pkg}.{simple}"));
        }
        for pkg in &wildcard {
            out.push(format!("{pkg}.{simple}"));
        }
        out.push(name.to_string());
        out
    }

    fn is_known_project_type(&self, name: &str) -> bool {
        match self.known_types {
            Some(k) => {
                let simple = name.rsplit('.').next().unwrap_or(name);
                // The known set holds both FQNs and bare simple names, so a
                // nested/same-package reference by simple name resolves too.
                k.contains(simple) || self.type_candidates(name).iter().any(|c| k.contains(c))
            }
            None => false,
        }
    }

    /// If `name` references a type that cannot be resolved (not stdlib-mapped,
    /// not linked, not defined in this tree, not a primitive/type-parameter),
    /// return a best-effort FQN key to record a stub under. Returns `None` when
    /// stub collection is off or the type is resolvable.
    fn missing_type_key(&self, name: &str) -> Option<String> {
        if !self.emit_stubs {
            return None;
        }
        let simple = name.rsplit('.').next().unwrap_or(name);
        // Must look like a class name (CamelCase): excludes type parameters (`T`,
        // `E`) and all-caps acronyms we can't distinguish from generics.
        let first_upper = simple.chars().next().map(|c| c.is_uppercase()).unwrap_or(false);
        if !first_upper {
            return None;
        }
        // Short all-caps names are type parameters / acronym generics (`T`, `E`,
        // `K`, `V`, `T1`); longer all-caps names are real classes (`URL`, `URI`,
        // `CRC32`). Only exclude the short ones.
        if !simple.chars().any(|c| c.is_lowercase()) && simple.chars().count() <= 2 {
            return None;
        }
        if self.resolve_type_sym(simple).is_some() {
            return None; // linked
        }
        if map_type_name(simple) != simple {
            return None; // stdlib-mapped (List->Vec, Integer->i32, Exception->String, ...)
        }
        // Types valid in Rust as-is or handled elsewhere are not stubbed; every
        // other unmapped JDK/external class IS (so it — and its called methods —
        // resolves via a stub, e.g. `File`, `Path`, `InputStream`).
        if matches!(simple, "String" | "Object") {
            return None;
        }
        if self.is_known_project_type(simple) {
            return None; // defined elsewhere in this tree
        }
        let cands = self.type_candidates(simple);
        Some(cands.into_iter().find(|c| c.contains('.')).unwrap_or_else(|| simple.to_string()))
    }

    /// Resolve a Java type name to a Rust type for use in a stub signature,
    /// mapping the untranslatable `Object` to the `Unknown` placeholder.
    fn stub_type_name(&self, java: &str) -> String {
        let r = self.resolve_type_name(java);
        if r == "Object" { crate::stubs::UNKNOWN.into() } else { r }
    }

    /// Best-effort Rust type of an expression, for stub parameter inference.
    fn infer_expr_rust_type(&self, e: NodeId) -> String {
        match self.arena.kind(e) {
            Node::IntegerLiteralExpr { .. } => "i32".into(),
            Node::LongLiteralExpr { .. } => "i64".into(),
            Node::DoubleLiteralExpr { .. } => "f64".into(),
            Node::BooleanLiteralExpr { .. } => "bool".into(),
            Node::CharLiteralExpr { .. } => "char".into(),
            Node::StringLiteralExpr { .. } => "String".into(),
            Node::EnclosedExpr { inner: Some(i) } => self.infer_expr_rust_type(*i),
            Node::NameExpr { name } => self
                .decl_java_type_name(name, e)
                .map(|t| self.stub_type_name(&t))
                .unwrap_or_else(|| crate::stubs::UNKNOWN.into()),
            _ => crate::stubs::UNKNOWN.into(),
        }
    }

    /// Best-effort Rust return type of a call, inferred from its usage context
    /// (assigned into a typed local, or returned from the enclosing method).
    fn infer_call_ret_type(&self, call: NodeId) -> Option<String> {
        let p = self.arena.parent(call)?;
        match self.arena.kind(p) {
            Node::VariableDeclarator { init: Some(i), .. } if *i == call => {
                let gp = self.arena.parent(p)?;
                if let Node::VariableDeclarationExpr { typ, .. } = self.arena.kind(gp) {
                    return self.rust_type_of(*typ);
                }
                None
            }
            Node::ReturnStmt { expr: Some(e) } if *e == call => self.enclosing_method_ret_type(call),
            _ => None,
        }
    }

    fn enclosing_method_ret_type(&self, mut n: NodeId) -> Option<String> {
        while let Some(p) = self.arena.parent(n) {
            if let Node::MethodDeclaration { typ, .. } = self.arena.kind(p) {
                return self.rust_type_of(*typ);
            }
            n = p;
        }
        None
    }

    /// The Rust type string for a type node (primitives, mapped/linked class
    /// types, array element type), for stub return-type inference. `None` for
    /// `void`/unknown.
    fn rust_type_of(&self, typ: NodeId) -> Option<String> {
        match self.arena.kind(typ) {
            Node::PrimitiveType { kind } => Some(
                match kind {
                    PrimitiveKind::Boolean => "bool",
                    PrimitiveKind::Byte => "i8",
                    PrimitiveKind::Char => "char",
                    PrimitiveKind::Double => "f64",
                    PrimitiveKind::Float => "f32",
                    PrimitiveKind::Int => "i32",
                    PrimitiveKind::Long => "i64",
                    PrimitiveKind::Short => "i16",
                }
                .to_string(),
            ),
            Node::ClassOrInterfaceType { name, .. } => Some(self.stub_type_name(name)),
            Node::ReferenceType { typ, .. } => self.rust_type_of(*typ),
            _ => None,
        }
    }

    fn build_stub_sig(
        &self,
        args: &[NodeId],
        call: NodeId,
        receiver: crate::stubs::Receiver,
    ) -> crate::stubs::StubSig {
        crate::stubs::StubSig {
            receiver,
            params: args.iter().map(|&a| self.infer_expr_rust_type(a)).collect(),
            ret: self.infer_call_ret_type(call),
        }
    }

    /// Record an unresolved method / static / free-function call as a stub.
    fn record_missing_call(&self, scope: Option<NodeId>, name: &str, args: &[NodeId], id: NodeId) {
        if !self.emit_stubs {
            return;
        }
        match scope {
            Some(s) => {
                let Some(tname) = self.callee_recv_type(s) else { return };
                let Some(key) = self.missing_type_key(&tname) else { return };
                let is_static = self.is_static_class_ref(s);
                let rust_struct = map_type_name(&tname).replace('$', "_");
                let recv = if is_static {
                    crate::stubs::Receiver::None
                } else if crate::id_tracker::is_mutating_method(name) {
                    crate::stubs::Receiver::RefMut
                } else {
                    crate::stubs::Receiver::Ref
                };
                let sig = self.build_stub_sig(args, id, recv);
                let m = self.to_snake_if_necessary(name);
                self.stubs.borrow_mut().add_method(&key, &rust_struct, &m, sig, is_static);
            }
            None => {
                // A free function (static import / unresolved): only when it is
                // not a method of the current class.
                if self.id.find_declaration_node_for(self.arena, name, id).is_none() {
                    let sig = self.build_stub_sig(args, id, crate::stubs::Receiver::None);
                    let m = self.to_snake_if_necessary(name);
                    self.stubs.borrow_mut().add_free_fn(&m, sig);
                }
            }
        }
    }

    /// The simple Java type name of a method-call receiver, if it can be tied to
    /// a concrete type (a typed local/param/field, a `new X()`, or a static type
    /// reference). Used to look the call up in the linked maps.
    fn callee_recv_type(&self, scope: NodeId) -> Option<String> {
        match self.arena.kind(scope) {
            Node::NameExpr { name } => {
                if let Some(t) = self.decl_java_type_name(name, scope) {
                    Some(t)
                } else if self.is_static_class_ref(scope) {
                    // `Point.staticMethod()` — the name itself is the type.
                    Some(name.clone())
                } else {
                    None
                }
            }
            Node::ObjectCreationExpr { typ, .. } => self.type_simple_name(*typ),
            Node::EnclosedExpr { inner: Some(i) } => self.callee_recv_type(*i),
            _ => None,
        }
    }

    /// If a scoped call resolves to a method of a linked dependency type, return
    /// that method's recorded signature so the call site can be shaped to match
    /// (exact Rust name, argument borrowing, return nullability).
    fn resolve_linked_callee(
        &self,
        scope: Option<NodeId>,
        name: &str,
    ) -> Option<&'a crate::symbol_map::MethodSym> {
        if self.link.is_empty() {
            return None;
        }
        let simple = self.callee_recv_type(scope?)?;
        let t = self.resolve_type_sym(&simple)?;
        t.methods.get(name)
    }

    /// Emit a call's arguments shaped to a linked callee's parameter signature:
    /// `&`/`&mut` for by-reference params, `Some(..)` for nullable-by-value
    /// params, a clone for non-Copy by-value names. Args beyond the recorded
    /// params (e.g. varargs) fall back to the default argument emission.
    fn print_arguments_linked(
        &mut self,
        args: &[NodeId],
        params: &[crate::symbol_map::ParamSym],
        arg: Arg,
    ) {
        self.printer.print("(");
        for (i, &e) in args.iter().enumerate() {
            match params.get(i) {
                Some(p) if p.by_ref => {
                    self.printer.print(if p.mutable { "&mut " } else { "&" });
                    if p.nullable {
                        let saved = self.expect_option;
                        self.expect_option = true;
                        self.visit(e, arg);
                        self.expect_option = saved;
                    } else {
                        self.visit(e, arg);
                    }
                }
                Some(p) if p.nullable => self.emit_into_option(e, arg),
                Some(_) => self.emit_moved_value(e, arg),
                None => self.print_one_default_argument(e, arg),
            }
            if i + 1 < args.len() {
                self.printer.print(", ");
            }
        }
        self.printer.print(")");
    }

    /// The default per-argument emission (a `&` borrow for non-primitive names),
    /// factored out of [`print_arguments`] so it can be reused for trailing
    /// (e.g. varargs) arguments of a linked call.
    fn print_one_default_argument(&mut self, e: NodeId, arg: Arg) {
        if let Node::NameExpr { name } = self.arena.kind(e) {
            if let Some((Some(left), _)) = self.id.find_declaration_node_for(self.arena, name, e) {
                if !left.is_primitive || left.array_count > 0 {
                    self.printer.print("&");
                }
            }
        }
        self.visit(e, arg);
    }

    // ---- nullability helpers ----

    fn decl_nullable(&self, decl: NodeId) -> bool {
        self.nullable.contains(&decl)
    }

    fn name_decl_nullable(&self, name: &str, at: NodeId) -> bool {
        self.id
            .find_declaration_node_for(self.arena, name, at)
            .map(|(_, d)| self.nullable.contains(&d))
            .unwrap_or(false)
    }

    /// Mirror of `nullability::expr_nullable` for the dumper.
    fn expr_nullable(&self, e: NodeId) -> bool {
        match self.arena.kind(e) {
            Node::NullLiteralExpr => true,
            Node::NameExpr { name } => self.name_decl_nullable(name, e),
            Node::MethodCallExpr { scope: None, name, .. } => self.name_decl_nullable(name, e),
            Node::EnclosedExpr { inner: Some(i) } => self.expr_nullable(*i),
            Node::CastExpr { expr, .. } => self.expr_nullable(*expr),
            Node::ConditionalExpr { then_expr, else_expr, .. } => {
                self.expr_nullable(*then_expr) || self.expr_nullable(*else_expr)
            }
            _ => false,
        }
    }

    /// A name referring to a non-primitive (non-Copy) declaration — reading it
    /// by value out of a borrow needs `.clone()`.
    fn is_non_copy_name(&self, e: NodeId) -> bool {
        if let Node::NameExpr { name } = self.arena.kind(e) {
            if let Some((Some(td), _)) = self.id.find_declaration_node_for(self.arena, name, e) {
                return !td.is_primitive;
            }
        }
        false
    }

    /// A read of an instance field (`self.field` / `this.field` / a bare field
    /// name) — moving it out of `&self` needs `.clone()`. Cloning a Copy field is
    /// harmless, so we don't need the field's exact type here.
    fn is_field_read(&self, e: NodeId) -> bool {
        match self.arena.kind(e) {
            Node::FieldAccessExpr { scope, field, .. } => {
                matches!(self.arena.kind(*scope), Node::ThisExpr { .. })
                    || self.class_field_names.contains(field)
            }
            _ => false,
        }
    }

    /// Emit a value in a move position, cloning if it is a non-Copy name read or
    /// an instance-field read (both move out of a borrow otherwise).
    fn emit_moved_value(&mut self, e: NodeId, arg: Arg) {
        self.visit(e, arg);
        if self.is_non_copy_name(e) || self.is_field_read(e) {
            self.printer.print(".clone()");
        }
    }

    fn enclosing_method_nullable(&self, mut n: NodeId) -> bool {
        while let Some(p) = self.arena.parent(n) {
            if matches!(self.arena.kind(p), Node::MethodDeclaration { .. }) {
                return self.nullable.contains(&p);
            }
            n = p;
        }
        false
    }

    /// Emit `value` into an `Option<T>` slot: `None` / existing-Option as-is /
    /// `Some(value)` for a plain value.
    fn emit_into_option(&mut self, value: NodeId, arg: Arg) {
        if self.expr_nullable(value) {
            let saved = self.expect_option;
            self.expect_option = true;
            self.visit(value, arg);
            self.expect_option = saved;
        } else {
            self.printer.print("Some(");
            let saved = self.expect_option;
            self.expect_option = false;
            self.visit(value, arg);
            self.expect_option = saved;
            self.printer.print(")");
        }
    }

    pub fn get_source(&self) -> String {
        self.printer.get_source()
    }

    // ---- helpers ----

    fn kind(&self, id: NodeId) -> Node {
        self.arena.kind(id).clone()
    }

    fn is_type(&self, arg: Arg) -> bool {
        match arg {
            Some(a) => matches!(
                self.arena.kind(a),
                Node::PrimitiveType { .. }
                    | Node::ReferenceType { .. }
                    | Node::ClassOrInterfaceType { .. }
                    | Node::VoidType
                    | Node::WildcardType { .. }
                    | Node::UnknownType
                    | Node::IntersectionType { .. }
                    | Node::UnionType { .. }
            ),
            None => false,
        }
    }

    fn to_snake_if_necessary(&self, n: &str) -> String {
        let n = match n {
            "NaN" => "NAN",
            "NEGATIVE_INFINITY" => "NEG_INFINITY",
            "POSITIVE_INFINITY" => "INFINITY",
            "MIN_VALUE" => "MIN",
            "MAX_VALUE" => "MAX",
            other => other,
        };
        let first = n.chars().next().unwrap();
        let s = if first.is_lowercase() {
            camel_to_snake_case(n)
        } else {
            n.to_string()
        };
        // `$` (Scala/synthetic names) is illegal in Rust identifiers.
        escape_rust_keyword(s.replace('$', "_"))
    }

    fn remove_plus_and_suffix(&self, mut value: String, suffixes: &[&str]) -> String {
        if value.starts_with('+') {
            value = value[1..].to_string();
        }
        if value.starts_with('.') {
            value = format!("0{value}");
        }
        if suffixes.iter().any(|s| ends_with_ignore_none(&value, s)) {
            value = value[..value.len() - 1].to_string();
        }
        if value.ends_with('.') {
            value = format!("{value}0");
        }
        value = value.replace("d.", ".");
        value
    }

    // ---- printModifiers ----

    fn print_modifiers(&mut self, _m: i32) {
        // Rust forbids visibility qualifiers on trait items.
        if self.in_trait {
            return;
        }
        // Emit everything `pub`: Java's package-private default is more visible
        // than Rust's private, and once the tree is one crate (`--crate`),
        // cross-module references need the items to be visible (else E0603).
        self.printer.print("pub ");
    }

    fn print_members(&mut self, members: &[NodeId], arg: Arg, filter: Filter) {
        for &member in members {
            let keep = match filter {
                Filter::Method => !matches!(
                    self.arena.kind(member),
                    Node::FieldDeclaration { .. }
                        | Node::ClassOrInterfaceDeclaration { .. }
                        | Node::EnumDeclaration { .. }
                ),
            };
            if keep {
                self.printer.print_ln();
                self.visit(member, arg);
                self.printer.print_ln();
            }
        }
    }

    fn print_type_args(&mut self, args: &[NodeId], arg: Arg) {
        if !args.is_empty() {
            self.printer.print("<");
            for (i, &t) in args.iter().enumerate() {
                self.visit(t, arg);
                if i + 1 < args.len() {
                    self.printer.print(", ");
                }
            }
            self.printer.print(">");
        }
    }

    fn print_type_parameters(&mut self, args: &[NodeId], arg: Arg) {
        if !args.is_empty() {
            self.printer.print("<");
            for (i, &t) in args.iter().enumerate() {
                self.visit(t, arg);
                if i + 1 < args.len() {
                    self.printer.print(", ");
                }
            }
            self.printer.print(">");
        }
    }

    /// Print only the type-parameter *names* (no bounds): `<T, SOURCE>`. Used for
    /// the type path in an `impl<…> Type<…>` header, where bounds belong in the
    /// binder, not the path.
    fn print_type_param_names(&mut self, args: &[NodeId]) {
        if args.is_empty() {
            return;
        }
        self.printer.print("<");
        for (i, &t) in args.iter().enumerate() {
            if let Node::TypeParameter { name, .. } = self.arena.kind(t) {
                self.printer.print(name);
            }
            if i + 1 < args.len() {
                self.printer.print(", ");
            }
        }
        self.printer.print(">");
    }

    fn print_arguments(&mut self, args: &[NodeId], arg: Arg) {
        self.printer.print("(");
        for (i, &e) in args.iter().enumerate() {
            self.print_one_default_argument(e, arg);
            if i + 1 < args.len() {
                self.printer.print(", ");
            }
        }
        self.printer.print(")");
    }

    fn print_java_comment(&mut self, id: NodeId, arg: Arg) {
        if let Some(c) = self.arena.comment(id) {
            self.visit(c, arg);
        }
    }

    fn accept_and_cut(&mut self, n: NodeId, arg: Arg) -> String {
        let mark = self.printer.push();
        self.visit(n, arg);
        let result = self.printer.get_mark(mark);
        self.printer.pop();
        result
    }

    // ---- main dispatch ----

    pub fn visit(&mut self, id: NodeId, arg: Arg) {
        use Node::*;
        match self.kind(id) {
            CompilationUnit { .. } => self.visit_compilation_unit(id, arg),
            PackageDeclaration { name } => {
                self.print_java_comment(id, arg);
                self.printer.print("// package ");
                self.visit(name, arg);
                self.printer.print_ln_s(";");
                self.printer.print_ln();
                self.print_orphan_comments_ending(id);
            }
            NameExpr { name } => self.visit_name_expr(id, &name, arg),
            QualifiedNameExpr { qualifier, name } => {
                self.print_java_comment(id, arg);
                self.visit(qualifier, arg);
                self.printer.print("::");
                self.printer.print(&name);
                self.print_orphan_comments_ending(id);
            }
            ImportDeclaration {
                name,
                is_static,
                is_asterisk,
            } => {
                self.print_java_comment(id, arg);
                self.printer.print("use ");
                if is_static {
                    self.printer.print("/* static */");
                }
                self.visit(name, arg);
                if is_asterisk {
                    self.printer.print("::*");
                }
                self.printer.print_ln_s(";");
                self.print_orphan_comments_ending(id);
            }
            ClassOrInterfaceDeclaration { .. } => self.visit_class(id, arg),
            EmptyTypeDeclaration => {
                // A stray `;` (or an unsupported declaration like `@interface`,
                // `record`) is not a valid item; emit nothing.
                self.print_java_comment(id, arg);
                self.print_orphan_comments_ending(id);
            }
            JavadocComment { content } => {
                // Emit as a normal block comment, not `/**` — a Rust doc comment
                // requires an item to follow, which isn't guaranteed here.
                self.printer.print("/*");
                self.printer.print(&sanitize_block_comment(&content));
                self.printer.print_ln_s("*/");
            }
            ClassOrInterfaceType { .. } => self.visit_class_type(id, arg),
            TypeParameter { name, type_bound } => {
                self.print_java_comment(id, arg);
                self.printer.print(&name);
                // Rust type-parameter bounds must be traits. Java allows a class
                // bound (`T extends ConcreteClass`); drop any bound that resolves
                // to a known non-trait (struct/enum/stub), which has no Rust
                // equivalent and would be `expected trait, found struct`.
                let kept: Vec<NodeId> = type_bound
                    .iter()
                    .copied()
                    .filter(|&c| {
                        !self.bound_is_known_non_trait(c)
                            && !self.bound_has_bare_wildcard(c)
                            && !self.bound_is_std_concrete(c)
                    })
                    .collect();
                if !kept.is_empty() {
                    self.printer.print(": ");
                    for (i, &c) in kept.iter().enumerate() {
                        self.trait_bound_pos = true;
                        self.visit(c, arg);
                        if i + 1 < kept.len() {
                            self.printer.print(" + ");
                        }
                    }
                }
            }
            PrimitiveType { kind } => {
                self.print_java_comment(id, arg);
                self.printer.print(match kind {
                    PrimitiveKind::Boolean => "bool",
                    PrimitiveKind::Byte => "i8",
                    PrimitiveKind::Char => "char",
                    PrimitiveKind::Double => "f64",
                    PrimitiveKind::Float => "f32",
                    PrimitiveKind::Int => "i32",
                    PrimitiveKind::Long => "i64",
                    PrimitiveKind::Short => "i16",
                });
            }
            ReferenceType { typ, array_count } => {
                self.print_java_comment(id, arg);
                for _ in 0..array_count {
                    self.printer.print("Vec<");
                }
                self.visit(typ, arg);
                for _ in 0..array_count {
                    self.printer.print(">");
                }
            }
            IntersectionType { elements } => {
                self.print_java_comment(id, arg);
                let mut first = true;
                for e in elements {
                    self.visit(e, arg);
                    if first {
                        first = false;
                    } else {
                        self.printer.print(" & ");
                    }
                }
            }
            UnionType { elements } => {
                self.print_java_comment(id, arg);
                let mut first = true;
                for e in elements {
                    self.visit(e, arg);
                    if first {
                        first = false;
                    } else {
                        self.printer.print(" | ");
                    }
                }
            }
            WildcardType { ext, sup } => {
                // `? extends T` / `? super T` -> the bound type; bare `?` -> `_`.
                self.print_java_comment(id, arg);
                match ext.or(sup) {
                    Some(b) => self.visit(b, arg),
                    None => self.printer.print("_"),
                }
            }
            UnknownType => {}
            VoidType => {
                self.print_java_comment(id, arg);
                self.printer.print("void");
            }
            FieldDeclaration { typ, variables, .. } => {
                self.print_orphan_comments_before_this_child_node(id);
                self.print_java_comment(id, arg);
                self.printer.print(" ");
                for (i, &var) in variables.iter().enumerate() {
                    self.visit(var, Some(typ));
                    if i + 1 < variables.len() {
                        self.printer.print(", ");
                    }
                }
                self.printer.print(";");
            }
            VariableDeclarator { .. } => self.visit_variable_declarator(id, arg),
            VariableDeclaratorId { name } => {
                self.print_java_comment(id, arg);
                let s = self.to_snake_if_necessary(&name);
                self.printer.print(&s);
            }
            ArrayInitializerExpr { .. } => self.visit_array_initializer(id, arg),
            ArrayCreationExpr { .. } => self.visit_array_creation(id, arg),
            ArrayAccessExpr { name, index } => {
                self.print_java_comment(id, arg);
                self.visit(name, arg);
                // Rust indices are usize; Java's are int.
                self.printer.print("[(");
                self.visit(index, arg);
                self.printer.print(") as usize]");
            }
            AssignExpr { .. } => self.visit_assign(id, arg),
            BinaryExpr { .. } => self.visit_binary(id, arg),
            CastExpr { typ, expr } => {
                self.print_java_comment(id, arg);
                // Parenthesize: an unparenthesized cast can't be the operand of a
                // shift (`x as i64 << 31` parses `i64<…>`) or a method receiver
                // (`x as T.m()`), both hard parse errors.
                self.printer.print("(");
                self.visit(expr, arg);
                self.printer.print(" as ");
                self.visit(typ, arg);
                self.printer.print(")");
            }
            ClassExpr { typ } => {
                self.print_java_comment(id, arg);
                self.printer.print("std::any::TypeId::of::<");
                self.visit(typ, arg);
                self.printer.print(">()");
            }
            ConditionalExpr {
                condition,
                then_expr,
                else_expr,
            } => {
                self.print_java_comment(id, arg);
                self.printer.print(" if ");
                self.visit(condition, arg);
                self.printer.print(" { ");
                self.visit(then_expr, arg);
                self.printer.print(" } else { ");
                self.visit(else_expr, arg);
                self.printer.print(" }");
            }
            EnclosedExpr { inner } => {
                self.print_java_comment(id, arg);
                self.printer.print("(");
                if let Some(i) = inner {
                    self.visit(i, arg);
                }
                self.printer.print(")");
            }
            FieldAccessExpr { .. } => self.visit_field_access(id, arg),
            InstanceOfExpr { typ, .. } => {
                // Rust has no general runtime type test; emit a compiling stub.
                self.print_java_comment(id, arg);
                self.printer.print("false /* instanceof ");
                let t = self.accept_and_cut(typ, None);
                self.printer.print(t.trim());
                self.printer.print(" */");
            }
            CharLiteralExpr { value } => {
                self.print_java_comment(id, arg);
                self.printer.print("'");
                self.printer.print(&java_escapes_to_rust(&value));
                self.printer.print("'");
            }
            DoubleLiteralExpr { value } => {
                self.print_java_comment(id, arg);
                // Rust has no hex float literals; compute the value as decimal.
                if let Some(dec) = hex_float_to_decimal(&value) {
                    self.printer.print(&dec);
                } else {
                    // Strip the Java float/double suffix first (Rust infers the
                    // type; `f`/`F`/`d`/`D` are not valid Rust literal suffixes),
                    // then ensure a decimal point so it's a float literal.
                    let mut value = self.remove_plus_and_suffix(value, &["D", "d", "F", "f"]);
                    if !value.contains(['.', 'e', 'E', 'x', 'X']) {
                        value = format!("{value}.0");
                    }
                    self.printer.print(&value);
                }
            }
            IntegerLiteralExpr { value } => {
                self.print_java_comment(id, arg);
                let output = self.remove_plus_and_suffix(value, &[]);
                if self.is_float_in_history(Some(id)) {
                    self.printer.print(&format!("{output}.0"));
                } else {
                    self.printer.print(&output);
                }
            }
            LongLiteralExpr { value } => {
                self.print_java_comment(id, arg);
                let s = self.remove_plus_and_suffix(value, &["l", "L"]);
                self.printer.print(&s);
            }
            IntegerLiteralMinValueExpr { value } | LongLiteralMinValueExpr { value } => {
                self.print_java_comment(id, arg);
                self.printer.print(&value);
            }
            StringLiteralExpr { value } => {
                self.print_java_comment(id, arg);
                // Java String is owned, so emit `"...".to_string()` so String-typed
                // bindings type-check — except in a match pattern, where a raw
                // `"..."` (str literal) is required. (Concatenation uses format!.)
                self.printer.print("\"");
                self.printer.print(&java_escapes_to_rust(&value));
                self.printer
                    .print(if self.raw_string { "\"" } else { "\".to_string()" });
            }
            BooleanLiteralExpr { value } => {
                self.print_java_comment(id, arg);
                self.printer.print(if value { "true" } else { "false" });
            }
            NullLiteralExpr => {
                self.print_java_comment(id, arg);
                self.printer.print("None");
            }
            ThisExpr { class_expr } => {
                self.print_java_comment(id, arg);
                if let Some(ce) = class_expr {
                    self.visit(ce, arg);
                } else if self.id.is_in_constructor() {
                    self.printer.print("__self");
                } else {
                    self.printer.print("self");
                }
            }
            SuperExpr { class_expr } => {
                // `super` refers to the embedded base (`super.m()` -> `self.base.m()`).
                self.print_java_comment(id, arg);
                let _ = class_expr;
                self.printer
                    .print(if self.id.is_in_constructor() { "__self.base" } else { "self.base" });
            }
            MethodCallExpr { .. } => self.visit_method_call(id, arg),
            ObjectCreationExpr { .. } => self.visit_object_creation(id, arg),
            UnaryExpr { .. } => self.visit_unary(id, arg),
            ConstructorDeclaration { .. } => self.visit_constructor(id, arg),
            MethodDeclaration { .. } => self.visit_method(id, arg),
            Parameter { .. } => self.visit_parameter(id, arg),
            MultiTypeParameter { modifiers: _, typ, id: vid } => {
                self.visit(typ, arg);
                self.printer.print(" ");
                self.visit(vid, arg);
            }
            ExplicitConstructorInvocationStmt { is_this, args, .. } => {
                self.print_java_comment(id, arg);
                let (is_this, args) = (is_this, args.clone());
                if is_this {
                    // Delegating constructor `this(args)` -> rebuild via the
                    // matching `Self::new*`.
                    let nm = self.call_emitted_name(None, "new", args.len());
                    self.printer.print(&format!("*__self = Self::{nm}"));
                    self.print_arguments(&args, arg);
                    self.printer.print(";");
                } else if let Some(p) = self.current_parent_rust_path() {
                    // `super(args)` -> initialise the embedded base.
                    self.printer.print(&format!("__self.base = {p}::new"));
                    self.print_arguments(&args, arg);
                    self.printer.print(";");
                } else {
                    self.printer.print("/* super(...) omitted */");
                }
            }
            VariableDeclarationExpr { modifiers: _, typ, vars } => {
                self.print_java_comment(id, arg);
                self.printer.print(" ");
                // `int i = 0, j = 1;` -> separate `let` statements (`; `-joined).
                for (i, &v) in vars.iter().enumerate() {
                    self.visit(v, Some(typ));
                    if i + 1 < vars.len() {
                        self.printer.print("; ");
                    }
                }
            }
            TypeDeclarationStmt { type_declaration } => {
                self.print_java_comment(id, arg);
                self.visit(type_declaration, arg);
            }
            AssertStmt { check, message } => {
                self.print_java_comment(id, arg);
                self.printer.print("assert!(");
                self.visit(check, arg);
                if let Some(msg) = message {
                    self.printer.print(", \"{}\", ");
                    self.visit(msg, arg);
                }
                self.printer.print(");");
            }
            BlockStmt { .. } => self.visit_block(id, arg),
            LabeledStmt { label, stmt } => {
                self.print_java_comment(id, arg);
                self.printer.print("'");
                self.printer.print(&label);
                self.printer.print(": ");
                self.visit(stmt, arg);
            }
            EmptyStmt => {
                self.print_java_comment(id, arg);
                self.printer.print(";");
            }
            ExpressionStmt { expression } => {
                self.print_orphan_comments_before_this_child_node(id);
                self.print_java_comment(id, arg);
                self.visit(expression, arg);
                self.printer.print(";");
            }
            SwitchStmt { .. } => self.visit_switch(id, arg),
            SwitchEntryStmt { .. } => {} // handled inline by visit_switch
            BreakStmt { id: lbl } => {
                self.print_java_comment(id, arg);
                self.printer.print("break");
                if let Some(l) = lbl {
                    self.printer.print(" '");
                    self.printer.print(&l);
                }
                self.printer.print(";");
            }
            ReturnStmt { expr } => {
                self.print_java_comment(id, arg);
                self.printer.print("return");
                if let Some(e) = expr {
                    self.printer.print(" ");
                    let throws = self.id.has_throws();
                    if throws {
                        self.printer.print("Ok(");
                    }
                    if self.enclosing_method_nullable(id) {
                        self.emit_into_option(e, arg);
                    } else {
                        self.emit_moved_value(e, arg);
                    }
                    if throws {
                        self.printer.print(")");
                    }
                }
                self.printer.print(";");
            }
            EnumDeclaration { .. } => self.visit_enum(id, arg),
            EnumConstantDeclaration { .. } => self.visit_enum_constant(id, arg),
            EmptyMemberDeclaration => {
                // A stray `;` is not a valid Rust item; emit nothing.
                self.print_java_comment(id, arg);
            }
            InitializerDeclaration { .. } => {
                // Static/instance initializer blocks have no item-level Rust form.
                self.print_java_comment(id, arg);
                self.printer.print("// initializer block omitted");
            }
            IfStmt { .. } => self.visit_if(id, arg),
            WhileStmt { condition, body } => {
                self.print_java_comment(id, arg);
                self.printer.print("while ");
                self.visit(condition, arg);
                self.printer.print(" ");
                self.encapsulate_if_not_block(body, arg);
            }
            ContinueStmt { id: lbl } => {
                self.print_java_comment(id, arg);
                self.printer.print("continue");
                if let Some(l) = lbl {
                    self.printer.print(" '");
                    self.printer.print(&l);
                }
                self.printer.print(";");
            }
            DoStmt { body, condition } => {
                self.print_java_comment(id, arg);
                self.printer.print("loop { ");
                self.visit(body, arg);
                self.printer.print(" if !(");
                self.visit(condition, arg);
                self.printer.print(") { break; } }");
            }
            ForeachStmt { variable, iterable, body } => {
                self.print_java_comment(id, arg);
                self.printer.print("for ");
                // Bind the loop variable by value (clone the iterable) to match
                // Java's value semantics and keep the body's uses simple.
                let vname = self.foreach_var_name(variable);
                self.printer.print(&vname);
                self.printer.print(" in ");
                self.visit(iterable, arg);
                self.printer.print(".clone() ");
                self.encapsulate_if_not_block(body, arg);
            }
            ForStmt { .. } => self.visit_for(id, arg),
            ThrowStmt { expr } => {
                self.print_java_comment(id, arg);
                // `throw new SomeException(msg)` -> `panic!("{:?}", msg)`. Using the
                // constructor argument (not the exception type) keeps it compiling
                // even though the exception type isn't defined.
                self.printer.print("panic!(\"{:?}\", ");
                match self.arena.kind(expr) {
                    Node::ObjectCreationExpr { args, .. } if !args.is_empty() => {
                        let first = args[0];
                        self.visit(first, arg);
                    }
                    Node::ObjectCreationExpr { .. } => self.printer.print("\"exception\""),
                    _ => self.visit(expr, arg),
                }
                self.printer.print(");");
            }
            SynchronizedStmt { block, .. } => {
                // No direct Rust equivalent; run the body (lock dropped).
                self.print_java_comment(id, arg);
                self.printer.print("/* synchronized */ ");
                self.visit(block, arg);
            }
            TryStmt { .. } => self.visit_try(id, arg),
            CatchClause { param, catch_block } => {
                self.print_java_comment(id, arg);
                self.printer.print(" catch (");
                self.visit(param, arg);
                self.printer.print(") ");
                self.visit(catch_block, arg);
            }
            MemberValuePair { name, value } => {
                self.print_java_comment(id, arg);
                self.printer.print(&name);
                self.printer.print(" = ");
                self.visit(value, arg);
            }
            LineComment { content } => {
                if self.print_comments {
                    self.printer.print("//");
                    let tmp = content.replace('\r', " ").replace('\n', " ");
                    self.printer.print_ln_s(&tmp);
                }
            }
            BlockComment { content } => {
                if self.print_comments {
                    self.printer.print("/*");
                    self.printer.print(&sanitize_block_comment(&content));
                    self.printer.print_ln_s("*/");
                }
            }
            LambdaExpr { .. } => self.visit_lambda(id, arg),
            MethodReferenceExpr { .. } => self.visit_method_ref(id, arg),
            TypeExpr { typ } => {
                self.print_java_comment(id, arg);
                if let Some(t) = typ {
                    self.visit(t, arg);
                }
            }
            AnnotationExpr { .. } => {}
        }
    }

    // ---- per-node helpers (the heavier methods) ----

    fn visit_compilation_unit(&mut self, id: NodeId, arg: Arg) {
        let (package, imports, types) = match self.kind(id) {
            Node::CompilationUnit { package, imports, types } => (package, imports, types),
            _ => unreachable!(),
        };
        self.print_java_comment(id, arg);
        if let Some(p) = package {
            self.visit(p, arg);
        }
        let _ = imports; // imports are intentionally not emitted (commented out upstream)
        if self.id.has_throws() {
            self.printer.print_ln_s("use std::rc::*;");
            self.printer.print_ln_s("use java::exc::*;");
        }
        if !types.is_empty() {
            let n = types.len();
            for (i, &t) in types.iter().enumerate() {
                self.visit(t, arg);
                self.printer.print_ln();
                if i + 1 < n {
                    self.printer.print_ln();
                }
            }
        }
        self.print_orphan_comments_ending(id);
    }

    fn visit_name_expr(&mut self, id: NodeId, name: &str, arg: Arg) {
        self.print_java_comment(id, arg);
        // A variable captured by the current anonymous class -> its field.
        if self.anon_captures.contains(name) {
            let s = self.to_snake_if_necessary(name);
            self.printer.print(&format!("self.{s}"));
            self.print_orphan_comments_ending(id);
            return;
        }
        let decl = self.id.find_declaration_node_for(self.arena, name, id);
        // An inherited instance field (not declared locally): reach it through the
        // embedded `base` field(s).
        if decl.is_none() {
            // Inherited instance field of a project superclass -> through `base`.
            if let Some(path) = self.inherited_field(name) {
                self.printer.print(&path);
                self.print_orphan_comments_ending(id);
                return;
            }
            // An instance field of the enclosing class, reached from a non-static
            // inner class -> `self.__outer.<field>`.
            if let Some(path) = self.enclosing_field(name) {
                self.printer.print(&path);
                self.print_orphan_comments_ending(id);
                return;
            }
            // An inherited static constant (a `static final` in an ancestor) ->
            // `<ParentPath>::NAME` (associated consts aren't reached via Deref).
            if let Some(path) = self.inherited_static_const(name) {
                self.printer.print(&path);
                self.print_orphan_comments_ending(id);
                return;
            }
            // A statically-imported constant/field referenced bare -> qualify
            // with the owning type's path (`Class::NAME`).
            if let Some(path) = self.static_import_path(name) {
                self.printer.print(&path);
                self.print_orphan_comments_ending(id);
                return;
            }
            // Otherwise, an unresolved bare name in a class extending an external
            // (stub) superclass is assumed to be that parent's field: `self.base.x`
            // (and recorded on the parent's stub so the field exists).
            if let Some((fqn, rust_name)) = self.current_external_base.clone() {
                let snake = self.to_snake_if_necessary(name);
                if self.emit_stubs {
                    self.stubs.borrow_mut().add_field(&fqn, &rust_name, &snake);
                }
                self.printer.print(&format!("{}.base.{snake}", self.self_receiver()));
                self.print_orphan_comments_ending(id);
                return;
            }
        }
        let nullable = decl.map(|(_, d)| self.nullable.contains(&d)).unwrap_or(false);
        // In an inner class, a name whose declaration is a *field* but which is
        // not one of the inner's own fields is an enclosing-class member found in
        // the same compilation unit -> reach it through `__outer`.
        if let Some((_, right)) = decl {
            if self.enclosing_class_fqn.is_some()
                && self.is_non_static_field_declaration(right)
                && !self.class_field_names.contains(name)
            {
                if let Some(path) = self.enclosing_field(name) {
                    self.printer.print(&path);
                    if nullable && !self.expect_option {
                        self.printer.print(".unwrap()");
                    }
                    self.print_orphan_comments_ending(id);
                    return;
                }
            }
        }
        if let Some((_, right)) = decl {
            // In a `static` method there's no receiver, so a member accessed bare
            // there is necessarily static -> `Self::` (Java forbids reaching an
            // instance member without a receiver from a static context).
            let recv = if self.in_static_method {
                "Self::"
            } else if self.id.is_in_constructor() {
                "__self."
            } else {
                "self."
            };
            if self.is_non_static_field_declaration(right)
                || self.is_non_static_method_declaration(right)
            {
                self.printer.print(recv);
            } else if self.is_static_field_declaration(right) {
                // A static field is an associated const: `Self::F`.
                self.printer.print("Self::");
            }
        }
        let s = self.to_snake_if_necessary(name);
        self.printer.print(&s);
        // A nullable value used where the plain value is expected gets unwrapped.
        if nullable && !self.expect_option {
            self.printer.print(".unwrap()");
        }
        self.print_orphan_comments_ending(id);
    }

    // ---- provenance (Java FQN markers, for symbol-map extraction) ----

    fn package_prefix(&self) -> String {
        match &self.id.package_name {
            Some(p) if !p.is_empty() => format!("{p}."),
            _ => String::new(),
        }
    }

    /// Java FQN of a type declaration: `pkg.Outer.Inner` (walks the parent chain
    /// so hoisted nested types keep their enclosing names).
    fn java_type_fqn(&self, type_node: NodeId) -> String {
        let mut parts = Vec::new();
        let mut cur = Some(type_node);
        while let Some(n) = cur {
            match self.arena.kind(n) {
                Node::ClassOrInterfaceDeclaration { name, .. }
                | Node::EnumDeclaration { name, .. } => parts.push(name.clone()),
                _ => {}
            }
            cur = self.arena.parent(n);
        }
        parts.reverse();
        format!("{}{}", self.package_prefix(), parts.join("."))
    }

    /// Java FQN of a member: `pkg.Type#memberName`.
    fn java_member_fqn(&self, member_node: NodeId, java_name: &str) -> String {
        let mut cur = self.arena.parent(member_node);
        while let Some(n) = cur {
            if matches!(
                self.arena.kind(n),
                Node::ClassOrInterfaceDeclaration { .. } | Node::EnumDeclaration { .. }
            ) {
                return format!("{}#{}", self.java_type_fqn(n), java_name);
            }
            cur = self.arena.parent(n);
        }
        let pkg = self.package_prefix();
        format!("{}#{}", pkg.trim_end_matches('.'), java_name)
    }

    /// Emit a `/// @java <marker>` provenance doc line.
    fn emit_provenance(&mut self, marker: &str) {
        self.printer.print(&format!("/// @java {marker}"));
        self.printer.print_ln();
    }

    fn visit_class(&mut self, id: NodeId, arg: Arg) {
        let (modifiers_v, is_interface, name, type_parameters, extends, implements, members) =
            match self.kind(id) {
                Node::ClassOrInterfaceDeclaration {
                    modifiers,
                    is_interface,
                    name,
                    type_parameters,
                    extends,
                    implements,
                    members,
                } => (modifiers, is_interface, name, type_parameters, extends, implements, members),
                _ => unreachable!(),
            };
        self.print_java_comment(id, arg);
        self.emit_provenance(&self.java_type_fqn(id));

        if is_interface {
            self.visit_trait(modifiers_v, &name, &type_parameters, &extends, &members, arg);
            return;
        }

        // Track this class's instance field names for `&mut self` decisions.
        let saved_fields = std::mem::take(&mut self.class_field_names);
        // Overload-disambiguation table for this type (restored after, so nested
        // types don't clobber the enclosing type's table).
        let saved_overload_name = std::mem::take(&mut self.overload_name);
        let saved_overload_arity = std::mem::take(&mut self.overload_by_arity);
        let saved_class_fqn = self.current_class_fqn.take();
        self.current_class_fqn = Some(self.java_type_fqn(id));
        // This class's non-static inner classes — their instantiation sites get
        // the enclosing instance threaded in.
        let saved_inner = std::mem::take(&mut self.current_inner_classes);
        self.current_inner_classes = members
            .iter()
            .filter_map(|&m| match self.arena.kind(m) {
                Node::ClassOrInterfaceDeclaration { modifiers, name, .. }
                    if !modifiers::is_static(*modifiers) =>
                {
                    Some(name.clone())
                }
                _ => None,
            })
            .collect();
        self.compute_overloads(&members);

        // A hoisted *inner* (non-static) class re-declares the enclosing type
        // parameters it references. The inner's own params take precedence (Java
        // allows shadowing; Rust forbids a duplicate name), so the carried set
        // excludes any name the inner already declares.
        let own_names: std::collections::HashSet<String> =
            type_parameters.iter().filter_map(|&p| self.type_param_name(p)).collect();
        let extra_params: Vec<NodeId> = if modifiers::is_static(modifiers_v) {
            Vec::new()
        } else if self.enclosing_class_fqn.is_some() {
            // Capturing inner: carry *all* the immediate outer's params (the
            // `__outer: Rc<RefCell<Outer<…>>>` field references every one).
            self.enclosing_class_params
                .clone()
                .into_iter()
                .filter(|&p| self.type_param_name(p).map(|n| !own_names.contains(&n)).unwrap_or(false))
                .collect()
        } else if !self.enclosing_type_params.is_empty() {
            // Non-capturing nested: re-declare only the outer params it uses.
            self.enclosing_type_params
                .clone()
                .into_iter()
                .filter(|&p| {
                    self.type_param_name(p)
                        .map(|n| {
                            !own_names.contains(&n)
                                && members.iter().any(|&m| self.subtree_uses_type(m, &n))
                        })
                        .unwrap_or(false)
                })
                .collect()
        } else {
            Vec::new()
        };
        let mut combined: Vec<NodeId> = extra_params.clone();
        combined.extend(type_parameters.iter().copied());

        for &m in &members {
            if let Node::FieldDeclaration { modifiers, variables, .. } = self.arena.kind(m) {
                if !modifiers::is_static(*modifiers) {
                    for &var in variables {
                        if let Node::VariableDeclarator { id: vid, .. } = self.arena.kind(var) {
                            if let Node::VariableDeclaratorId { name } = self.arena.kind(*vid) {
                                self.class_field_names.insert(name.clone());
                            }
                        }
                    }
                }
            }
        }

        // ---- struct ----
        // Clone: so field values can be cloned out from behind `&self`.
        // Default: so generated `new(...) -> Self` can start from a default value.
        self.printer.print_ln_s("#[derive(Clone, Default)]");
        self.print_modifiers(modifiers_v);
        self.printer.print("struct ");
        self.printer.print(&name);
        self.print_type_parameters(&combined, arg);
        let _ = &implements; // `implements` -> traits is Stage 2; not modelled here.
        // Single inheritance via composition: embed the superclass as `base`.
        let parent_rust = extends.first().map(|&e| self.accept_and_cut(e, arg).trim().to_string());
        // External (stub) superclass? Then bare inherited fields go through `base`.
        let saved_ext_base = self.current_external_base.take();
        self.current_external_base = extends.first().and_then(|&e| {
            let simple = self.type_simple_name(e)?;
            let external = match self.resolve_type_sym(&simple) {
                None => true,
                Some(t) => t.rust_path.contains("::stub_"),
            };
            if !external {
                return None;
            }
            let cands = self.type_candidates(&simple);
            let fqn = cands.into_iter().find(|c| c.contains('.')).unwrap_or_else(|| simple.clone());
            Some((fqn, map_type_name(&simple).replace('$', "_")))
        });
        self.printer.print_ln_s(" {");
        self.printer.indent();
        if let Some(p) = &parent_rust {
            self.printer.print_ln_s(&format!("pub base: {p},"));
        }
        // Carried outer type params must appear in a field (E0392) — `PhantomData`.
        if !extra_params.is_empty() {
            let names: Vec<String> =
                extra_params.iter().filter_map(|&p| self.type_param_name(p)).collect();
            let inner =
                if names.len() == 1 { names[0].clone() } else { format!("({})", names.join(", ")) };
            self.printer.print_ln_s(&format!("pub __phantom: std::marker::PhantomData<{inner}>,"));
        }
        // A non-static inner class captures the enclosing instance: an
        // `Rc<RefCell<Outer<…>>>` field models shared mutable access to the real
        // parent. `Default` fills it (a default parent) so the struct still
        // derives `Default`; the invented constructor overwrites it with the
        // passed-in parent (the real wiring is a downstream ownership concern).
        if let Some(outer_fqn) = self.enclosing_class_fqn.clone() {
            if let Some(t) = self.link.lookup(&outer_fqn) {
                let path = self.crate_relativize(&t.rust_path);
                let names: Vec<String> = self
                    .enclosing_class_params
                    .iter()
                    .filter_map(|&p| self.type_param_name(p))
                    .collect();
                let args = if names.is_empty() { String::new() } else { format!("<{}>", names.join(", ")) };
                self.printer
                    .print_ln_s(&format!("pub __outer: std::rc::Rc<std::cell::RefCell<{path}{args}>>,"));
            }
        }
        for &m in &members {
            if let Node::FieldDeclaration { modifiers, .. } = self.arena.kind(m) {
                if !modifiers::is_static(*modifiers) {
                    self.emit_struct_field(m, arg);
                }
            }
        }
        self.printer.unindent();
        self.printer.print_ln_s("}");
        // `Deref`/`DerefMut` to the base so inherited methods dispatch and
        // overrides (inherent items) shadow them.
        if let Some(p) = &parent_rust {
            self.printer.print_ln();
            self.printer.print("impl");
            self.print_type_parameters(&combined, arg);
            self.printer.print(" std::ops::Deref for ");
            self.printer.print(&name);
            self.print_type_param_names(&combined);
            self.printer.print_ln_s(" {");
            self.printer.print_ln_s(&format!("    type Target = {p};"));
            self.printer.print_ln_s(&format!("    fn deref(&self) -> &{p} {{ &self.base }}"));
            self.printer.print_ln_s("}");
            self.printer.print("impl");
            self.print_type_parameters(&combined, arg);
            self.printer.print(" std::ops::DerefMut for ");
            self.printer.print(&name);
            self.print_type_param_names(&combined);
            self.printer.print_ln_s(" {");
            self.printer.print_ln_s(&format!("    fn deref_mut(&mut self) -> &mut {p} {{ &mut self.base }}"));
            self.printer.print_ln_s("}");
        }
        self.printer.print_ln();

        // ---- impl ----
        // `impl<T: Bound> Type<T>` — bounds in the binder, names in the path.
        self.printer.print("impl");
        self.print_type_parameters(&combined, arg);
        self.printer.print(" ");
        self.printer.print(&name);
        self.print_type_param_names(&combined);
        self.printer.print_ln_s(" {");
        self.printer.indent();
        let impl_names: Vec<String> =
            combined.iter().filter_map(|&p| self.type_param_name(p)).collect();
        let saved_impl_params = std::mem::replace(&mut self.impl_param_names, impl_names);
        // static fields as associated constants
        for &m in &members {
            if let Node::FieldDeclaration { modifiers, .. } = self.arena.kind(m) {
                if modifiers::is_static(*modifiers) {
                    self.emit_const_field(m, arg);
                }
            }
        }
        // methods / constructors / initializers (NOT nested types — Rust forbids
        // `struct`/`enum`/`trait` items inside an `impl`).
        self.print_members(&members, arg, Filter::Method);
        // Java's implicit no-arg constructor: emit a default `new()` when the
        // class declares none, so `new X()` (-> `X::new()`) resolves.
        let has_ctor = members
            .iter()
            .any(|&m| matches!(self.arena.kind(m), Node::ConstructorDeclaration { .. }));
        if !has_ctor {
            self.printer.print_ln();
            // A capturing inner class's generated default ctor still takes the
            // enclosing instance (matching the threaded call sites).
            if let Some(ty) = self.enclosing_outer_type() {
                self.printer.print_ln_s(&format!(
                    "pub fn new(__outer: {ty}) -> Self {{ let mut s = Self::default(); s.__outer = __outer; s }}"
                ));
            } else {
                self.printer.print_ln_s("pub fn new() -> Self { Default::default() }");
            }
        }
        self.print_orphan_comments_ending(id);
        self.printer.unindent();
        self.printer.print_ln_s("}");
        self.impl_param_names = saved_impl_params;

        // Nested type declarations are hoisted to module level. Expose this
        // class's (effective) type params so nested inner classes can re-declare
        // the ones they use.
        let saved_enclosing = self.enclosing_type_params.len();
        self.enclosing_type_params.extend(combined.iter().copied());
        // A non-static inner class captures the enclosing instance: expose this
        // class's FQN and type params to it (the inner re-declares the params and
        // types its `__outer` field against them).
        let outer_capturable = self.current_class_fqn.is_some();
        for &m in &members {
            let nested_kind = match self.arena.kind(m) {
                Node::ClassOrInterfaceDeclaration { modifiers, .. } => {
                    Some(modifiers::is_static(*modifiers))
                }
                Node::EnumDeclaration { .. } => Some(true), // enums are never inner
                _ => None,
            };
            if let Some(is_static_nested) = nested_kind {
                let prev_fqn = self.enclosing_class_fqn.take();
                let prev_params = std::mem::take(&mut self.enclosing_class_params);
                if !is_static_nested && outer_capturable {
                    self.enclosing_class_fqn = self.current_class_fqn.clone();
                    self.enclosing_class_params = combined.clone();
                }
                self.printer.print_ln();
                self.visit(m, arg);
                self.printer.print_ln();
                self.enclosing_class_fqn = prev_fqn;
                self.enclosing_class_params = prev_params;
            }
        }
        self.enclosing_type_params.truncate(saved_enclosing);
        self.class_field_names = saved_fields;
        self.overload_name = saved_overload_name;
        self.overload_by_arity = saved_overload_arity;
        self.current_class_fqn = saved_class_fqn;
        self.current_inner_classes = saved_inner;
        self.current_external_base = saved_ext_base;
    }

    fn type_param_name(&self, p: NodeId) -> Option<String> {
        match self.arena.kind(p) {
            Node::TypeParameter { name, .. } => Some(name.clone()),
            _ => None,
        }
    }

    /// Does `node`'s subtree reference a type named `name` (e.g. an outer class's
    /// type parameter used inside a nested class)?
    fn subtree_uses_type(&self, node: NodeId, name: &str) -> bool {
        let mut stack = vec![node];
        while let Some(n) = stack.pop() {
            if let Node::ClassOrInterfaceType { name: tn, .. } = self.arena.kind(n) {
                if tn == name {
                    return true;
                }
            }
            for c in self.arena.children(n) {
                stack.push(c);
            }
        }
        false
    }

    /// Does this subtree assign to an instance field (→ method needs `&mut self`)?
    fn mutates_self(&self, node: NodeId) -> bool {
        let mut stack = vec![node];
        while let Some(n) = stack.pop() {
            match self.arena.kind(n) {
                Node::AssignExpr { target, .. } => {
                    if self.is_self_target(*target) {
                        return true;
                    }
                }
                Node::UnaryExpr { expr, op } => {
                    use crate::ast::UnaryOp::*;
                    if matches!(op, PreIncrement | PreDecrement | PosIncrement | PosDecrement)
                        && self.is_self_target(*expr)
                    {
                        return true;
                    }
                }
                _ => {}
            }
            for c in self.arena.children(n) {
                stack.push(c);
            }
        }
        false
    }

    fn is_self_target(&self, t: NodeId) -> bool {
        match self.arena.kind(t) {
            Node::NameExpr { name } => self.class_field_names.contains(name),
            Node::FieldAccessExpr { scope, field, .. } => {
                matches!(self.arena.kind(*scope), Node::ThisExpr { .. })
                    || self.class_field_names.contains(field)
            }
            Node::ArrayAccessExpr { name, .. } => self.is_self_target(*name),
            _ => false,
        }
    }

    fn visit_trait(
        &mut self,
        modifiers_v: i32,
        name: &str,
        type_parameters: &[NodeId],
        extends: &[NodeId],
        members: &[NodeId],
        arg: Arg,
    ) {
        self.print_modifiers(modifiers_v);
        self.printer.print("trait ");
        self.printer.print(name);
        self.print_type_parameters(type_parameters, arg);
        // Supertraits: only keep extends that resolve to a non-generic project
        // trait. External/stub interfaces resolve to a struct (E0404) and generic
        // ones need their args (E0107), so drop those.
        let supers: Vec<NodeId> = extends
            .iter()
            .copied()
            .filter(|&e| {
                self.type_simple_name(e).map(|n| self.resolved_is_trait(&n)).unwrap_or(false)
            })
            .collect();
        if !supers.is_empty() {
            self.printer.print(" : ");
            for (i, &e) in supers.iter().enumerate() {
                if i > 0 {
                    self.printer.print(" + ");
                }
                // A supertrait is a bound: emit the trait bare, not `Box<dyn …>`.
                self.trait_bound_pos = true;
                self.visit(e, arg);
            }
        }
        self.printer.print_ln_s(" {");
        self.printer.indent();
        // Methods only; nested types (and fields) can't live in a trait body.
        let saved_overload_name = std::mem::take(&mut self.overload_name);
        let saved_overload_arity = std::mem::take(&mut self.overload_by_arity);
        self.compute_overloads(members);
        self.in_trait = true;
        self.print_members(members, arg, Filter::Method);
        self.in_trait = false;
        self.overload_name = saved_overload_name;
        self.overload_by_arity = saved_overload_arity;
        self.printer.unindent();
        self.printer.print_ln_s("}");
        // Hoist nested type declarations to module level.
        for &m in members {
            if matches!(
                self.arena.kind(m),
                Node::ClassOrInterfaceDeclaration { .. } | Node::EnumDeclaration { .. }
            ) {
                self.printer.print_ln();
                self.visit(m, arg);
                self.printer.print_ln();
            }
        }
    }

    /// Emit a non-static field as a Rust struct field: `name: Type,` (one per
    /// declared variable; Java field initializers are dropped here).
    fn emit_struct_field(&mut self, field_id: NodeId, _arg: Arg) {
        let (typ, variables) = match self.kind(field_id) {
            Node::FieldDeclaration { typ, variables, .. } => (typ, variables),
            _ => return,
        };
        for var in variables {
            let name = self.field_var_name(var);
            let java_name = self
                .var_decl_id(var)
                .and_then(|d| match self.arena.kind(d) {
                    Node::VariableDeclaratorId { name } => Some(name.clone()),
                    _ => None,
                })
                .unwrap_or_default();
            let m = self.java_member_fqn(field_id, &java_name);
            self.emit_provenance(&m);
            let nullable = self.var_decl_id(var).map(|d| self.decl_nullable(d)).unwrap_or(false);
            // `pub` so fields accessed cross-module (`x.field`) resolve.
            self.printer.print(&format!("pub {name}: "));
            if nullable {
                self.printer.print("Option<");
            }
            self.visit(typ, None);
            if nullable {
                self.printer.print(">");
            }
            self.printer.print_ln_s(",");
        }
    }

    /// The (snake-cased) loop variable name of a foreach's VariableDeclarationExpr.
    fn foreach_var_name(&self, variable: NodeId) -> String {
        if let Node::VariableDeclarationExpr { vars, .. } = self.arena.kind(variable) {
            if let Some(&v) = vars.first() {
                return self.field_var_name(v);
            }
        }
        String::new()
    }

    fn var_decl_id(&self, var: NodeId) -> Option<NodeId> {
        if let Node::VariableDeclarator { id, .. } = self.arena.kind(var) {
            Some(*id)
        } else {
            None
        }
    }

    /// Emit a static field as an associated `const`.
    fn emit_const_field(&mut self, field_id: NodeId, _arg: Arg) {
        let (typ, variables) = match self.kind(field_id) {
            Node::FieldDeclaration { typ, variables, .. } => (typ, variables),
            _ => return,
        };
        let type_str = self.accept_and_cut(typ, None);
        let type_str = type_str.trim().to_string();
        for var in variables {
            let name = self.field_var_name(var);
            let init = match self.arena.kind(var) {
                Node::VariableDeclarator { init: Some(i), .. } => Some(*i),
                _ => None,
            };
            match init {
                // String literal -> `const X: &'static str = "...";` (a Rust
                // `const`/`static` initializer must be const-evaluable, so the
                // usual `"...".to_string()` is illegal here).
                Some(i) if type_str == "String" && self.is_string_literal(i) => {
                    self.printer.print(&format!("const {name}: &'static str = "));
                    let saved = self.raw_string;
                    self.raw_string = true;
                    self.visit(i, None);
                    self.raw_string = saved;
                    self.printer.print_ln_s(";");
                }
                // Other const-evaluable literal (numeric / bool / char).
                Some(i) if self.is_const_literal(i) => {
                    self.printer.print(&format!("const {name}: "));
                    self.visit(typ, None);
                    self.printer.print(" = ");
                    self.visit(i, None);
                    self.printer.print_ln_s(";");
                }
                // Non-const initializer (constructor, Vec::new, method call):
                // wrap in `LazyLock`. `LazyLock::new` is a `const fn`, so this is
                // a valid associated `const` (a `static` is forbidden in `impl`).
                Some(i) => {
                    self.printer.print(&format!("const {name}: std::sync::LazyLock<"));
                    self.visit(typ, None);
                    self.printer.print("> = std::sync::LazyLock::new(|| ");
                    self.visit(i, None);
                    self.printer.print_ln_s(");");
                }
                // No initializer: fall back to a const default.
                None => {
                    self.printer.print(&format!("const {name}: "));
                    self.visit(typ, None);
                    self.printer.print(" = ");
                    let d = self.default_value(&type_str);
                    self.printer.print(&d);
                    self.printer.print_ln_s(";");
                }
            }
        }
    }

    fn is_string_literal(&self, n: NodeId) -> bool {
        matches!(self.arena.kind(n), Node::StringLiteralExpr { .. })
    }

    /// Is `n` a const-evaluable literal (numeric/bool/char/string, possibly with
    /// a unary sign or parens)? Such initializers are legal in a Rust `const`.
    fn is_const_literal(&self, n: NodeId) -> bool {
        match self.arena.kind(n) {
            Node::IntegerLiteralExpr { .. }
            | Node::LongLiteralExpr { .. }
            | Node::DoubleLiteralExpr { .. }
            | Node::BooleanLiteralExpr { .. }
            | Node::CharLiteralExpr { .. }
            | Node::StringLiteralExpr { .. } => true,
            Node::EnclosedExpr { inner: Some(i) } => self.is_const_literal(*i),
            Node::UnaryExpr { expr, op } => {
                matches!(op, UnaryOp::Negative | UnaryOp::Positive | UnaryOp::Inverse)
                    && self.is_const_literal(*expr)
            }
            _ => false,
        }
    }

    fn field_var_name(&self, var: NodeId) -> String {
        if let Node::VariableDeclarator { id, .. } = self.arena.kind(var) {
            if let Node::VariableDeclaratorId { name } = self.arena.kind(*id) {
                return self.to_snake_if_necessary(name);
            }
        }
        String::new()
    }

    fn visit_class_type(&mut self, id: NodeId, arg: Arg) {
        let (scope, name, type_args, using_diamond) = match self.kind(id) {
            Node::ClassOrInterfaceType { scope, name, type_args, using_diamond } => {
                (scope, name, type_args, using_diamond)
            }
            _ => unreachable!(),
        };
        self.print_java_comment(id, arg);
        // Special-case JDK types with no plain identifier mapping: the
        // untranslatable `Object`/`Class`, and functional interfaces (which
        // become `Box<dyn Fn(..)->..>`, reordering their type arguments).
        if scope.is_none() {
            let simple = name.rsplit('.').next().unwrap_or(&name);
            // Skip the JDK special-casing for a type that resolves via the link
            // (a project/dep type is never `Object`/`Class`/a functional
            // interface): `special_jdk_type` eagerly renders the type args, which
            // would consume `trait_bound_pos` before the outer type is examined.
            if !self.is_known_project_type(simple) && self.resolve_type_sym(simple).is_none() {
                if let Some(rendered) = self.special_jdk_type(simple, &type_args) {
                    self.printer.print(&rendered);
                    return;
                }
            }
        }
        let resolved = self.resolve_type_name(&name);
        // An interface (non-generic trait) used as a type isn't a value type in
        // Rust — it needs `dyn`. In an owned position it must be sized, so box
        // it (`Box<dyn Trait>`); a parameter already behind `&` opts into the
        // unsized `dyn Trait` via `trait_dyn_ref`. Reset the flag before type
        // args so nested traits box by default.
        let is_trait = scope.is_none() && self.resolved_is_trait(&name);
        let dyn_ref = self.trait_dyn_ref;
        // In a bound position (`T: Trait`) a trait is emitted bare, not wrapped.
        let wrap = is_trait && !self.trait_bound_pos;
        self.trait_dyn_ref = false;
        self.trait_bound_pos = false;
        // A scoped type `Outer.Inner`: when the name resolves to a full path (a
        // hoisted/stubbed nested type), the qualifier is subsumed by that path —
        // emit it alone. Only an unresolved nested name keeps `Outer::Inner`.
        if let Some(s) = scope {
            if !resolved.contains("::") {
                self.visit(s, arg);
                self.printer.print("::");
            }
        }
        if wrap {
            self.printer.print(if dyn_ref { "dyn " } else { "Box<dyn " });
        }
        self.printer.print(&resolved);
        // A type that resolves to a non-generic Rust type (a stub, a dep, or a
        // non-generic project type) takes no arguments — drop any Java type args
        // (`PooledWriter<T>` -> `PooledWriter`), which would be `takes 0 generic
        // arguments but N were supplied`.
        let (drop_args, raw_arity) = match self.resolve_type_sym(&name) {
            Some(t) => (
                !t.generic,
                if t.generic && type_args.is_empty() { t.generic_params.len() } else { 0 },
            ),
            None => (false, 0),
        };
        if using_diamond || drop_args {
            // No empty turbofish in Rust; let the args be inferred.
        } else if raw_arity > 0 {
            // Java raw use of a generic type (`HuffmanTree` for `HuffmanTree<T>`):
            // fill `()` placeholders so the generic arity is correct.
            let ph = vec!["()"; raw_arity].join(", ");
            self.printer.print(&format!("<{ph}>"));
        } else {
            self.print_type_args(&type_args, arg);
        }
        if wrap && !dyn_ref {
            self.printer.print(">");
        }
    }

    /// Map JDK types that have no identifier-level Rust equivalent: `Object`/
    /// `Class` → `Box<dyn Any>`, and functional interfaces → `Box<dyn Fn...>`
    /// (their type args reorder into the Fn signature). Returns `None` to fall
    /// through to normal mapping (e.g. a raw functional interface with no args).
    fn special_jdk_type(&mut self, simple: &str, type_args: &[NodeId]) -> Option<String> {
        if matches!(simple, "Object" | "Class") {
            return Some("Box<dyn std::any::Any>".to_string());
        }
        // Render each type argument to a Rust type string.
        let a: Vec<String> = type_args
            .iter()
            .map(|&t| self.accept_and_cut(t, None).trim().to_string())
            .collect();
        let n = a.len();
        let func = |params: String, ret: String| Some(format!("Box<dyn Fn({params}){ret}>"));
        match (simple, n) {
            ("Function", 2) => func(a[0].clone(), format!(" -> {}", a[1])),
            ("BiFunction", 3) => func(format!("{}, {}", a[0], a[1]), format!(" -> {}", a[2])),
            ("UnaryOperator", 1) => func(a[0].clone(), format!(" -> {}", a[0])),
            ("BinaryOperator", 1) => func(format!("{}, {}", a[0], a[0]), format!(" -> {}", a[0])),
            ("Predicate", 1) => func(a[0].clone(), " -> bool".to_string()),
            ("BiPredicate", 2) => func(format!("{}, {}", a[0], a[1]), " -> bool".to_string()),
            ("Supplier", 1) | ("Callable", 1) => func(String::new(), format!(" -> {}", a[0])),
            ("Consumer", 1) => func(a[0].clone(), String::new()),
            ("BiConsumer", 2) => func(format!("{}, {}", a[0], a[1]), String::new()),
            ("Runnable", 0) => func(String::new(), String::new()),
            _ => None,
        }
    }

    fn visit_variable_declarator(&mut self, id: NodeId, arg: Arg) {
        let (vid, init) = match self.kind(id) {
            Node::VariableDeclarator { id: vid, init } => (vid, init),
            _ => unreachable!(),
        };
        self.print_java_comment(id, arg);
        let name = self.accept_and_cut(vid, arg);
        // A local hoisted above a `switch` is declared before the match; here its
        // declaration is just an assignment (`name = init`), no `let`/type.
        let java_name = match self.arena.kind(vid) {
            Node::VariableDeclaratorId { name } => name.clone(),
            _ => name.clone(),
        };
        if self.hoisted_switch_vars.contains(&java_name) {
            self.printer.print(&name);
            let nullable = self.decl_nullable(vid);
            if let Some(i) = init {
                self.printer.print(" = ");
                if nullable {
                    self.emit_into_option(i, arg);
                } else {
                    self.emit_moved_value(i, arg);
                }
            }
            return;
        }
        let mut is_constant = false;
        // An uppercase name is a constant only as a *field* (a class static ->
        // associated `const`); a local variable that merely starts uppercase is
        // still a `let` binding (and may be mutated).
        let is_field = self
            .arena
            .parent(id)
            .map(|p| matches!(self.arena.kind(p), Node::FieldDeclaration { .. }))
            .unwrap_or(false);
        if is_field && name.chars().next().map(|c| c.is_uppercase()).unwrap_or(false) {
            self.printer.print("const ");
            is_constant = true;
        } else {
            self.printer.print("let ");
            // A linked `&mut self` call on this local also requires `let mut`.
            let java_name = match self.arena.kind(vid) {
                Node::VariableDeclaratorId { name } => name.clone(),
                _ => name.clone(),
            };
            if self.id.is_changed(self.arena, &name, id) || self.mut_borrow_params.contains(&java_name)
            {
                self.printer.print("mut ");
            }
        }
        self.printer.print(&name);
        let nullable = self.decl_nullable(vid);
        if self.is_type(arg) {
            let tmp = self.accept_and_cut(arg.unwrap(), None);
            let tmp = tmp.trim().to_string();
            // Java `var` -> let inference (no annotation).
            if tmp == "var" && !nullable {
                // emit no `: Type`
            } else {
                self.printer.print(": ");
                if nullable {
                    self.printer.print(&format!("Option<{tmp}>"));
                } else if is_constant && tmp == "String" {
                    self.printer.print("&'static str");
                } else {
                    self.printer.print(&tmp);
                }
            }
        }
        if let Some(i) = init {
            self.printer.print(" = ");
            // Java allows `char c = 65;` (int->char); Rust needs an explicit cast.
            let char_from_int = self.is_char_type(arg)
                && matches!(self.arena.kind(i), Node::IntegerLiteralExpr { .. });
            if nullable {
                self.emit_into_option(i, arg);
            } else if char_from_int {
                self.visit(i, arg);
                self.printer.print(" as u8 as char");
            } else {
                self.emit_moved_value(i, arg);
            }
        }
    }

    fn is_char_type(&self, arg: Arg) -> bool {
        matches!(
            arg.map(|a| self.arena.kind(a)),
            Some(Node::PrimitiveType { kind: PrimitiveKind::Char })
        )
    }

    fn visit_array_initializer(&mut self, id: NodeId, arg: Arg) {
        let values = match self.kind(id) {
            Node::ArrayInitializerExpr { values } => values,
            _ => unreachable!(),
        };
        self.print_java_comment(id, arg);
        // A plain Vec literal; the binding's type is emitted by the declarator.
        self.printer.print("vec![");
        for &val in &values {
            self.visit(val, None);
            self.printer.print(", ");
        }
        self.printer.print("]");
    }

    fn default_value(&self, ty: &str) -> String {
        match ty {
            "f64" | "f32" => "0.0",
            "u64" | "u32" | "u16" | "u8" | "usize" | "i64" | "i32" | "i16" | "i8" => "0",
            "bool" => "false",
            _ => "None",
        }
        .to_string()
    }

    fn visit_array_creation(&mut self, id: NodeId, arg: Arg) {
        let (typ, dimensions, initializer) = match self.kind(id) {
            Node::ArrayCreationExpr {
                typ,
                dimensions,
                initializer,
                ..
            } => (typ, dimensions, initializer),
            _ => unreachable!(),
        };
        self.print_java_comment(id, arg);
        if let Some(init) = initializer {
            // `new T[]{a, b}` -> `vec![a, b]`.
            self.visit(init, None);
        } else if !dimensions.is_empty() {
            // `new T[n]` -> `vec![<default>; (n) as usize]`, nested for `[a][b]`.
            let ty = self.accept_and_cut(typ, arg);
            let ty = ty.trim().to_string();
            let default = self.default_value(&ty);
            let mut s = default;
            for &d in dimensions.iter().rev() {
                let dim = self.accept_and_cut(d, arg);
                s = format!("vec![{s}; ({}) as usize]", dim.trim());
            }
            self.printer.print(&s);
        }
    }

    fn visit_assign(&mut self, id: NodeId, arg: Arg) {
        let (target, op, value) = match self.kind(id) {
            Node::AssignExpr { target, op, value } => (target, op, value),
            _ => unreachable!(),
        };
        self.print_java_comment(id, arg);
        // Assigning to a nullable slot: keep the target as the bare Option (no
        // unwrap) and wrap the value with Some/None.
        let target_nullable = matches!(op, AssignOp::Assign) && self.expr_nullable(target);
        let saved = self.expect_option;
        self.expect_option = target_nullable;
        self.visit(target, arg);
        self.expect_option = saved;
        self.printer.print(" ");
        self.printer.print(match op {
            AssignOp::Assign => "=",
            AssignOp::And => "&=",
            AssignOp::Or => "|=",
            AssignOp::Xor => "^=",
            AssignOp::Plus => "+=",
            AssignOp::Minus => "-=",
            AssignOp::Rem => "%=",
            AssignOp::Slash => "/=",
            AssignOp::Star => "*=",
            AssignOp::LShift => "<<=",
            AssignOp::RSignedShift => ">>=",
            AssignOp::RUnsignedShift => ">>= /* >>>= */",
        });
        self.printer.print(" ");
        if target_nullable {
            self.emit_into_option(value, arg);
        } else if matches!(op, AssignOp::Assign) {
            self.emit_moved_value(value, arg);
        } else {
            self.visit(value, arg);
        }
    }

    /// A `getClass()` call (any receiver, no args), possibly parenthesized.
    fn is_get_class_call(&self, n: NodeId) -> bool {
        match self.arena.kind(n) {
            Node::MethodCallExpr { name, args, .. } => name == "getClass" && args.is_empty(),
            Node::EnclosedExpr { inner: Some(i) } => self.is_get_class_call(*i),
            _ => false,
        }
    }

    fn visit_binary(&mut self, id: NodeId, arg: Arg) {
        if self.id.get_type(id) == Some(JClass::StringClass) {
            self.print_string_expression(id, arg);
            return;
        }
        let (left, op, right) = match self.kind(id) {
            Node::BinaryExpr { left, op, right } => (left, op, right),
            _ => unreachable!(),
        };
        // Null comparison -> Option::is_none()/is_some().
        if matches!(op, BinaryOp::Equals | BinaryOp::NotEquals) {
            let l_null = matches!(self.arena.kind(left), Node::NullLiteralExpr);
            let r_null = matches!(self.arena.kind(right), Node::NullLiteralExpr);
            if l_null ^ r_null {
                let other = if l_null { right } else { left };
                self.print_java_comment(id, arg);
                let saved = self.expect_option;
                self.expect_option = true;
                self.visit(other, arg);
                self.expect_option = saved;
                self.printer.print(if matches!(op, BinaryOp::Equals) {
                    ".is_none()"
                } else {
                    ".is_some()"
                });
                return;
            }
            // `getClass() == other.getClass()` (the Java `equals` idiom): in Rust
            // the operands are statically typed, so the runtime classes always
            // match — fold to a constant (`true` for `==`, `false` for `!=`).
            if self.is_get_class_call(left) && self.is_get_class_call(right) {
                self.print_java_comment(id, arg);
                self.printer.print(if matches!(op, BinaryOp::Equals) { "true" } else { "false" });
                return;
            }
        }
        self.print_java_comment(id, arg);
        self.visit(left, arg);
        self.printer.print(" ");
        self.printer.print(match op {
            BinaryOp::Or => "||",
            BinaryOp::And => "&&",
            BinaryOp::BinOr => "|",
            BinaryOp::BinAnd => "&",
            BinaryOp::Xor => "^",
            BinaryOp::Equals => "==",
            BinaryOp::NotEquals => "!=",
            BinaryOp::Less => "<",
            BinaryOp::Greater => ">",
            BinaryOp::LessEquals => "<=",
            BinaryOp::GreaterEquals => ">=",
            BinaryOp::LShift => "<<",
            BinaryOp::RSignedShift => ">>",
            BinaryOp::RUnsignedShift => ">> /* >>> */",
            BinaryOp::Plus => "+",
            BinaryOp::Minus => "-",
            BinaryOp::Times => "*",
            BinaryOp::Divide => "/",
            BinaryOp::Remainder => "%",
        });
        self.printer.print(" ");
        self.visit(right, arg);
    }

    fn gen_string_expr_sequence(&self, id: NodeId, result: &mut Vec<NodeId>) {
        if let Node::BinaryExpr { left, op, right } = self.kind(id) {
            if op == BinaryOp::Plus {
                self.gen_string_part(left, result);
                self.gen_string_part(right, result);
                return;
            }
        }
        result.push(id);
    }

    fn gen_string_part(&self, n: NodeId, result: &mut Vec<NodeId>) {
        if matches!(self.arena.kind(n), Node::BinaryExpr { .. }) {
            self.gen_string_expr_sequence(n, result);
        } else {
            result.push(n);
        }
    }

    fn print_string_expression(&mut self, id: NodeId, arg: Arg) {
        let mut chain = Vec::new();
        self.gen_string_expr_sequence(id, &mut chain);
        self.printer.print("format!(\"");
        for &node in &chain {
            if let Node::StringLiteralExpr { value } = self.arena.kind(node) {
                // Literal text: escape `{`/`}` (format! placeholders) and convert
                // Java escapes.
                let v = java_escapes_to_rust(value).replace('{', "{{").replace('}', "}}");
                self.printer.print(&v);
            } else {
                self.printer.print("{}");
            }
        }
        self.printer.print("\"");
        for &node in &chain {
            if !matches!(self.arena.kind(node), Node::StringLiteralExpr { .. }) && node != id {
                self.printer.print(", ");
                self.visit(node, arg);
            }
        }
        self.printer.print(")");
    }

    /// Is `s` a reference to a class/type (→ `::`), as opposed to a value
    /// (variable/field, → `.`)? A class name is an uppercase `NameExpr` that does
    /// not resolve to any declaration in scope.
    /// Reconstruct a dotted name from a pure `Name`/`FieldAccess` chain
    /// (`htsjdk.samtools.util.IOUtil`); `None` if any segment is a call/index.
    fn fqn_chain(&self, n: NodeId) -> Option<String> {
        match self.arena.kind(n) {
            Node::NameExpr { name } => Some(name.clone()),
            Node::FieldAccessExpr { scope, field, .. } => {
                Some(format!("{}.{}", self.fqn_chain(*scope)?, field))
            }
            _ => None,
        }
    }

    /// If a receiver is a fully-qualified reference to a linked type
    /// (`a.b.C.method()` / `a.b.C.FIELD`), the type's crate-relative path.
    fn resolve_fqn_type(&self, s: NodeId) -> Option<String> {
        if self.link.is_empty() {
            return None;
        }
        let chain = self.fqn_chain(s)?;
        if !chain.contains('.') {
            return None; // a bare simple name is handled elsewhere
        }
        let t = self.link.lookup(&chain)?;
        Some(self.crate_relativize(&t.rust_path))
    }

    fn is_static_class_ref(&self, s: NodeId) -> bool {
        // A fully-qualified type reference (`a.b.C`) is a static class ref.
        if self.resolve_fqn_type(s).is_some() {
            return true;
        }
        match self.arena.kind(s) {
            Node::NameExpr { name } => {
                if !name.chars().next().map(|c| c.is_uppercase()).unwrap_or(false) {
                    return false;
                }
                // A statically-imported *member* (e.g. an enum constant `CG` from
                // `import static SAMTag.CG`) is a value, not a static class ref:
                // it resolves via `static_import_path` to `Owner::CG`, and the
                // call separator must be `.` (`SAMTag::CG.name()`).
                if self.id.find_declaration_node_for(self.arena, name, s).is_none()
                    && self.static_import_path(name).is_some()
                {
                    return false;
                }
                // A type reference: either an unknown uppercase name, or one that
                // resolves to a type declaration (e.g. a static call on the
                // class's own name, `Foo.bar()` inside `Foo`). A name resolving to
                // a value (local/param/field) is NOT a static class ref.
                match self.id.find_declaration_node_for(self.arena, name, s) {
                    None => true,
                    Some((_, decl)) => matches!(
                        self.arena.kind(decl),
                        Node::ClassOrInterfaceDeclaration { .. } | Node::EnumDeclaration { .. }
                    ),
                }
            }
            _ => false,
        }
    }

    /// Emit a method-call / field-access receiver. For a static type reference
    /// (`Foo.bar()`, `Foo.CONST`), emit the *resolved* type path (crate- or
    /// dependency-qualified) instead of the bare name, so it resolves.
    fn emit_scope(&mut self, s: NodeId, arg: Arg) {
        // A fully-qualified type chain (`a.b.C.m()`) -> the resolved crate path,
        // not the bare `a.b.C` dotted chain (whose head `a` is an unknown value).
        if let Some(path) = self.resolve_fqn_type(s) {
            self.printer.print(&path);
            return;
        }
        if self.is_static_class_ref(s) {
            if let Node::NameExpr { name } = self.arena.kind(s) {
                let name = name.clone();
                let resolved = self.resolve_type_name(&name);
                self.printer.print(&resolved);
                return;
            }
        }
        self.visit(s, arg);
    }

    fn visit_field_access(&mut self, id: NodeId, arg: Arg) {
        let (scope, field) = match self.kind(id) {
            Node::FieldAccessExpr { scope, field, .. } => (scope, field),
            _ => unreachable!(),
        };
        self.print_java_comment(id, arg);
        // Record an instance-field access on an unresolved external type as a
        // stub struct field.
        if self.emit_stubs && field != "length" && !self.is_static_class_ref(scope) {
            if let Some(tname) = self.callee_recv_type(scope) {
                if let Some(key) = self.missing_type_key(&tname) {
                    let rust_struct = map_type_name(&tname).replace('$', "_");
                    let f = self.to_snake_if_necessary(&field);
                    self.stubs.borrow_mut().add_field(&key, &rust_struct, &f);
                }
            }
        }
        self.emit_scope(scope, arg);
        self.printer.print(if self.is_static_class_ref(scope) { "::" } else { "." });
        // `.length` -> `.len()`; otherwise snake-case + keyword-escape the field
        // to match how the field declaration is emitted.
        if field == "length" {
            self.printer.print("len()");
        } else {
            let f = self.to_snake_if_necessary(&field);
            self.printer.print(&f);
        }
    }

    fn visit_method_call(&mut self, id: NodeId, arg: Arg) {
        let (scope, type_args, name, args) = match self.kind(id) {
            Node::MethodCallExpr { scope, type_args, name, args } => (scope, type_args, name, args),
            _ => unreachable!(),
        };
        // `getClass().getName()`/`.getSimpleName()` (and a bare `getClass()`) ->
        // the Rust type name. These appear in `toString`/log strings; folding the
        // chain to `type_name::<Self>()` compiles (display-only; precise runtime
        // class is a later concern).
        if args.is_empty() {
            if matches!(name.as_str(), "getName" | "getSimpleName" | "getCanonicalName")
                && scope.map(|s| self.is_get_class_call(s)).unwrap_or(false)
            {
                self.print_java_comment(id, arg);
                self.printer.print("std::any::type_name::<Self>()");
                return;
            }
            if name == "getClass" {
                self.print_java_comment(id, arg);
                self.printer.print("std::any::type_name::<Self>()");
                return;
            }
        }
        // A linked dependency method takes precedence over the built-in stdlib
        // rewrites below — those assume Rust collection/String/Math receivers, so
        // e.g. `jsonObj.put(k, v)` must not be rewritten to `.insert(...)` when
        // `jsonObj` is a linked `JSONObject`. Resolve the callee first and, when
        // it matches a linked type's method, skip the heuristic shortcuts.
        let callee = self.resolve_linked_callee(scope, &name);
        // Also skip the stdlib rewrites when the receiver has a known user type
        // (a project/linked class that defines its own method of this name) —
        // e.g. `dict.size()` on a `SAMSequenceDictionary` must call its `size`,
        // not become `.len()`. Unknown receiver types keep the old behaviour.
        let user_recv = scope.map(|s| self.receiver_is_user_type(s)).unwrap_or(false);
        if callee.is_none() && !user_recv {
            if self.try_emit_print_macro(scope, &name, &args, arg) {
                return;
            }
            if self.try_emit_math(scope, &name, &args, arg) {
                return;
            }
            if self.try_emit_int_range(scope, &name, &args, arg) {
                return;
            }
            if self.try_emit_optional_static(scope, &name, &args, arg) {
                return;
            }
            if self.try_emit_string_format(scope, &name, &args, arg) {
                return;
            }
            if self.try_emit_known_method(scope, &name, &args, arg) {
                return;
            }
        }
        self.print_java_comment(id, arg);
        if let Some(s) = scope {
            self.emit_scope(s, arg);
            self.printer.print(if self.is_static_class_ref(s) { "::" } else { "." });
        }
        // Explicit method type arguments are dropped (Rust infers them; emitting
        // `::<T>name` here would be invalid).
        let _ = &type_args;
        if scope.is_none() {
            // In a constructor body the receiver is `__self`, not `self`. Inside a
            // `static` method there is no receiver, so a bare self-call must be a
            // static `Self::` call (Java only allows calling static methods bare
            // from a static context).
            let recv = if self.id.is_in_constructor() { "__self." } else { "self." };
            if let Some((_, right)) = self.id.find_declaration_node_for(self.arena, &name, id) {
                match self.arena.kind(right) {
                    Node::MethodDeclaration { modifiers: m, .. } => {
                        if modifiers::is_static(*m) || self.in_static_method {
                            // A static method of the current type: `Self::name`,
                            // never a bare `::name` (an invalid crate-root path).
                            self.printer.print("Self::");
                        } else {
                            self.printer.print(recv);
                        }
                    }
                    // A non-callable shadow (e.g. a same-named local): in a static
                    // method a bare call is still `Self::` (no `self`).
                    _ => self.printer.print(if self.in_static_method { "Self::" } else { recv }),
                }
            } else if self.inherited_method(&name) {
                // Inherited instance method: `self.m()` dispatches through Deref
                // (or `Self::` from a static context).
                self.printer.print(if self.in_static_method { "Self::" } else { recv });
            } else if self.enclosing_method(&name) {
                // An instance method of the enclosing class, called from a
                // non-static inner class: `<recv>.__outer.borrow().m()`.
                self.printer.print(&format!("{}.__outer.borrow().", self.self_receiver()));
            } else if !self.in_static_method {
                // An unresolved bare call in an instance method is `this.method()`
                // — an inner-class or inherited method we couldn't resolve
                // statically. Emit the receiver so `Deref` dispatch can find it
                // (Java has no free functions, and stdlib shortcuts ran earlier).
                self.printer.print(recv);
            }
        }
        match callee {
            Some(m) => {
                self.printer.print(&m.rust);
                self.print_arguments_linked(&args, &m.params, arg);
                if m.ret_nullable && !self.expect_option {
                    self.printer.print(".unwrap()");
                }
            }
            None => {
                self.record_missing_call(scope, &name, &args, id);
                let s = self.call_emitted_name(scope, &name, args.len());
                self.printer.print(&s);
                self.print_arguments(&args, arg);
                // A call to a nullable-returning method used as a plain value is
                // unwrapped.
                if scope.is_none() && !self.expect_option && self.name_decl_nullable(&name, id) {
                    self.printer.print(".unwrap()");
                }
            }
        }
    }

    /// Map `System.out.println(x)` / `System.err.print(x)` etc. to the Rust
    /// print macros. Returns true if it handled the call.
    fn try_emit_print_macro(&mut self, scope: Option<NodeId>, name: &str, args: &[NodeId], arg: Arg) -> bool {
        let Some(s) = scope else { return false };
        let Node::FieldAccessExpr { scope: inner, field, .. } = self.arena.kind(s) else {
            return false;
        };
        let (inner, field) = (*inner, field.clone());
        if !matches!(self.arena.kind(inner), Node::NameExpr { name } if name == "System") {
            return false;
        }
        let mac = match (field.as_str(), name) {
            ("out", "println") => "println",
            ("err", "println") => "eprintln",
            ("out", "print") => "print",
            ("err", "print") => "eprint",
            _ => return false,
        };
        self.printer.print(mac);
        self.printer.print("!(");
        if let Some(&first) = args.first() {
            self.printer.print("\"{}\", ");
            self.visit(first, arg);
        }
        self.printer.print(")");
        true
    }

    /// `Optional.of(x)` -> `Some(x)`, `Optional.empty()` -> `None`.
    fn try_emit_optional_static(&mut self, scope: Option<NodeId>, name: &str, args: &[NodeId], arg: Arg) -> bool {
        let Some(s) = scope else { return false };
        if !matches!(self.arena.kind(s), Node::NameExpr { name } if name == "Optional") {
            return false;
        }
        match (name, args.len()) {
            ("of", 1) | ("ofNullable", 1) => {
                self.printer.print("Some(");
                self.visit(args[0], arg);
                self.printer.print(")");
                true
            }
            ("empty", 0) => {
                self.printer.print("None");
                true
            }
            _ => false,
        }
    }

    /// `IntStream.range(a, b)` -> `((a)..(b))`, `rangeClosed` -> `..=`.
    fn try_emit_int_range(&mut self, scope: Option<NodeId>, name: &str, args: &[NodeId], arg: Arg) -> bool {
        let Some(s) = scope else { return false };
        let is_intstream = matches!(
            self.arena.kind(s),
            Node::NameExpr { name } if name == "IntStream" || name == "LongStream"
        );
        if !is_intstream || args.len() != 2 {
            return false;
        }
        let sep = match name {
            "range" => "..",
            "rangeClosed" => "..=",
            _ => return false,
        };
        self.printer.print("((");
        self.visit(args[0], arg);
        self.printer.print(")");
        self.printer.print(sep);
        self.printer.print("(");
        self.visit(args[1], arg);
        self.printer.print("))");
        true
    }

    /// `String.format("%d ...", a, ...)` -> `format!("{} ...", a, ...)`.
    fn try_emit_string_format(&mut self, scope: Option<NodeId>, name: &str, args: &[NodeId], arg: Arg) -> bool {
        let Some(s) = scope else { return false };
        if name != "format"
            || !matches!(self.arena.kind(s), Node::NameExpr { name } if name == "String")
        {
            return false;
        }
        let Some(&fmt_node) = args.first() else { return false };
        let Node::StringLiteralExpr { value } = self.arena.kind(fmt_node) else {
            return false;
        };
        let converted = java_format_to_rust(value);
        self.printer.print("format!(\"");
        self.printer.print(&converted);
        self.printer.print("\"");
        // Emit only as many value args as there are `{}` placeholders: Java code
        // that misuses SLF4J-style `{}` inside `String.format` leaves them as
        // literal text (zero real specifiers), and Rust rejects unused args.
        let placeholders = count_fmt_placeholders(&converted);
        for &a in args[1..].iter().take(placeholders) {
            self.printer.print(", ");
            self.visit(a, arg);
        }
        self.printer.print(")");
        true
    }

    /// The declared Java type's simple name for the variable `name` refers to.
    fn decl_java_type_name(&self, name: &str, at: NodeId) -> Option<String> {
        let (_, decl) = self.id.find_declaration_node_for(self.arena, name, at)?;
        let parent = self.arena.parent(decl)?;
        let grand = self.arena.parent(parent);
        let typ = match self.arena.kind(parent) {
            Node::Parameter { typ, .. } => *typ,
            _ => match grand.map(|g| self.arena.kind(g)) {
                Some(Node::FieldDeclaration { typ, .. })
                | Some(Node::VariableDeclarationExpr { typ, .. }) => Some(*typ),
                _ => None,
            },
        }?;
        self.type_simple_name(typ)
    }

    fn type_simple_name(&self, typ: NodeId) -> Option<String> {
        match self.arena.kind(typ) {
            Node::ClassOrInterfaceType { name, .. } => Some(name.clone()),
            Node::ReferenceType { typ, .. } => self.type_simple_name(*typ),
            _ => None,
        }
    }

    /// Map common collection / String methods to their Rust equivalents.
    fn try_emit_known_method(&mut self, scope: Option<NodeId>, name: &str, args: &[NodeId], arg: Arg) -> bool {
        let Some(recv) = scope else { return false };
        // A static class reference (`Paths.get(...)`, `Files.x(...)`) is not a
        // collection/String value, so the stdlib instance-method rewrites (e.g.
        // `.get(i)` -> indexing) must not apply.
        if self.is_static_class_ref(recv) {
            return false;
        }
        match (name, args.len()) {
            ("size", 0) | ("length", 0) => {
                self.printer.print("(");
                self.visit(recv, arg);
                self.printer.print(".len() as i32)");
                true
            }
            ("isEmpty", 0) => {
                // Optional.isEmpty -> is_none; collection isEmpty -> is_empty.
                let opt = matches!(self.recv_type_name(recv).as_deref(), Some("Optional"));
                self.visit(recv, arg);
                self.printer.print(if opt { ".is_none()" } else { ".is_empty()" });
                true
            }
            // ---- Optional ----
            ("isPresent", 0) => self.emit_recv_method(recv, "is_some", arg),
            ("orElse", 1) => {
                self.visit(recv, arg);
                self.printer.print(".unwrap_or(");
                self.visit(args[0], arg);
                self.printer.print(")");
                true
            }
            ("orElseGet", 1) => {
                self.visit(recv, arg);
                self.printer.print(".unwrap_or_else(");
                self.visit(args[0], arg);
                self.printer.print(")");
                true
            }
            ("get", 0) => self.emit_recv_method(recv, "unwrap", arg),
            // ---- reduce / sorted ----
            ("reduce", 2) => {
                self.visit(recv, arg);
                self.printer.print(".fold(");
                self.visit(args[0], arg);
                self.printer.print(", ");
                self.visit(args[1], arg);
                self.printer.print(")");
                true
            }
            ("sorted", 0) => {
                self.printer.print("{ let mut __s = ");
                self.visit(recv, arg);
                self.printer.print(".collect::<Vec<_>>(); __s.sort(); __s.into_iter() }");
                true
            }
            ("equals", 1) => {
                self.printer.print("(");
                self.visit(recv, arg);
                self.printer.print(" == ");
                self.visit(args[0], arg);
                self.printer.print(")");
                true
            }
            ("add", 1) => {
                // Set.add -> insert; List/Collection.add -> push.
                let is_set = matches!(
                    self.recv_type_name(recv).as_deref(),
                    Some("Set" | "HashSet" | "LinkedHashSet" | "TreeSet")
                );
                self.visit(recv, arg);
                self.printer.print(if is_set { ".insert(" } else { ".push(" });
                self.visit(args[0], arg);
                self.printer.print(")");
                true
            }
            ("get", 1) => {
                let is_map = matches!(
                    self.recv_type_name(recv).as_deref(),
                    Some("Map" | "HashMap" | "LinkedHashMap" | "TreeMap")
                );
                if is_map {
                    // Map.get(k) -> value by clone (panics if absent, ~ Java null deref).
                    self.visit(recv, arg);
                    self.printer.print(".get(&(");
                    self.visit(args[0], arg);
                    self.printer.print(")).cloned().unwrap()");
                } else {
                    // List.get(i) -> indexed element (cloned to own it).
                    self.visit(recv, arg);
                    self.printer.print("[(");
                    self.visit(args[0], arg);
                    self.printer.print(") as usize].clone()");
                }
                true
            }
            ("put", 2) => {
                self.visit(recv, arg);
                self.printer.print(".insert(");
                self.visit(args[0], arg);
                self.printer.print(", ");
                self.visit(args[1], arg);
                self.printer.print(")");
                true
            }
            ("contains", 1) => {
                let is_string = matches!(self.recv_type_name(recv).as_deref(), Some("String"));
                self.visit(recv, arg);
                if is_string {
                    self.printer.print(".contains((");
                    self.visit(args[0], arg);
                    self.printer.print(").as_str())");
                } else {
                    self.printer.print(".contains(&(");
                    self.visit(args[0], arg);
                    self.printer.print("))");
                }
                true
            }
            // ---- more streams ----
            ("findFirst", 0) | ("findAny", 0) => {
                self.visit(recv, arg);
                self.printer.print(".next()");
                true
            }
            ("limit", 1) => self.emit_iter_count(recv, "take", args[0], arg),
            ("skip", 1) => self.emit_iter_count(recv, "skip", args[0], arg),
            ("sum", 0) => {
                self.visit(recv, arg);
                self.printer.print(".sum::<i32>()");
                true
            }
            // ---- more String ops (arg is a String -> &str) ----
            ("startsWith", 1) => self.emit_str_arg(recv, "starts_with", args[0], arg),
            ("endsWith", 1) => self.emit_str_arg(recv, "ends_with", args[0], arg),
            ("replace", 2) => {
                self.visit(recv, arg);
                self.printer.print(".replace((");
                self.visit(args[0], arg);
                self.printer.print(").as_str(), (");
                self.visit(args[1], arg);
                self.printer.print(").as_str())");
                true
            }
            ("split", 1) => {
                self.visit(recv, arg);
                self.printer.print(".split((");
                self.visit(args[0], arg);
                self.printer.print(").as_str()).map(|x| x.to_string()).collect::<Vec<_>>()");
                true
            }
            ("indexOf", 1) => {
                self.visit(recv, arg);
                self.printer.print(".find((");
                self.visit(args[0], arg);
                self.printer.print(").as_str()).map(|i| i as i32).unwrap_or(-1)");
                true
            }
            ("containsKey", 1) => {
                self.visit(recv, arg);
                self.printer.print(".contains_key(&(");
                self.visit(args[0], arg);
                self.printer.print("))");
                true
            }
            // ---- streams ----
            ("stream", 0) => {
                // Owned-value iterator so map/forEach closures see `T`, not `&T`.
                self.visit(recv, arg);
                self.printer.print(".iter().cloned()");
                true
            }
            ("toArray", 0) => {
                self.visit(recv, arg);
                self.printer.print(".collect::<Vec<_>>()");
                true
            }
            ("collect", _) => {
                // Inspect the Collector: joining -> join, toSet -> HashSet.
                let collector = args.first().and_then(|&a| match self.arena.kind(a) {
                    Node::MethodCallExpr { scope: Some(s), name, args, .. }
                        if matches!(self.arena.kind(*s), Node::NameExpr { name } if name == "Collectors") =>
                    {
                        Some((name.clone(), args.clone()))
                    }
                    _ => None,
                });
                match collector {
                    Some((m, cargs)) if m == "joining" => {
                        self.visit(recv, arg);
                        self.printer
                            .print(".map(|x| x.to_string()).collect::<Vec<_>>().join(");
                        if let Some(&sep) = cargs.first() {
                            self.printer.print("(");
                            self.visit(sep, arg);
                            self.printer.print(").as_str()");
                        } else {
                            self.printer.print("\"\"");
                        }
                        self.printer.print(")");
                    }
                    Some((m, _)) if m == "toSet" => {
                        self.visit(recv, arg);
                        self.printer.print(".collect::<std::collections::HashSet<_>>()");
                    }
                    _ => {
                        self.visit(recv, arg);
                        self.printer.print(".collect::<Vec<_>>()");
                    }
                }
                true
            }
            ("mapToInt", 1) | ("mapToLong", 1) | ("mapToDouble", 1) | ("mapToObj", 1) => {
                self.visit(recv, arg);
                self.printer.print(".map(");
                self.visit(args[0], arg);
                self.printer.print(")");
                true
            }
            // Predicate combinators borrow each item (`&T`), but the Java lambda
            // treats it as `T` — clone-shadow inside the closure to bridge that.
            ("filter", 1) => self.emit_predicate(recv, "filter", args[0], arg),
            ("anyMatch", 1) => self.emit_predicate(recv, "any", args[0], arg),
            ("allMatch", 1) => self.emit_predicate(recv, "all", args[0], arg),
            ("count", 0) => {
                self.printer.print("(");
                self.visit(recv, arg);
                self.printer.print(".count() as i32)");
                true
            }
            ("toLowerCase", 0) => self.emit_recv_method(recv, "to_lowercase", arg),
            ("toUpperCase", 0) => self.emit_recv_method(recv, "to_uppercase", arg),
            ("trim", 0) => {
                self.visit(recv, arg);
                self.printer.print(".trim().to_string()");
                true
            }
            ("charAt", 1) => {
                self.visit(recv, arg);
                self.printer.print(".chars().nth((");
                self.visit(args[0], arg);
                self.printer.print(") as usize).unwrap()");
                true
            }
            ("substring", 1) => {
                self.visit(recv, arg);
                self.printer.print("[(");
                self.visit(args[0], arg);
                self.printer.print(") as usize..].to_string()");
                true
            }
            ("substring", 2) => {
                self.visit(recv, arg);
                self.printer.print("[(");
                self.visit(args[0], arg);
                self.printer.print(") as usize..(");
                self.visit(args[1], arg);
                self.printer.print(") as usize].to_string()");
                true
            }
            _ => false,
        }
    }

    /// Emit `recv.<method>(|p| { let p = p.clone(); <body> })` for a borrowing
    /// predicate combinator whose argument is a one-parameter lambda.
    fn emit_predicate(&mut self, recv: NodeId, method: &str, pred: NodeId, arg: Arg) -> bool {
        let (params, body) = match self.arena.kind(pred) {
            Node::LambdaExpr { parameters, body, .. } if parameters.len() == 1 => {
                (parameters.clone(), *body)
            }
            // Non-lambda predicate (e.g. method ref): fall back to a plain call.
            _ => {
                self.visit(recv, arg);
                self.printer.print(".");
                self.printer.print(method);
                self.printer.print("(");
                self.visit(pred, arg);
                self.printer.print(")");
                return true;
            }
        };
        let p = self.param_name(params[0]);
        self.visit(recv, arg);
        self.printer.print(".");
        self.printer.print(method);
        self.printer
            .print(&format!("(|{p}| {{ let {p} = {p}.clone(); "));
        if let Node::ExpressionStmt { expression } = self.arena.kind(body) {
            let e = *expression;
            self.visit(e, arg);
        } else {
            self.visit(body, arg);
        }
        self.printer.print(" })");
        true
    }

    /// `recv.<method>((arg) as usize)` — for iterator take/skip.
    fn emit_iter_count(&mut self, recv: NodeId, method: &str, n: NodeId, arg: Arg) -> bool {
        self.visit(recv, arg);
        self.printer.print(&format!(".{method}(("));
        self.visit(n, arg);
        self.printer.print(") as usize)");
        true
    }

    /// `recv.<method>((arg).as_str())` — for String methods taking a &str.
    fn emit_str_arg(&mut self, recv: NodeId, method: &str, s: NodeId, arg: Arg) -> bool {
        self.visit(recv, arg);
        self.printer.print(&format!(".{method}(("));
        self.visit(s, arg);
        self.printer.print(").as_str())");
        true
    }

    fn emit_recv_method(&mut self, recv: NodeId, method: &str, arg: Arg) -> bool {
        self.visit(recv, arg);
        self.printer.print(".");
        self.printer.print(method);
        self.printer.print("()");
        true
    }

    fn recv_type_name(&self, recv: NodeId) -> Option<String> {
        if let Node::NameExpr { name } = self.arena.kind(recv) {
            self.decl_java_type_name(name, recv)
        } else {
            None
        }
    }

    /// Is `simple` a stdlib type the built-in method rewrites apply to (a
    /// collection that maps to `Vec`/`HashMap`/…, a `String`/`StringBuilder`, or
    /// `Optional`)? Other class types are user types and must NOT be rewritten.
    fn is_stdlib_rewritable(simple: &str) -> bool {
        map_type_name(simple) != simple
            || matches!(simple, "String" | "CharSequence" | "StringBuilder" | "StringBuffer")
    }

    /// Does the call receiver have a *known* user (project/linked/other class)
    /// type — i.e. one the collection/String rewrites must not fire on? Returns
    /// false when the type is unknown (keep the existing best-effort behaviour)
    /// or is a stdlib collection/String/Optional.
    fn receiver_is_user_type(&self, recv: NodeId) -> bool {
        match self.recv_type_name(recv) {
            Some(t) => {
                let simple = t.rsplit('.').next().unwrap_or(&t);
                !Self::is_stdlib_rewritable(simple)
            }
            None => false,
        }
    }

    /// Map `Math.x(...)` to a Rust receiver method, e.g. `Math.max(a, b)` ->
    /// `(a).max(b)`, `Math.sqrt(x)` -> `(x).sqrt()`. Returns true if handled.
    /// Is `java.lang.Math` statically imported (so its functions are called bare)?
    fn math_statically_imported(&self) -> bool {
        self.id.imports.iter().any(|i| {
            i.static_import
                && (i.import_string == "java.lang.Math"
                    || i.import_string.starts_with("java.lang.Math."))
        })
    }

    fn try_emit_math(&mut self, scope: Option<NodeId>, name: &str, args: &[NodeId], arg: Arg) -> bool {
        // `Math.x(..)`, or a bare `x(..)` when `java.lang.Math` is statically
        // imported (`import static java.lang.Math.*`).
        let is_math = match scope {
            Some(s) => matches!(self.arena.kind(s), Node::NameExpr { name } if name == "Math"),
            None => self.math_statically_imported(),
        };
        if !is_math {
            return false;
        }
        // (receiver-method, arity)
        let m = match name {
            "abs" | "sqrt" | "floor" | "ceil" | "round" | "signum" | "sin" | "cos" | "tan"
            | "exp" | "sinh" | "cosh" | "tanh" => (name, 1),
            "log" => ("ln", 1),
            "log10" => ("log10", 1),
            "max" | "min" => (name, 2),
            "pow" => ("powf", 2),
            _ => return false,
        };
        if args.len() != m.1 {
            return false;
        }
        self.printer.print("(");
        self.visit(args[0], arg);
        self.printer.print(").");
        self.printer.print(m.0);
        self.printer.print("(");
        if m.1 == 2 {
            self.visit(args[1], arg);
        }
        self.printer.print(")");
        true
    }

    fn visit_object_creation(&mut self, id: NodeId, arg: Arg) {
        let (scope, typ, type_args, args, anonymous_body) = match self.kind(id) {
            Node::ObjectCreationExpr { scope, typ, type_args, args, anonymous_body } => {
                (scope, typ, type_args, args, anonymous_body)
            }
            _ => unreachable!(),
        };
        self.print_java_comment(id, arg);
        // Anonymous class: `new Iface() { body }` -> an inline struct carrying the
        // body's methods, instantiated as a block expression. (Capturing enclosing
        // state and implementing the interface are left for an LLM pass.)
        if let Some(body) = &anonymous_body {
            let body = body.clone();
            let n = self.anon_counter;
            self.anon_counter += 1;
            let anon = format!("__Anon{n}");
            // Capture the enclosing locals/params the body references. Each is a
            // generic field, so its type is inferred from the construction site
            // (no fragile type re-derivation) — references become `self.<field>`.
            let caps = self.collect_anon_captures(&body, id);
            let snakes: Vec<String> = caps.iter().map(|c| self.to_snake_if_necessary(c)).collect();
            let generics = if caps.is_empty() {
                String::new()
            } else {
                format!("<{}>", (0..caps.len()).map(|i| format!("Cap{i}")).collect::<Vec<_>>().join(", "))
            };
            self.printer.print_ln_s("{");
            self.printer.indent();
            if caps.is_empty() {
                self.printer.print_ln_s("#[derive(Clone, Default)]");
                self.printer.print_ln_s(&format!("struct {anon} {{}}"));
            } else {
                self.printer.print_ln_s("#[derive(Clone)]");
                self.printer.print_ln_s(&format!("struct {anon}{generics} {{"));
                self.printer.indent();
                for (i, s) in snakes.iter().enumerate() {
                    self.printer.print_ln_s(&format!("{s}: Cap{i},"));
                }
                self.printer.unindent();
                self.printer.print_ln_s("}");
            }
            self.printer.print_ln_s(&format!("impl{generics} {anon}{generics} {{"));
            self.printer.indent();
            let saved_caps =
                std::mem::replace(&mut self.anon_captures, caps.iter().cloned().collect());
            self.print_members(&body, arg, Filter::Method);
            self.anon_captures = saved_caps;
            self.printer.unindent();
            self.printer.print_ln_s("}");
            if caps.is_empty() {
                self.printer.print_ln_s(&format!("{anon}::default()"));
            } else {
                let inits: Vec<String> =
                    snakes.iter().map(|s| format!("{s}: {s}.clone()")).collect();
                self.printer.print_ln_s(&format!("{anon} {{ {} }}", inits.join(", ")));
            }
            self.printer.unindent();
            self.printer.print("}");
            return;
        }
        if let Some(s) = scope {
            self.visit(s, arg);
            self.printer.print(".");
        }
        let _ = type_args;
        // Emit `<MappedType>::new(...)`, dropping the diamond/type-args. Known
        // collections are constructed with no arguments.
        let base = match self.arena.kind(typ) {
            Node::ClassOrInterfaceType { name, .. } => self.resolve_type_name(name),
            _ => self.accept_and_cut(typ, arg).trim().to_string(),
        };
        // Record a constructor stub for an unresolved external type.
        if self.emit_stubs {
            let tname = match self.arena.kind(typ) {
                Node::ClassOrInterfaceType { name, .. } => Some(name.clone()),
                _ => None,
            };
            if let Some(tname) = tname {
                if let Some(key) = self.missing_type_key(&tname) {
                    let rust_struct = map_type_name(&tname).replace('$', "_");
                    let sig = self.build_stub_sig(&args, id, crate::stubs::Receiver::None);
                    self.stubs.borrow_mut().add_ctor(&key, &rust_struct, sig);
                }
            }
        }
        // Is this a `new Inner(…)` for a non-static inner class of the current
        // class? If so, thread the enclosing instance in as the synthesized
        // `__outer` first argument (matching the invented constructor param).
        let inner_simple = match self.arena.kind(typ) {
            Node::ClassOrInterfaceType { name, .. } => {
                Some(name.rsplit('.').next().unwrap_or(name).to_string())
            }
            _ => None,
        };
        let is_inner = inner_simple.map(|n| self.current_inner_classes.contains(&n)).unwrap_or(false);
        self.printer.print(&base);
        self.printer.print("::new");
        if is_rust_collection(&base) {
            self.printer.print("()");
        } else if is_inner {
            // Thread the enclosing instance in. A static method has no `this`, so
            // there's no real outer to capture -> a default placeholder (the
            // value is a downstream ownership concern anyway).
            let parent = if self.in_static_method {
                "std::default::Default::default()".to_string()
            } else {
                format!(
                    "std::rc::Rc::new(std::cell::RefCell::new({}.clone()))",
                    self.self_receiver()
                )
            };
            self.printer.print("(");
            self.printer.print(&parent);
            for &a in &args {
                self.printer.print(", ");
                self.visit(a, arg);
            }
            self.printer.print(")");
        } else {
            self.print_arguments(&args, arg);
        }
    }

    fn is_embedded_in_stmt(&self, id: NodeId) -> bool {
        match self.arena.parent(id).map(|p| self.arena.kind(p)) {
            Some(Node::ExpressionStmt { .. }) | Some(Node::ForStmt { .. }) => false,
            _ => true,
        }
    }

    fn org_visit_unary(&mut self, id: NodeId, arg: Arg) {
        let (expr, op) = match self.kind(id) {
            Node::UnaryExpr { expr, op } => (expr, op),
            _ => unreachable!(),
        };
        self.print_java_comment(id, arg);
        match op {
            UnaryOp::Positive => self.printer.print("+"),
            UnaryOp::Negative => self.printer.print("-"),
            UnaryOp::Inverse => self.printer.print("!"), // Java `~` is Rust `!` on ints
            UnaryOp::Not => self.printer.print("!"),
            UnaryOp::PreIncrement => self.printer.print("++"),
            UnaryOp::PreDecrement => self.printer.print("--"),
            _ => {}
        }
        self.visit(expr, arg);
        match op {
            UnaryOp::PosIncrement => self.printer.print("++"),
            UnaryOp::PosDecrement => self.printer.print("--"),
            _ => {}
        }
    }

    fn visit_unary(&mut self, id: NodeId, arg: Arg) {
        let (expr, op) = match self.kind(id) {
            Node::UnaryExpr { expr, op } => (expr, op),
            _ => unreachable!(),
        };
        use UnaryOp::*;
        match op {
            PreIncrement | PreDecrement | PosIncrement | PosDecrement => {
                self.print_java_comment(id, arg);
                let opstr = if matches!(op, PreDecrement | PosDecrement) {
                    " -= 1"
                } else {
                    " += 1"
                };
                let post = matches!(op, PosIncrement | PosDecrement);
                if self.is_embedded_in_stmt(id) {
                    // Used as a value: lower to a block expression.
                    if post {
                        // x++ : yield old value
                        self.printer.print("{ let __v = ");
                        self.visit(expr, arg);
                        self.printer.print("; ");
                        self.visit(expr, arg);
                        self.printer.print(opstr);
                        self.printer.print("; __v }");
                    } else {
                        // ++x : increment then yield
                        self.printer.print("{ ");
                        self.visit(expr, arg);
                        self.printer.print(opstr);
                        self.printer.print("; ");
                        self.visit(expr, arg);
                        self.printer.print(" }");
                    }
                } else {
                    // Statement context: a plain compound assignment.
                    self.visit(expr, arg);
                    self.printer.print(opstr);
                }
            }
            Positive => {
                self.print_java_comment(id, arg);
                self.visit(expr, arg);
            }
            // Negative / Not / Inverse: prefix operator.
            _ => self.org_visit_unary(id, arg),
        }
    }

    fn visit_constructor(&mut self, id: NodeId, arg: Arg) {
        let (modifiers_v, name, type_parameters, parameters, throws, block) = match self.kind(id) {
            Node::ConstructorDeclaration {
                modifiers, name, type_parameters, parameters, throws, block,
            } => (modifiers, name, type_parameters, parameters, throws, block),
            _ => unreachable!(),
        };
        self.id.set_in_constructor(true);
        self.mut_borrow_params = self.collect_mut_borrow_params(block);
        self.print_java_comment(id, arg);
        self.emit_provenance(&self.java_member_fqn(id, "<init>"));
        self.print_modifiers(modifiers_v);
        self.print_type_parameters(&type_parameters, arg);
        if !type_parameters.is_empty() {
            self.printer.print(" ");
        }
        let _ = throws; // Java `throws` has no Rust equivalent here.
        let ctor_name = self.decl_emitted_name(id, "new");
        self.printer.print("fn ");
        self.printer.print(&ctor_name);
        self.printer.print("(");
        // A capturing inner class's constructor takes the enclosing instance as a
        // synthesized first parameter `__outer` (invented for the capture).
        let outer_ty = self.enclosing_outer_type();
        if let Some(ty) = &outer_ty {
            self.printer.print(&format!("__outer: {ty}"));
            if !parameters.is_empty() {
                self.printer.print(", ");
            }
        }
        for (i, &p) in parameters.iter().enumerate() {
            self.visit(p, arg);
            if i + 1 < parameters.len() {
                self.printer.print(", ");
            }
        }
        self.printer.print(") -> ");
        self.printer.print(&name);
        // Build the value in `__self` (`this` maps to it), then return it.
        self.printer.print_ln_s(" {");
        self.printer.indent();
        self.printer
            .print_ln_s(&format!("let mut __self: {name} = Default::default();"));
        if outer_ty.is_some() {
            self.printer.print_ln_s("__self.__outer = __outer;");
        }
        if let Node::BlockStmt { stmts } = self.kind(block) {
            for s in stmts {
                self.visit(s, arg);
                self.printer.print_ln();
            }
        }
        self.printer.print_ln_s("return __self;");
        self.printer.unindent();
        self.printer.print_ln_s("}");
        self.id.set_in_constructor(false);
        self.mut_borrow_params.clear();
    }

    fn visit_method(&mut self, id: NodeId, arg: Arg) {
        let (modifiers_v, typ, name, type_parameters, parameters, throws, body, array_count, is_default, annotations) =
            match self.kind(id) {
                Node::MethodDeclaration {
                    modifiers, typ, name, type_parameters, parameters, throws, body, array_count, is_default, annotations,
                } => (modifiers, typ, name, type_parameters, parameters, throws, body, array_count, is_default, annotations),
                _ => unreachable!(),
            };
        self.id.set_current_method(Some(name.clone()));
        self.in_static_method = modifiers::is_static(modifiers_v);
        self.mut_borrow_params =
            body.map(|b| self.collect_mut_borrow_params(b)).unwrap_or_default();
        self.print_orphan_comments_before_this_child_node(id);
        self.print_java_comment(id, arg);
        self.emit_provenance(&self.java_member_fqn(id, &name));
        for a in &annotations {
            if let Node::AnnotationExpr { name: an } = self.arena.kind(*a) {
                if self.annotation_simple_name(*an) == "Test" {
                    self.printer.print_ln_s("#[test]");
                }
            }
        }
        let _ = is_default; // Rust default methods are just methods.
        self.print_modifiers(modifiers_v);
        self.printer.print("fn ");
        let raw_type = self.accept_and_cut(typ, arg);
        let ret_nullable = self.decl_nullable(id) && raw_type.trim() != "void";
        let type_string = if ret_nullable {
            format!("Option<{}>", raw_type.trim())
        } else {
            raw_type.clone()
        };
        let snake = self.to_snake_if_necessary(&name);
        let snake = self.decl_emitted_name(id, &snake);
        self.printer.print(&snake);
        // Type parameters go after the name in Rust: `fn name<T>(...)`. Drop any
        // that re-declare an `impl` type param (Java static generic method
        // shadowing the class param — Rust forbids the shadow, E0403).
        let method_params: Vec<NodeId> = type_parameters
            .iter()
            .copied()
            .filter(|&p| {
                self.type_param_name(p).map(|n| !self.impl_param_names.contains(&n)).unwrap_or(true)
            })
            .collect();
        self.print_type_parameters(&method_params, arg);
        self.printer.print("(");
        if !modifiers::is_static(modifiers_v) {
            let needs_mut = body.map(|b| self.mutates_self(b)).unwrap_or(false);
            self.printer.print(if needs_mut { "&mut self" } else { "&self" });
            if !parameters.is_empty() {
                self.printer.print(", ");
            }
        }
        for (i, &p) in parameters.iter().enumerate() {
            self.visit(p, arg);
            if i + 1 < parameters.len() {
                self.printer.print(", ");
            }
        }
        self.printer.print(") ");
        if type_string != "void" {
            self.printer.print("-> ");
            if array_count > 0 {
                self.printer.print("/* ");
                for _ in 0..array_count {
                    self.printer.print("[]");
                }
                self.printer.print(" */");
            }
            if !throws.is_empty() {
                self.replace_throws(&throws, arg, &type_string);
            } else {
                self.printer.print(&type_string);
            }
        } else if !throws.is_empty() {
            self.printer.print(" -> ");
            self.replace_throws(&throws, arg, "()");
        }
        // A Java `static` interface method becomes a trait method with no `self`
        // receiver, which makes the trait not object-safe (so `Box<dyn Trait>`
        // fails). `where Self: Sized` exempts it from object-safety while keeping
        // it callable as `Trait::method`.
        if self.in_trait && modifiers::is_static(modifiers_v) {
            self.printer.print(" where Self: Sized");
        }
        self.printer.print(" ");
        match body {
            // No body: a trait method declaration keeps `;`; an abstract method
            // in a concrete `impl` needs a body (Rust requires one) -> stub it.
            None if self.in_trait => self.printer.print(";"),
            None => self.printer.print("{ unimplemented!() }"),
            Some(b) => {
                self.printer.print(" ");
                self.visit(b, arg);
            }
        }
        self.id.set_current_method(None);
        self.in_static_method = false;
        self.mut_borrow_params.clear();
    }

    fn replace_throws(&mut self, throws: &[NodeId], arg: Arg, type_string: &str) {
        self.printer.print("/* ");
        self.printer.print(" throws ");
        for (i, &r) in throws.iter().enumerate() {
            self.visit(r, arg);
            if i + 1 < throws.len() {
                self.printer.print(", ");
            }
        }
        self.printer.print(" */");
        // Java exceptions have no Rust type; use `String` as the error channel
        // (a defined type — avoids undefined `Rc`/`Exception`).
        self.printer.print("Result<");
        self.printer.print(type_string);
        self.printer.print(", String> ");
    }

    fn visit_parameter(&mut self, id: NodeId, arg: Arg) {
        let (typ, vid, is_var_args) = match self.kind(id) {
            Node::Parameter { typ, id: vid, is_var_args, .. } => (typ, vid, is_var_args),
            _ => unreachable!(),
        };
        self.print_java_comment(id, arg);
        self.printer.print(" ");
        self.visit(vid, arg);
        self.printer.print(": ");
        let nullable = self.decl_nullable(vid);
        if is_var_args {
            // Java varargs `T... xs` -> `xs: Vec<T>`.
            self.printer.print("Vec<");
            if let Some(t) = typ {
                self.visit(t, arg);
            }
            self.printer.print(">");
            return;
        }
        let is_primitive = typ
            .map(|t| matches!(self.arena.kind(t), Node::PrimitiveType { .. }))
            .unwrap_or(false);
        if nullable {
            // Option<T> owns its value; no borrow.
            self.printer.print("Option<");
            if let Some(t) = typ {
                self.visit(t, arg);
            }
            self.printer.print(">");
        } else {
            // An interface-typed parameter becomes `&dyn Trait`: implementors
            // coerce at the call site (`&concrete` -> `&dyn Trait`), since we now
            // generate `impl Trait for Class`.
            let is_trait = typ
                .and_then(|t| self.type_simple_name(t))
                .map(|n| self.resolved_is_trait(&n))
                .unwrap_or(false);
            if !is_primitive {
                let needs_mut = match self.arena.kind(vid) {
                    Node::VariableDeclaratorId { name } => self.mut_borrow_params.contains(name),
                    _ => false,
                };
                self.printer.print(if needs_mut { "&mut " } else { "&" });
                // The trait type renders as `dyn Trait` (behind the `&` just
                // printed) rather than the owned `Box<dyn Trait>`.
                self.trait_dyn_ref = is_trait;
            }
            if let Some(t) = typ {
                self.visit(t, arg);
            }
            self.trait_dyn_ref = false;
        }
    }

    /// Does `name` resolve to a *non-generic* trait (interface)? Only those can
    /// be a plain `&dyn Trait` (a generic trait needs its type args).
    fn resolved_is_trait(&self, name: &str) -> bool {
        // Generic traits are included: their type args are kept on the rendered
        // type (non-generic-arg dropping doesn't apply), so `dyn Trait<T>` is
        // well-formed.
        self.resolve_type_sym(name).map(|t| t.kind == "trait").unwrap_or(false)
    }

    /// Does this bound type resolve to a *known non-trait* (a struct/enum/stub)?
    /// Such a bound is invalid in Rust and is dropped. Unresolved bounds are
    /// kept (they may be traits).
    fn bound_is_known_non_trait(&self, c: NodeId) -> bool {
        self.type_simple_name(c)
            .and_then(|n| self.resolve_type_sym(&n))
            .map(|t| t.kind != "trait")
            .unwrap_or(false)
    }

    /// Does this bound's erasure map to a concrete (non-trait) Rust type — a Java
    /// `T extends List<E>` (-> `Vec`), `T extends Object` (-> `Box<dyn Any>`), or
    /// a primitive? Such a bound has no Rust trait meaning and is dropped.
    fn bound_is_std_concrete(&self, c: NodeId) -> bool {
        let Some(simple) = self.type_simple_name(c) else { return false };
        if matches!(simple.as_str(), "Object" | "Class") {
            return true; // rendered as `Box<dyn Any>`
        }
        matches!(
            map_type_name(&simple),
            "Vec" | "Box" | "HashMap" | "HashSet" | "BTreeMap" | "String" | "str" | "i8" | "i16"
                | "i32" | "i64" | "i128" | "u8" | "u16" | "u32" | "u64" | "u128" | "usize"
                | "isize" | "f32" | "f64" | "bool" | "char"
        )
    }

    /// Does this bound contain a bare wildcard (`Foo<?>` -> `Foo<_>`)? A `_`
    /// placeholder is illegal in an item-signature bound, so the bound is dropped.
    fn bound_has_bare_wildcard(&self, c: NodeId) -> bool {
        let mut stack = vec![c];
        while let Some(n) = stack.pop() {
            if let Node::WildcardType { ext: None, sup: None } = self.arena.kind(n) {
                return true;
            }
            for ch in self.arena.children(n) {
                stack.push(ch);
            }
        }
        false
    }

    fn visit_block(&mut self, id: NodeId, arg: Arg) {
        let stmts = match self.kind(id) {
            Node::BlockStmt { stmts } => stmts,
            _ => unreachable!(),
        };
        self.print_orphan_comments_before_this_child_node(id);
        self.print_java_comment(id, arg);
        self.printer.print_ln_s("{");
        self.printer.indent();
        for &s in &stmts {
            self.visit(s, arg);
            self.printer.print_ln();
        }
        self.printer.unindent();
        self.print_orphan_comments_ending(id);
        self.printer.print("}");
    }

    /// Locals declared in one `switch` case and referenced in another (Java
    /// cases share a scope) — `(java name, type node)`, to hoist above the match.
    fn switch_hoist_vars(&self, entries: &[NodeId]) -> Vec<(String, NodeId)> {
        use std::collections::HashMap;
        let mut decls: HashMap<String, (usize, NodeId)> = HashMap::new();
        let mut uses: HashMap<String, Vec<usize>> = HashMap::new();
        for (i, &e) in entries.iter().enumerate() {
            let Node::SwitchEntryStmt { stmts, .. } = self.arena.kind(e) else { continue };
            let mut stack: Vec<NodeId> = stmts.clone();
            while let Some(n) = stack.pop() {
                match self.arena.kind(n) {
                    Node::VariableDeclarationExpr { typ, vars, .. } => {
                        for &v in vars {
                            if let Node::VariableDeclarator { id: vid, .. } = self.arena.kind(v) {
                                if let Node::VariableDeclaratorId { name } = self.arena.kind(*vid) {
                                    decls.entry(name.clone()).or_insert((i, *typ));
                                }
                            }
                        }
                    }
                    Node::NameExpr { name } => uses.entry(name.clone()).or_default().push(i),
                    _ => {}
                }
                for c in self.arena.children(n) {
                    stack.push(c);
                }
            }
        }
        decls
            .into_iter()
            .filter(|(name, (decl_i, _))| {
                uses.get(name).map(|v| v.iter().any(|j| j != decl_i)).unwrap_or(false)
            })
            .map(|(name, (_, typ))| (name, typ))
            .collect()
    }

    fn visit_switch(&mut self, id: NodeId, arg: Arg) {
        let (selector, entries) = match self.kind(id) {
            Node::SwitchStmt { selector, entries } => (selector, entries),
            _ => unreachable!(),
        };
        self.print_java_comment(id, arg);
        // Java `switch` cases share one scope; a local declared in one case and
        // used in another must be hoisted above the `match` (whose arms are
        // separate scopes). Declare them `let mut x: T;` (uninitialized — any
        // definite-assignment issue is ownership, out of scope) in a wrapping
        // block; their in-case declarations become assignments.
        let hoist = self.switch_hoist_vars(&entries);
        if !hoist.is_empty() {
            self.printer.print_ln_s("{");
            self.printer.indent();
            for (name, typ) in &hoist {
                let snake = self.to_snake_if_necessary(name);
                let ty = self.accept_and_cut(*typ, None).trim().to_string();
                self.printer.print_ln_s(&format!("let mut {snake}: {ty};"));
            }
        }
        let saved_hoist = std::mem::replace(
            &mut self.hoisted_switch_vars,
            hoist.iter().map(|(n, _)| n.clone()).collect(),
        );
        // Matching on Java String requires `match sel.as_str() { "a" => ... }`.
        let string_switch = entries.iter().any(|&e| {
            matches!(self.arena.kind(e),
                Node::SwitchEntryStmt { label: Some(l), .. }
                if matches!(self.arena.kind(*l), Node::StringLiteralExpr { .. }))
        });
        self.printer.print("match ");
        if string_switch {
            self.printer.print("(");
            self.visit(selector, arg);
            self.printer.print(").as_str()");
        } else {
            self.visit(selector, arg);
        }
        self.printer.print_ln_s(" {");
        self.printer.indent();
        // If every bare-name case label is a variant of one enum, qualify them as
        // `Enum::Label` patterns (a bare `Label` would be a binding, breaking
        // or-patterns with E0408).
        let label_names: Vec<String> = entries
            .iter()
            .filter_map(|&e| match self.arena.kind(e) {
                Node::SwitchEntryStmt { label: Some(l), .. } => match self.arena.kind(*l) {
                    Node::NameExpr { name } => Some(name.clone()),
                    _ => None,
                },
                _ => None,
            })
            .collect();
        let new_enum_path = self.enum_path_for_labels(&label_names);
        let saved_enum_path = std::mem::replace(&mut self.switch_enum_path, new_enum_path);
        // Java case fall-through: consecutive labels with no body share the body
        // of the next labelled case. In Rust that is an or-pattern
        // `a | b | c => { ... }`. A `default` label becomes `_`.
        let mut pending: Vec<NodeId> = Vec::new(); // accumulated case-label exprs
        let mut pending_default = false;
        for &e in &entries {
            let (label, stmts) = match self.arena.kind(e) {
                Node::SwitchEntryStmt { label, stmts } => (*label, stmts.clone()),
                _ => continue,
            };
            match label {
                Some(l) => pending.push(l),
                None => pending_default = true,
            }
            if stmts.is_empty() {
                continue; // fall through to the next case
            }
            // Emit the accumulated pattern.
            self.emit_switch_patterns(&pending, pending_default, arg);
            self.printer.print_ln_s(" => {");
            self.printer.indent();
            // Drop a trailing unlabeled `break;` — in Java it terminates the
            // case, but a Rust `match` arm has no `break` (E0268).
            let mut body = stmts.clone();
            if matches!(body.last().map(|&s| self.arena.kind(s)), Some(Node::BreakStmt { id: None })) {
                body.pop();
            }
            for &s in &body {
                self.visit(s, arg);
                self.printer.print_ln();
            }
            self.printer.unindent();
            self.printer.print_ln_s("}");
            pending.clear();
            pending_default = false;
        }
        // Trailing labels with no body (e.g. an empty final case).
        if pending_default || !pending.is_empty() {
            self.emit_switch_patterns(&pending, pending_default, arg);
            self.printer.print_ln_s(" => {}");
        }
        self.switch_enum_path = saved_enum_path;
        self.printer.unindent();
        self.printer.print("}");
        // Close the hoist wrapping block.
        self.hoisted_switch_vars = saved_hoist;
        if !hoist.is_empty() {
            self.printer.unindent();
            self.printer.print_ln();
            self.printer.print("}");
        }
    }

    /// Emit a match pattern: `_` for default, else `p1 | p2 | …`. String labels
    /// are emitted raw (no `.to_string()`).
    fn emit_switch_patterns(&mut self, pending: &[NodeId], default: bool, arg: Arg) {
        if default {
            self.printer.print("_");
            return;
        }
        let saved = self.raw_string;
        self.raw_string = true;
        for (i, &l) in pending.iter().enumerate() {
            if i > 0 {
                self.printer.print(" | ");
            }
            // Qualify a bare enum-variant label as `Enum::Label` (a unit-variant
            // pattern), rather than emitting a binding.
            let qualified = match (self.switch_enum_path.as_deref(), self.arena.kind(l)) {
                (Some(path), Node::NameExpr { name }) => Some(format!("{path}::{name}")),
                _ => None,
            };
            match qualified {
                Some(p) => self.printer.print(&p),
                None => self.visit(l, arg),
            }
        }
        self.raw_string = saved;
    }

    fn visit_enum(&mut self, id: NodeId, arg: Arg) {
        let (modifiers_v, name, implements, entries, members) = match self.kind(id) {
            Node::EnumDeclaration { modifiers, name, implements, entries, members } => {
                (modifiers, name, implements, entries, members)
            }
            _ => unreachable!(),
        };
        self.print_java_comment(id, arg);
        self.emit_provenance(&self.java_type_fqn(id));
        self.print_modifiers(modifiers_v);
        self.printer.print("enum ");
        self.printer.print(&name);
        let _ = &implements; // `implements` has no Rust enum equivalent; dropped.
        self.printer.print_ln_s(" {");
        self.printer.indent();
        // Variants only. Java enum fields/constructors/methods have no direct
        // Rust enum equivalent and are dropped.
        let _ = &members;
        for &e in &entries {
            self.visit(e, arg);
            self.printer.print_ln_s(",");
        }
        self.printer.unindent();
        self.printer.print_ln_s("}");
    }

    fn visit_enum_constant(&mut self, id: NodeId, arg: Arg) {
        let (name, args, class_body) = match self.kind(id) {
            Node::EnumConstantDeclaration { name, args, class_body } => (name, args, class_body),
            _ => unreachable!(),
        };
        // A Rust enum variant is just a name; Java enum constructor args and
        // per-constant class bodies have no equivalent and are dropped.
        let _ = (&args, &class_body);
        self.print_java_comment(id, arg);
        self.printer.print(&name);
    }

    fn visit_if(&mut self, id: NodeId, arg: Arg) {
        let (condition, then_stmt, else_stmt) = match self.kind(id) {
            Node::IfStmt { condition, then_stmt, else_stmt } => (condition, then_stmt, else_stmt),
            _ => unreachable!(),
        };
        self.print_java_comment(id, arg);
        self.printer.print("if ");
        self.visit(condition, arg);
        let then_block = matches!(self.arena.kind(then_stmt), Node::BlockStmt { .. });
        if then_block {
            self.printer.print(" ");
        } else {
            self.printer.print_ln_s(" {");
            self.printer.indent();
        }
        self.visit(then_stmt, arg);
        if !then_block {
            self.printer.unindent();
            self.printer.print_ln();
            self.printer.print_ln_s("}");
        }
        if let Some(else_s) = else_stmt {
            if then_block {
                self.printer.print(" ");
            }
            let else_if = matches!(self.arena.kind(else_s), Node::IfStmt { .. });
            let else_block = matches!(self.arena.kind(else_s), Node::BlockStmt { .. });
            if else_if || else_block {
                self.printer.print("else ");
            } else {
                self.printer.print("else {");
                self.printer.indent();
            }
            self.visit(else_s, arg);
            if !(else_if || else_block) {
                self.printer.unindent();
                self.printer.print_ln();
                self.printer.print_ln_s("}");
            }
        }
    }

    fn encapsulate_if_not_block(&mut self, n: NodeId, arg: Arg) {
        if matches!(self.arena.kind(n), Node::BlockStmt { .. }) {
            self.visit(n, arg);
        } else {
            self.printer.print_ln_s(" {");
            self.printer.indent();
            self.visit(n, arg);
            self.printer.print_ln_s("}");
        }
    }

    fn visit_for(&mut self, id: NodeId, arg: Arg) {
        let (init, compare, update, body) = match self.kind(id) {
            Node::ForStmt { init, compare, update, body } => (init, compare, update, body),
            _ => unreachable!(),
        };
        self.print_java_comment(id, arg);
        if !init.is_empty() {
            self.printer.print_ln_s(" {");
            self.printer.indent();
            for &e in &init {
                self.visit(e, arg);
                self.printer.print_ln_s(";");
            }
        }

        // If the body `continue`s, the naive `while cond { body; update }` form
        // would skip `update` (Java runs the update on `continue`). Move the
        // update into the loop condition, guarded so it doesn't run on the first
        // iteration — this reproduces C-`for` semantics: update runs before the
        // condition on every iteration after the first, and `continue` jumps to
        // the condition (so it runs the update too).
        let needs_continue_safe = !update.is_empty() && self.body_has_unlabeled_continue(body);

        if needs_continue_safe {
            self.printer.print_ln_s("let mut __first = true;");
            self.printer.print("while { if !__first { ");
            for &e in &update {
                self.visit(e, arg);
                self.printer.print("; ");
            }
            self.printer.print("} __first = false; ");
            match compare {
                Some(c) => self.visit(c, arg),
                None => self.printer.print("true"),
            }
            self.printer.print(" } ");
            self.encapsulate_if_not_block(body, arg);
            self.printer.print_ln_s("");
        } else {
            if let Some(c) = compare {
                self.printer.print("while ");
                self.visit(c, arg);
            } else {
                self.printer.print("loop ");
            }
            if !update.is_empty() {
                self.printer.print_ln_s(" {");
                self.printer.indent();
            }
            self.encapsulate_if_not_block(body, arg);
            self.printer.print_ln_s("");
            if !update.is_empty() {
                for &e in &update {
                    self.visit(e, arg);
                    self.printer.print_ln_s(";");
                }
                self.printer.unindent();
                self.printer.print_ln_s(" }");
            }
        }

        if !init.is_empty() {
            self.printer.unindent();
            self.printer.print_ln_s(" }");
        }
    }

    /// Does `body` contain an unlabeled `continue` that targets *this* loop (i.e.
    /// not one nested inside another loop)? Used to pick a `continue`-safe `for`
    /// lowering.
    fn body_has_unlabeled_continue(&self, body: NodeId) -> bool {
        let mut stack = vec![body];
        while let Some(n) = stack.pop() {
            match self.arena.kind(n) {
                Node::ContinueStmt { id: None } => return true,
                // Don't descend into a nested loop — its `continue` targets it.
                Node::ForStmt { .. }
                | Node::WhileStmt { .. }
                | Node::DoStmt { .. }
                | Node::ForeachStmt { .. }
                    if n != body =>
                {
                    continue;
                }
                _ => {
                    for c in self.arena.children(n) {
                        stack.push(c);
                    }
                }
            }
        }
        false
    }

    fn visit_try(&mut self, id: NodeId, arg: Arg) {
        let (resources, try_block, catchs, finally_block) = match self.kind(id) {
            Node::TryStmt { resources, try_block, catchs, finally_block } => {
                (resources, try_block, catchs, finally_block)
            }
            _ => unreachable!(),
        };
        // Rust has no try/catch. Run the resource bindings + try body in a scope,
        // then the finally body. Catch clauses (the error path) are dropped.
        self.print_java_comment(id, arg);
        if !resources.is_empty() {
            self.printer.print_ln_s("{");
            self.printer.indent();
            for &r in &resources {
                self.visit(r, arg);
                self.printer.print_ln_s(";");
            }
            self.visit(try_block, arg);
            self.printer.print_ln();
            self.printer.unindent();
            self.printer.print("}");
        } else {
            self.visit(try_block, arg);
        }
        if !catchs.is_empty() {
            self.printer.print(" /* catch clauses omitted */");
        }
        if let Some(f) = finally_block {
            self.printer.print(" ");
            self.visit(f, arg);
        }
    }

    fn visit_lambda(&mut self, id: NodeId, arg: Arg) {
        let (parameters, body, parameters_enclosed) = match self.kind(id) {
            Node::LambdaExpr { parameters, body, parameters_enclosed } => {
                (parameters, body, parameters_enclosed)
            }
            _ => unreachable!(),
        };
        // Rust closure: |params| body  (parameter types are inferred).
        self.print_java_comment(id, arg);
        let _ = parameters_enclosed;
        self.printer.print("|");
        for (i, &p) in parameters.iter().enumerate() {
            let name = self.param_name(p);
            self.printer.print(&name);
            if i + 1 < parameters.len() {
                self.printer.print(", ");
            }
        }
        self.printer.print("| ");
        if let Node::ExpressionStmt { expression } = self.arena.kind(body) {
            let e = *expression;
            self.visit(e, arg);
        } else {
            self.visit(body, arg);
        }
    }

    /// The (snake-cased) name of a Parameter's declarator id.
    fn param_name(&self, p: NodeId) -> String {
        if let Node::Parameter { id, .. } = self.arena.kind(p) {
            if let Node::VariableDeclaratorId { name } = self.arena.kind(*id) {
                return self.to_snake_if_necessary(name);
            }
        }
        String::new()
    }

    /// If a method-reference scope is a bare name that resolves to a value
    /// (local/param/field) rather than a type, the receiver expression to use in
    /// the lowered closure (`name`, or `self.name` for a field). `None` for a
    /// genuine `Type::method`.
    fn method_ref_value_recv(&self, s: NodeId) -> Option<String> {
        let Node::TypeExpr { typ: Some(t) } = self.arena.kind(s) else { return None };
        let Node::ClassOrInterfaceType { scope: None, name, .. } = self.arena.kind(*t) else {
            return None;
        };
        let (_, decl) = self.id.find_declaration_node_for(self.arena, name, s)?;
        if matches!(
            self.arena.kind(decl),
            Node::ClassOrInterfaceDeclaration { .. } | Node::EnumDeclaration { .. }
        ) {
            return None; // a real type reference
        }
        let snake = self.to_snake_if_necessary(name);
        if self.is_non_static_field_declaration(decl) {
            Some(format!("{}.{snake}", self.self_receiver()))
        } else {
            Some(snake)
        }
    }

    fn visit_method_ref(&mut self, id: NodeId, arg: Arg) {
        let (scope, type_arguments, identifier) = match self.kind(id) {
            Node::MethodReferenceExpr { scope, type_arguments, identifier } => {
                (scope, type_arguments, identifier)
            }
            _ => unreachable!(),
        };
        self.print_java_comment(id, arg);
        let _ = &type_arguments;
        let ident = self.to_snake_if_necessary(&identifier);
        match scope {
            // `value::method` where the receiver only *looks* like a type (a bare
            // name resolving to a local/param/field): lower to a closure, not a
            // path (`name::ends_with` -> `|__mr| name.ends_with(__mr)`).
            Some(s) if self.method_ref_value_recv(s).is_some() => {
                let recv = self.method_ref_value_recv(s).unwrap();
                self.printer.print(&format!("|__mr| {recv}.{ident}(__mr)"));
            }
            // `Type::method` — a path (valid as a function value). A generic
            // type needs turbofish in a path: `Vec::<T>::new`, not `Vec<T>::new`.
            Some(s) if matches!(self.arena.kind(s), Node::TypeExpr { .. }) => {
                let ty = self.accept_and_cut(s, arg);
                let ty = match ty.find('<') {
                    Some(i) => format!("{}::{}", &ty[..i], &ty[i..]),
                    None => ty,
                };
                self.printer.print(&ty);
                self.printer.print("::");
                self.printer.print(&ident);
            }
            // `expr::method` on a value — lower to a one-arg closure.
            Some(s) => {
                self.printer.print("|__mr| ");
                self.visit(s, arg);
                self.printer.print(".");
                self.printer.print(&ident);
                self.printer.print("(__mr)");
            }
            None => {
                self.printer.print("::");
                self.printer.print(&ident);
            }
        }
    }

    // ---- comments / position ----

    fn annotation_simple_name(&self, name: NodeId) -> String {
        match self.arena.kind(name) {
            Node::NameExpr { name } => name.clone(),
            Node::QualifiedNameExpr { name, .. } => name.clone(),
            _ => String::new(),
        }
    }

    fn is_non_static_field_declaration(&self, n: NodeId) -> bool {
        if matches!(self.arena.kind(n), Node::VariableDeclaratorId { .. }) {
            if let Some(p) = self.arena.parent(n) {
                if let Some(g) = self.arena.parent(p) {
                    if let Node::FieldDeclaration { modifiers, .. } = self.arena.kind(g) {
                        return !modifiers::is_static(*modifiers);
                    }
                }
            }
        }
        false
    }

    fn is_static_field_declaration(&self, n: NodeId) -> bool {
        if matches!(self.arena.kind(n), Node::VariableDeclaratorId { .. }) {
            if let Some(p) = self.arena.parent(n) {
                if let Some(g) = self.arena.parent(p) {
                    if let Node::FieldDeclaration { modifiers, .. } = self.arena.kind(g) {
                        return modifiers::is_static(*modifiers);
                    }
                }
            }
        }
        false
    }

    fn is_non_static_method_declaration(&self, n: NodeId) -> bool {
        if let Node::MethodDeclaration { modifiers, .. } = self.arena.kind(n) {
            !modifiers::is_static(*modifiers)
        } else {
            false
        }
    }

    fn stop_history_search(&self, n: NodeId) -> bool {
        matches!(
            self.arena.kind(n),
            Node::VariableDeclarator { .. }
                | Node::MethodCallExpr { .. }
                | Node::ArrayAccessExpr { .. }
        ) || self.is_statement(n)
    }

    fn is_statement(&self, n: NodeId) -> bool {
        matches!(
            self.arena.kind(n),
            Node::BlockStmt { .. }
                | Node::ExpressionStmt { .. }
                | Node::ReturnStmt { .. }
                | Node::IfStmt { .. }
                | Node::WhileStmt { .. }
                | Node::DoStmt { .. }
                | Node::ForStmt { .. }
                | Node::ForeachStmt { .. }
                | Node::BreakStmt { .. }
                | Node::ContinueStmt { .. }
                | Node::ThrowStmt { .. }
                | Node::TryStmt { .. }
                | Node::SwitchStmt { .. }
                | Node::SwitchEntryStmt { .. }
                | Node::LabeledStmt { .. }
                | Node::AssertStmt { .. }
                | Node::SynchronizedStmt { .. }
                | Node::EmptyStmt
                | Node::TypeDeclarationStmt { .. }
                | Node::ExplicitConstructorInvocationStmt { .. }
                | Node::CatchClause { .. }
        )
    }

    fn is_float_in_siblings(&self, n: NodeId) -> bool {
        let parent = match self.arena.parent(n) {
            Some(p) => p,
            None => return false,
        };
        if self.stop_history_search(parent) {
            return false;
        }
        for sibling in self.arena.children(parent) {
            if self.id.is_float_node(Some(sibling)) {
                return true;
            }
        }
        false
    }

    fn is_float_in_history(&self, n: Option<NodeId>) -> bool {
        let id = match n {
            Some(x) => x,
            None => return false,
        };
        if self.stop_history_search(id) {
            return false;
        }
        if self.is_float_in_siblings(id) {
            return true;
        }
        let clazz = self.id.get_type(id);
        if clazz.map(IdTracker::is_float_class).unwrap_or(false) {
            true
        } else {
            self.is_float_in_history(self.arena.parent(id))
        }
    }

    fn sort_children_by_begin(&self, parent: NodeId) -> Vec<NodeId> {
        let mut everything = self.arena.children(parent);
        everything.sort_by(|&a, &b| {
            let pa = self.arena.begin(a);
            let pb = self.arena.begin(b);
            (pa.line, pa.column).cmp(&(pb.line, pb.column))
        });
        everything
    }

    fn is_comment(&self, n: NodeId) -> bool {
        matches!(
            self.arena.kind(n),
            Node::LineComment { .. } | Node::BlockComment { .. } | Node::JavadocComment { .. }
        )
    }

    fn print_orphan_comments_before_this_child_node(&mut self, node: NodeId) {
        if self.is_comment(node) {
            return;
        }
        let parent = match self.arena.parent(node) {
            Some(p) => p,
            None => return,
        };
        let everything = self.sort_children_by_begin(parent);
        let mut pos_child: i64 = -1;
        for (i, &e) in everything.iter().enumerate() {
            if e == node {
                pos_child = i as i64;
            }
        }
        if pos_child == -1 {
            // Should not happen given parent links; be lenient.
            return;
        }
        let mut pos_prev: i64 = -1;
        let mut i = pos_child - 1;
        while i >= 0 && pos_prev == -1 {
            if !self.is_comment(everything[i as usize]) {
                pos_prev = i;
            }
            i -= 1;
        }
        for i in (pos_prev + 1)..pos_child {
            let to_print = everything[i as usize];
            if self.is_comment(to_print) {
                self.visit(to_print, None);
            }
        }
    }

    fn print_orphan_comments_ending(&mut self, node: NodeId) {
        let everything = self.sort_children_by_begin(node);
        if everything.is_empty() {
            return;
        }
        let mut comments_at_end = 0usize;
        let mut finding = true;
        while finding && comments_at_end < everything.len() {
            let last = everything[everything.len() - 1 - comments_at_end];
            finding = self.is_comment(last);
            if finding {
                comments_at_end += 1;
            }
        }
        for i in 0..comments_at_end {
            let c = everything[everything.len() - comments_at_end + i];
            self.visit(c, None);
        }
    }
}

/// Convert Java string/char escape sequences to Rust ones. Java `\uXXXX`
/// becomes Rust `\u{XXXX}`; other common escapes are identical.
/// Neutralize `/*` and `*/` inside a comment body: Rust block comments *nest*
/// (Java's don't), so a Java comment containing `/*` would open an unbalanced
/// nested comment (leaving the outer one unterminated).
fn sanitize_block_comment(content: &str) -> String {
    content.replace("/*", "/ *").replace("*/", "* /")
}

fn java_escapes_to_rust(s: &str) -> String {
    let bytes: Vec<char> = s.chars().collect();
    let mut out = String::new();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == '\\' && i + 1 < bytes.len() {
            let c = bytes[i + 1];
            if c == 'u' {
                // \uXXXX -> \u{XXXX}
                let hex: String = bytes[i + 2..].iter().take(4).collect();
                if hex.len() == 4 && hex.chars().all(|c| c.is_ascii_hexdigit()) {
                    out.push_str(&format!("\\u{{{hex}}}"));
                    i += 6;
                    continue;
                }
            }
            // Java octal escape `\ooo` (1-3 octal digits) -> `\u{hex}` (Rust has
            // no octal string escapes).
            if ('0'..='7').contains(&c) {
                let mut digits = String::new();
                let mut j = i + 1;
                while j < bytes.len() && digits.len() < 3 && ('0'..='7').contains(&bytes[j]) {
                    digits.push(bytes[j]);
                    j += 1;
                }
                if let Ok(v) = u32::from_str_radix(&digits, 8) {
                    // Prefer `\xNN` for ASCII (no braces — safe inside a
                    // `format!` literal, where `{`/`}` would be doubled).
                    if v <= 0x7f {
                        out.push_str(&format!("\\x{v:02x}"));
                    } else {
                        out.push_str(&format!("\\u{{{v:x}}}"));
                    }
                    i = j;
                    continue;
                }
            }
            // Java-only escapes Rust lacks: \b (backspace), \f (form feed).
            if c == 'b' || c == 'f' {
                out.push_str(if c == 'b' { "\\u{8}" } else { "\\u{c}" });
                i += 2;
                continue;
            }
            // Valid-in-both escapes (\n \r \t \\ \" \' \0) kept verbatim.
            out.push(bytes[i]);
            out.push(c);
            i += 2;
            continue;
        }
        out.push(bytes[i]);
        i += 1;
    }
    out
}

/// Convert a Java hex floating-point literal (`0x1.8p3`) to a decimal Rust
/// literal. Returns None if `s` is not a hex float.
fn hex_float_to_decimal(s: &str) -> Option<String> {
    let s = s.trim_end_matches(['f', 'F', 'd', 'D']);
    let lower = s.to_ascii_lowercase();
    let body = lower.strip_prefix("0x")?;
    let (mant, exp) = body.split_once('p')?;
    let exp: i32 = exp.parse().ok()?;
    let (ip, fp) = mant.split_once('.').unwrap_or((mant, ""));
    let mut val = 0f64;
    for c in ip.chars() {
        val = val * 16.0 + c.to_digit(16)? as f64;
    }
    let mut scale = 1.0 / 16.0;
    for c in fp.chars() {
        val += c.to_digit(16)? as f64 * scale;
        scale /= 16.0;
    }
    val *= 2f64.powi(exp);
    Some(format!("{val:e}"))
}

/// If `s` is a Rust keyword, escape it as a raw identifier (`r#s`). A few
/// keywords (`self`/`super`/`crate`/`Self`) cannot be raw, but they are also
/// reserved in Java so never appear as user identifiers.
/// A constant-shaped name: all uppercase/digits/underscore, with at least one
/// letter (`UNKNOWN_SEQUENCE_LENGTH`, `MD5_TAG`).
fn is_const_name(name: &str) -> bool {
    name.chars().any(|c| c.is_ascii_alphabetic())
        && name.chars().all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '_')
}

/// Escape each `::`-segment of a path so it's a valid Rust path element: `$`
/// (synthetic/nested names) becomes `_`, and Rust keywords are raw-escaped.
pub fn sanitize_path_segments(path: &str) -> String {
    path.split("::")
        .map(|s| escape_rust_keyword(s.replace('$', "_")))
        .collect::<Vec<_>>()
        .join("::")
}

pub fn escape_rust_keyword(s: String) -> String {
    const KW: &[&str] = &[
        "as", "break", "const", "continue", "dyn", "else", "enum", "extern", "false", "fn",
        "for", "if", "impl", "in", "let", "loop", "match", "mod", "move", "mut", "pub", "ref",
        "return", "static", "struct", "trait", "true", "type", "unsafe", "use", "where", "while",
        "async", "await", "box", "do", "final", "macro", "override", "priv", "try", "typeof",
        "unsized", "virtual", "yield", "abstract", "become", "gen",
    ];
    if KW.contains(&s.as_str()) {
        format!("r#{s}")
    } else {
        s
    }
}

/// Convert a Java `String.format`/`printf` format string to a Rust `format!`
/// template: `%d`/`%s`/`%.2f`/… → `{}`, `%n` → newline, `%%` → `%`. Literal
/// braces are escaped.
/// Count `{}`-style placeholders in a Rust format string, skipping the escaped
/// `{{` / `}}`.
fn count_fmt_placeholders(s: &str) -> usize {
    let b = s.as_bytes();
    let mut i = 0;
    let mut n = 0;
    while i < b.len() {
        if b[i] == b'{' {
            if i + 1 < b.len() && b[i + 1] == b'{' {
                i += 2;
                continue;
            }
            n += 1;
        } else if b[i] == b'}' && i + 1 < b.len() && b[i + 1] == b'}' {
            i += 2;
            continue;
        }
        i += 1;
    }
    n
}

fn java_format_to_rust(fmt: &str) -> String {
    let mut out = String::new();
    let mut chars = fmt.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '{' => out.push_str("{{"),
            '}' => out.push_str("}}"),
            '%' => {
                if chars.peek() == Some(&'%') {
                    chars.next();
                    out.push('%');
                    continue;
                }
                // Skip flags / width / precision / argument-index.
                while matches!(
                    chars.peek(),
                    Some('0'..='9') | Some('$') | Some('.') | Some(',') | Some('+')
                        | Some('-') | Some(' ') | Some('#') | Some('(')
                ) {
                    chars.next();
                }
                // The conversion character.
                match chars.next() {
                    Some('n') => out.push_str("\\n"),
                    _ => out.push_str("{}"),
                }
            }
            other => out.push(other),
        }
    }
    out
}

/// Map a Java type simple name to its Rust equivalent (collections, boxed
/// primitives). Returns the name unchanged if there is no mapping.
pub fn map_type_name(name: &str) -> &str {
    // Use the simple name (drop any `pkg.` qualifier) for the mapping.
    let name = name.rsplit('.').next().unwrap_or(name);
    match name {
        "List" | "ArrayList" | "LinkedList" | "Collection" | "AbstractList" | "Vector"
        | "Stack" | "Queue" | "Deque" | "ArrayDeque" | "Iterable" => "Vec",
        "Map" | "HashMap" | "LinkedHashMap" | "TreeMap" | "SortedMap" | "NavigableMap"
        | "AbstractMap" => "std::collections::HashMap",
        "Set" | "HashSet" | "LinkedHashSet" | "TreeSet" | "SortedSet" | "NavigableSet"
        | "AbstractSet" => "std::collections::HashSet",
        "Optional" => "Option",
        "Integer" => "i32",
        "Long" => "i64",
        "Short" => "i16",
        "Byte" => "i8",
        "Double" => "f64",
        "Float" => "f32",
        "Boolean" => "bool",
        "Character" => "char",
        // Common java.lang types with no plain identifier mapping. These have no
        // import (auto-imported), so they must be mapped here by simple name.
        "Void" => "()",
        "StringBuilder" | "StringBuffer" | "CharSequence" => "String",
        "Number" => "f64",
        // Exceptions have no Rust equivalent; the throws channel uses `String`
        // (see `replace_throws`), so an exception-typed value maps the same way.
        "Exception" | "Throwable" | "Error" | "RuntimeException" => "String",
        other => other,
    }
}

/// Does this (already-mapped) Rust type name name a collection constructed with
/// `::new()` (no arguments)?
fn is_rust_collection(mapped: &str) -> bool {
    matches!(mapped, "Vec" | "std::collections::HashMap" | "std::collections::HashSet")
}

/// Mirrors `StringUtils.endsWithAny` for a single non-null suffix.
fn ends_with_ignore_none(value: &str, suffix: &str) -> bool {
    value.ends_with(suffix)
}
