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
    /// Array declarations whose *elements* are nullable: render `Vec<Option<T>>`
    /// and unwrap/`Some`-wrap element reads/assigns. See `array_elem_nullable`.
    elem_nullable: Option<&'a std::collections::HashSet<NodeId>>,
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
    /// Whether the method currently being emitted mutates `self` (needs
    /// `&mut self`), per the borrow analysis. Set in `visit_method`.
    method_recv_mut: bool,
    /// Names of the current method's parameters reassigned in the body, which
    /// need a `mut` binding. Set in `visit_method`, read by `visit_parameter`.
    reassigned_params: std::collections::HashSet<String>,
    /// A tail expression the next block must emit before its closing brace —
    /// used to append `Ok(())` to a `void`-but-`throws` method body (whose
    /// return type is `Result<(), String>`). Consumed by the first `visit_block`.
    pending_tail: Option<String>,
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
    /// R4: every hierarchy member's `rust_path` → (its `<Root>Kind` enum path,
    /// this member's variant name, is-root). Built once; drives slot routing
    /// (root slots → enum), construction-wrap, cast-extract, and instanceof.
    slot_enum_cache: std::sync::OnceLock<std::collections::HashMap<String, (String, String, bool)>>,
    /// Tier-2: per-file inferred element `Type` for RAW collection declarations,
    /// keyed by the declaration's type-node id. Built once (lazily) from
    /// `.add`/initializer evidence; shared with the resolver so render and
    /// `type_of` agree on the element.
    collection_elem: std::cell::OnceCell<std::rc::Rc<std::collections::HashMap<NodeId, crate::types::Type>>>,
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
            elem_nullable: None,
            expect_option: false,
            raw_string: false,
            link,
            mut_borrow_params: std::collections::HashSet::new(),
            emit_stubs: false,
            crate_mode: false,
            trait_dyn_ref: false,
            trait_bound_pos: false,
            in_static_method: false,
            method_recv_mut: false,
            reassigned_params: std::collections::HashSet::new(),
            pending_tail: None,
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
            collection_elem: std::cell::OnceCell::new(),
            slot_enum_cache: std::sync::OnceLock::new(),
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

    /// Is `member` an inherited (superclass) field that is nullable? Walks the
    /// parent chain in the symbol map, mirroring [`Self::inherited_field`].
    fn inherited_field_nullable(&self, member: &str) -> bool {
        let Some(mut t) = self.current_class_fqn.as_deref().and_then(|f| self.link.lookup(f))
        else {
            return false;
        };
        while let Some(parent) = t.parent.as_deref() {
            let Some(pt) = self.link.lookup(parent) else { return false };
            if let Some(f) = pt.fields.get(member) {
                return f.nullable;
            }
            t = pt;
        }
        false
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

    /// The crate path of the ancestor that declares an inherited method `member`
    /// (for qualifying an inherited *static* call, which can't dispatch through
    /// `Deref`). Walks the linked project map's parent chain.
    fn inherited_method_owner(&self, member: &str) -> Option<String> {
        let mut t = self.link.lookup(self.current_class_fqn.as_deref()?)?;
        while let Some(parent) = t.parent.as_deref() {
            let pt = self.link.lookup(parent)?;
            if pt.methods.contains_key(member) {
                return Some(self.crate_relativize(&pt.rust_path));
            }
            t = pt;
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
    /// The enclosing type *declaration* node (class/interface/enum) of `n`.
    fn owner_type_decl(&self, n: NodeId) -> Option<NodeId> {
        let mut cur = self.arena.parent(n)?;
        loop {
            if matches!(
                self.arena.kind(cur),
                Node::ClassOrInterfaceDeclaration { .. } | Node::EnumDeclaration { .. }
            ) {
                return Some(cur);
            }
            cur = self.arena.parent(cur)?;
        }
    }

    /// The declared (Java) name of a type declaration node.
    fn type_decl_name(&self, decl: NodeId) -> Option<String> {
        match self.arena.kind(decl) {
            Node::ClassOrInterfaceDeclaration { name, .. } | Node::EnumDeclaration { name, .. } => {
                Some(name.clone())
            }
            _ => None,
        }
    }

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

    pub fn set_elem_nullable(&mut self, e: &'a std::collections::HashSet<NodeId>) {
        self.elem_nullable = Some(e);
    }

    /// Is the declaration `decl` an array whose elements are nullable
    /// (`Vec<Option<T>>`)?
    fn decl_elem_nullable(&self, decl: NodeId) -> bool {
        self.elem_nullable.is_some_and(|s| s.contains(&decl))
    }

    /// Is `expr` an element read `base[i]` of an element-nullable array?
    fn array_access_elem_nullable(&self, expr: NodeId) -> bool {
        let Node::ArrayAccessExpr { name, .. } = self.arena.kind(expr) else {
            return false;
        };
        let ident = match self.arena.kind(*name) {
            Node::NameExpr { name } => name,
            Node::FieldAccessExpr { field, .. } => field,
            _ => return false,
        };
        self.id
            .find_declaration_node_for(self.arena, ident, *name)
            .map(|(_, d)| self.decl_elem_nullable(d))
            .unwrap_or(false)
    }

    /// `(can_derive_PartialEq, can_derive_Eq+Hash)` for a *rendered Rust type
    /// string*, considering only types that derive these unconditionally: this
    /// avoids any cross-struct dependency (so the per-struct decision can't
    /// cascade). A struct/enum path, trait object, or unknown ident returns
    /// `(false, false)` — conservatively excluded even when it would in fact
    /// derive. Floats are `PartialEq` but not `Eq`/`Hash`.
    /// Emit synthesized `impl PartialEq`/`Eq`/`Hash` (see call site). Only fires
    /// for a non-generic, non-`manual_impls` struct that did NOT `#[derive]` the
    /// trait but is capability-flagged in the symbol map.
    #[allow(clippy::too_many_arguments)]
    fn emit_synth_eq_impls(
        &mut self,
        name: &str,
        members: &[NodeId],
        has_base: bool,
        derived_pe: bool,
        derived_eh: bool,
        manual_impls: bool,
        non_generic: bool,
    ) {
        if manual_impls || !non_generic {
            return;
        }
        let (cap_pe, cap_eh) = self
            .current_class_fqn
            .as_deref()
            .and_then(|f| self.link.lookup(f))
            .map(|t| (t.partial_eq_capable, t.eq_hash_capable))
            .unwrap_or((false, false));
        let want_eh = !derived_eh && cap_eh;
        let want_pe = (!derived_pe && cap_pe) || want_eh;
        if !want_pe {
            return;
        }
        // (field ident, is-top-level-map/set, is-Option-wrapped) in declaration order.
        let mut fields: Vec<(String, bool, bool)> = Vec::new();
        if has_base {
            fields.push(("base".to_string(), false, false));
        }
        for &m in members {
            let (typ, vars) = match self.arena.kind(m) {
                Node::FieldDeclaration { modifiers, variables, typ, .. } => {
                    if modifiers::is_static(*modifiers) {
                        continue;
                    }
                    (*typ, variables.clone())
                }
                _ => continue,
            };
            let ty = self.accept_and_cut(typ, None);
            // `accept_and_cut` yields the bare type; `Option<…>` (nullable) and
            // `Vec<…>` (array dims) are added at field emission, so detect those
            // from the declarator, not the string.
            let core = ty.trim().trim_start_matches("std::collections::");
            let bare_is_map_set = core.starts_with("HashMap<")
                || core.starts_with("HashSet<")
                || core.starts_with("BTreeMap<")
                || core.starts_with("BTreeSet<");
            for &var in &vars {
                let array = matches!(self.arena.kind(var), Node::VariableDeclarator { array_count, .. } if *array_count > 0);
                // A nullable map/set is `Option<map>` → fold via `.iter().flatten()`.
                let opt = self.var_decl_id(var).map(|d| self.decl_nullable(d)).unwrap_or(false);
                let is_map_set = bare_is_map_set && !array;
                fields.push((self.field_var_name(var), is_map_set, is_map_set && opt));
            }
        }
        // impl PartialEq
        self.printer.print_ln_s(&format!("impl PartialEq for {name} {{"));
        self.printer.indent();
        self.printer.print_ln_s("fn eq(&self, other: &Self) -> bool {");
        self.printer.indent();
        if fields.is_empty() {
            self.printer.print_ln_s("true");
        } else {
            let conj = fields
                .iter()
                .map(|(f, _, _)| format!("self.{f} == other.{f}"))
                .collect::<Vec<_>>()
                .join(" && ");
            self.printer.print_ln_s(&conj);
        }
        self.printer.unindent();
        self.printer.print_ln_s("}");
        self.printer.unindent();
        self.printer.print_ln_s("}");
        if want_eh {
            self.printer.print_ln_s(&format!("impl Eq for {name} {{}}"));
            self.printer.print_ln_s(&format!("impl std::hash::Hash for {name} {{"));
            self.printer.indent();
            self.printer.print_ln_s("fn hash<H: std::hash::Hasher>(&self, state: &mut H) {");
            self.printer.indent();
            for (f, is_map_set, opt) in &fields {
                if *is_map_set {
                    // Order-independent fold (mirrors Java `Map`/`Set.hashCode()`),
                    // since std maps/sets don't implement `Hash`. A nullable
                    // map/set (`Option<…>`) folds via `.iter().flatten()`.
                    let iter = if *opt {
                        format!("self.{f}.iter().flatten()")
                    } else {
                        format!("self.{f}.iter()")
                    };
                    self.printer.print_ln_s(&format!(
                        "{{ let mut __acc: u64 = 0; for __e in {iter} {{ let mut __h = std::collections::hash_map::DefaultHasher::new(); std::hash::Hash::hash(&__e, &mut __h); __acc = __acc.wrapping_add(std::hash::Hasher::finish(&__h)); }} std::hash::Hash::hash(&__acc, state); }}"
                    ));
                } else {
                    self.printer.print_ln_s(&format!("std::hash::Hash::hash(&self.{f}, state);"));
                }
            }
            self.printer.unindent();
            self.printer.print_ln_s("}");
            self.printer.unindent();
            self.printer.print_ln_s("}");
        }
    }

    fn type_derives_eq(ty: &str) -> (bool, bool) {
        let ty = ty.trim();
        for w in ["Vec<", "Option<", "Box<"] {
            if let Some(inner) = ty.strip_prefix(w).and_then(|s| s.strip_suffix('>')) {
                if w == "Box<" && inner.trim_start().starts_with("dyn ") {
                    return (false, false);
                }
                return Self::type_derives_eq(inner);
            }
        }
        match ty {
            "i8" | "i16" | "i32" | "i64" | "u8" | "u16" | "u32" | "u64" | "usize" | "isize"
            | "bool" | "char" => (true, true),
            "f32" | "f64" => (true, false),
            "String" | "str" | "&str" | "&'static str" | "Unknown" => (true, true),
            // Struct/enum crate paths, trait objects, type params, unknown idents:
            // conservatively non-derivable (no cross-struct reasoning).
            _ => (false, false),
        }
    }

    /// Wrap the element of a rendered array type in `Option`: `Vec<T>` ->
    /// `Vec<Option<T>>` (for an element-nullable array declaration).
    fn wrap_elem_option(ty: &str) -> String {
        match ty.strip_prefix("Vec<").and_then(|s| s.strip_suffix('>')) {
            Some(inner) => format!("Vec<Option<{inner}>>"),
            None => ty.to_string(),
        }
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
            if let Node::MethodCallExpr { scope: Some(s), name, args, .. } = self.arena.kind(n) {
                if let Node::NameExpr { name: recv } = self.arena.kind(*s) {
                    if let Some(m) = self.resolve_linked_callee(Some(*s), name, args) {
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

    /// Is the type node `n` rendered at a value-storage SLOT — a field/parameter/
    /// method-return/local-variable declaration, or a type-argument nested inside
    /// such — as opposed to a non-slot render (a cast target, `instanceof` type,
    /// `new` type, `throws`, or an `extends`/`implements` clause)? Walks up through
    /// `ReferenceType` array wrappers and `ClassOrInterfaceType` type-arg nesting.
    ///
    /// R4 uses this to substitute a supertype's synthesized enum *only at slots*:
    /// a slot stores a value (so it must carry the variant), whereas the struct's
    /// own definition, its `base:` composition field (emitted directly, not via
    /// `visit_class_type`), `impl` headers, and dispatch-site type renders must
    /// keep the plain type.
    fn is_slot_type(&self, n: NodeId) -> bool {
        let Some(p) = self.arena.parent(n) else { return false };
        match self.arena.kind(p) {
            // Array wrapper / type-arg nesting: inherit the enclosing context.
            Node::ReferenceType { .. } => self.is_slot_type(p),
            Node::ClassOrInterfaceType { .. } => self.is_slot_type(p),
            // Declaration slots — match the *type* field specifically (so a
            // `MethodDeclaration`'s `throws` types are excluded, only its `typ`
            // return slot qualifies).
            Node::FieldDeclaration { typ, .. } => *typ == n,
            Node::VariableDeclarationExpr { typ, .. } => *typ == n,
            Node::MethodDeclaration { typ, .. } => *typ == n,
            Node::Parameter { typ, .. } => *typ == Some(n),
            // Cast / instanceof / new / extends-implements / throws / catch: the
            // type is being tested or constructed, not stored — not a slot.
            _ => false,
        }
    }

    /// R4 hook: the synthesized enum name for a supertype that has a closed
    /// project-subtype hierarchy, when the name appears at a value-storage slot.
    /// Returns `None` until enum synthesis (R4 step 2) is implemented — so this is
    /// currently a no-op and type rendering is unchanged.
    fn slot_enum_name(&self, name: &str) -> Option<String> {
        // ROOT-ONLY activation: only a hierarchy *root* slot becomes the enum
        // (its methods are covered by the enum's `Deref` to the root, so no
        // method delegation is needed). Intermediate supertypes stay concrete.
        if self.link.is_empty() {
            return None;
        }
        let t = self.resolve_type_sym(name)?;
        match self.enum_info_map().get(&t.rust_path) {
            Some((enum_path, _vname, true)) => Some(enum_path.clone()),
            _ => None,
        }
    }

    /// Build (once) `member rust_path → (enum_path, variant_name, is_root)` for
    /// every member of every synthesizable hierarchy. Drives slot routing,
    /// construction-wrap, cast-extract, and instanceof.
    fn enum_info_map(&self) -> &std::collections::HashMap<String, (String, String, bool)> {
        self.slot_enum_cache.get_or_init(|| {
            let mut m = std::collections::HashMap::new();
            if self.link.is_empty() {
                return m;
            }
            for (fqn, _) in self.link.iter() {
                // `enum_root_variants` returns Some only for a hierarchy root.
                if let Some((variants, _, _)) = self.enum_root_variants(fqn) {
                    // GATE: activate only a hierarchy actually dispatched
                    // dynamically (some member is an `instanceof`/cast target).
                    // A storage-only hierarchy (e.g. jhlabs's image-op classes)
                    // gains nothing from an enum and only regresses (construction
                    // sites the enum-wrap doesn't yet cover).
                    if !variants.iter().any(|(vname, _, _)| self.link.is_dispatched(vname)) {
                        continue;
                    }
                    let root = match self.link.lookup(fqn) {
                        Some(r) => r,
                        None => continue,
                    };
                    let enum_path = self.crate_relativize(&format!("{}Kind", root.rust_path));
                    for (vname, vpath, hops) in variants {
                        m.entry(vpath).or_insert((enum_path.clone(), vname, hops == 0));
                    }
                }
            }
            m
        })
    }

    /// A *read* of a place (name/field/array-elem/method-result, through
    /// parentheses) — i.e. a value that, if its slot is enum-routed, already
    /// renders as the `<Root>Kind` enum (so re-wrapping would double-wrap). A
    /// non-read root expr (e.g. a borrow/seam) is excluded so it still wraps.
    fn is_enum_read_expr(&self, expr: NodeId) -> bool {
        match self.arena.kind(expr) {
            Node::NameExpr { .. }
            | Node::FieldAccessExpr { .. }
            | Node::ArrayAccessExpr { .. }
            | Node::MethodCallExpr { .. } => true,
            Node::EnclosedExpr { inner: Some(i) } => self.is_enum_read_expr(*i),
            _ => false,
        }
    }

    /// If `expr`'s concrete type is a member of a synthesized hierarchy, return
    /// (enum_path, variant_name) — for wrapping a concrete value into an enum slot.
    fn enum_variant_for_expr(&self, expr: NodeId) -> Option<(String, String)> {
        let simple = self.expr_concrete_struct(expr)?;
        let t = self.resolve_type_sym(&simple)?;
        let (ep, vn, is_root) = self.enum_info_map().get(&t.rust_path)?;
        // A value resolving to the hierarchy ROOT that is a READ of an
        // already-routed slot/collection element (a name, field, array elem, or
        // a routed-return call — a for-each var, `.get()`, etc.) already renders
        // as the `<Root>Kind` enum, so wrapping it would double-wrap
        // (`Kind::Root(line)` where `line` is itself a `Kind`). Skip the wrap for
        // those reads only — a non-read root expr (a borrow/seam) is left to wrap
        // normally. This only removes spurious wraps.
        if *is_root && self.is_enum_read_expr(expr) && !self.is_object_creation(expr) {
            return None;
        }
        Some((ep.clone(), vn.clone()))
    }

    /// If the Java type `name` is a member of a synthesized hierarchy, its
    /// (enum_path, variant_name) — for cast-extract / instanceof on the enum.
    fn enum_variant_for_type(&self, name: &str) -> Option<(String, String)> {
        let t = self.resolve_type_sym(name)?;
        let (ep, vn, _) = self.enum_info_map().get(&t.rust_path)?;
        Some((ep.clone(), vn.clone()))
    }

    /// Does `expr` evaluate to a value of an enum'd hierarchy *root* (so a cast /
    /// instanceof on it operates on the `<Root>Kind` enum)? Returns the enum path.
    fn expr_enum_root(&self, expr: NodeId) -> Option<String> {
        let crate::types::Type::Named { path, .. } = self.ty(expr) else {
            return None;
        };
        let simple = path.rsplit(['.', ':']).next().unwrap_or(&path);
        self.slot_enum_name(simple)
    }

    /// The `<Root>Kind` enum path if `ty` is `Named` to an enum'd hierarchy root.
    fn enum_path_of_type(&self, ty: &crate::types::Type) -> Option<String> {
        let crate::types::Type::Named { path, .. } = ty else { return None };
        let simple = path.rsplit(['.', ':']).next().unwrap_or(path);
        self.slot_enum_name(simple)
    }

    /// The `<Root>Kind` enum path of the enclosing method's return type, if that
    /// return type is an enum'd hierarchy slot — so a concrete `return` value is
    /// construction-wrapped into the variant.
    fn enclosing_ret_enum(&self, mut n: NodeId) -> Option<String> {
        let typ = loop {
            match self.arena.parent(n) {
                Some(p) => {
                    if let Node::MethodDeclaration { typ, .. } = self.arena.kind(p) {
                        break *typ;
                    }
                    n = p;
                }
                None => return None,
            }
        };
        let simple = self.type_simple_name(typ)?;
        self.slot_enum_name(&simple)
    }

    /// The `<Root>Kind` enum path a linked-call param routes to, if its (plain,
    /// non-array, non-generic) Java type is an enum'd hierarchy supertype — so a
    /// concrete argument is construction-wrapped into the variant.
    fn param_enum_path(&self, p: &crate::symbol_map::ParamSym) -> Option<String> {
        let jt = p.java_type.trim();
        if jt.is_empty() || jt.contains('[') || jt.contains('<') {
            return None;
        }
        let simple = jt.rsplit('.').next().unwrap_or(jt);
        self.slot_enum_name(simple)
    }

    /// Emit `value` wrapped into `enum_path`'s variant if its concrete type is a
    /// member of that enum (construction-wrap); otherwise emit it plainly.
    fn emit_enum_wrapped(&mut self, value: NodeId, enum_path: &str, arg: Arg) {
        let wrap = self
            .enum_variant_for_expr(value)
            .filter(|(ep, _)| ep == enum_path)
            .map(|(_, vn)| vn);
        match wrap {
            Some(vname) => {
                self.printer.print(&format!("{enum_path}::{vname}("));
                self.visit(value, arg);
                self.printer.print(")");
            }
            None => self.visit(value, arg),
        }
    }

    /// If `root_fqn` is the top of a project class hierarchy (a non-generic struct
    /// whose parent is not a project type) with ≥1 project subtype, the
    /// synthesized-enum data: variants as `(rust variant name, crate-relative
    /// payload path, `.base` hops up to the root)`, plus whether *all* variants
    /// support `PartialEq` / `Eq+Hash` (gates the enum's derives). `None` if not a
    /// root, or if any member is generic/non-struct (out of scope here).
    fn enum_root_variants(&self, root_fqn: &str) -> Option<(Vec<(String, String, usize)>, bool, bool)> {
        let root = self.link.lookup(root_fqn)?;
        if root.kind != "struct" || root.generic {
            return None;
        }
        // The root must be the top of its *project* hierarchy.
        if root.parent.as_deref().map(|p| self.link.lookup(p).is_some()).unwrap_or(false) {
            return None;
        }
        let mut variants: Vec<(String, String, usize)> = Vec::new();
        let (mut all_pe, mut all_eh) = (true, true);
        for (fqn, t) in self.link.iter() {
            // hops from `t` up to the root via the `parent` chain.
            let mut hops = 0usize;
            let mut cur = fqn.clone();
            let mut found = false;
            let mut seen = std::collections::HashSet::new();
            loop {
                if cur == root_fqn {
                    found = true;
                    break;
                }
                if !seen.insert(cur.clone()) {
                    break;
                }
                match self.link.lookup(&cur).and_then(|c| c.parent.clone()) {
                    Some(p) => {
                        hops += 1;
                        cur = p;
                    }
                    None => break,
                }
            }
            if !found {
                continue;
            }
            if t.generic || t.kind != "struct" {
                return None; // a generic/non-struct member → bail the whole hierarchy
            }
            all_pe &= t.partial_eq_capable;
            all_eh &= t.eq_hash_capable;
            let vname = t.rust_path.rsplit("::").next().unwrap_or(fqn).to_string();
            variants.push((vname, self.crate_relativize(&t.rust_path), hops));
        }
        if variants.len() < 2 {
            return None; // no subtypes → not a polymorphic hierarchy
        }
        // Variant identifiers must be unique. The simple rust name collides when a
        // hierarchy has same-named members (e.g. many nested `Context` classes), so
        // disambiguate any colliding name with its module path (sanitized full
        // rust path, `crate::a::b::C` → `a_b_C`).
        let mut counts: std::collections::HashMap<&str, usize> = std::collections::HashMap::new();
        for (v, _, _) in &variants {
            *counts.entry(v.as_str()).or_insert(0) += 1;
        }
        let dup: std::collections::HashSet<String> =
            counts.iter().filter(|(_, c)| **c > 1).map(|(k, _)| k.to_string()).collect();
        for (vname, path, _) in &mut variants {
            if dup.contains(vname) {
                *vname = path
                    .trim_start_matches("crate::")
                    .replace("::", "_");
            }
        }
        variants.sort();
        Some((variants, all_pe, all_eh))
    }

    /// Emit the synthesized `<Root>Kind` enum + `Deref`/`DerefMut` to the root,
    /// when the type just emitted (`name` / `root_fqn`) is a hierarchy root. One
    /// variant per concrete type; `Deref` forwards base-method calls to the root
    /// via each variant's `.base` chain (so no per-method delegation is needed).
    /// Dormant until `slot_enum_name` routes slots to it; `#[allow(dead_code)]`
    /// keeps it warning-free meanwhile.
    fn emit_hierarchy_enum(&mut self, name: &str, root_fqn: &str) {
        let Some((variants, all_pe, all_eh)) = self.enum_root_variants(root_fqn) else {
            return;
        };
        let ename = format!("{name}Kind");
        // `Default` can't be *derived* for an enum whose default variant carries a
        // payload (derive needs a unit `#[default]`), so derive the rest and write
        // `Default` by hand, returning the root variant.
        let mut derives = String::from("Clone");
        if all_pe || all_eh {
            derives.push_str(", PartialEq");
        }
        if all_eh {
            derives.push_str(", Eq, Hash");
        }
        self.printer.print_ln();
        self.printer.print_ln_s("#[allow(dead_code)]");
        self.printer.print_ln_s(&format!("#[derive({derives})]"));
        self.printer.print_ln_s(&format!("pub enum {ename} {{"));
        self.printer.indent();
        for (vname, vpath, _hops) in &variants {
            self.printer.print_ln_s(&format!("{vname}({vpath}),"));
        }
        self.printer.unindent();
        self.printer.print_ln_s("}");
        // Manual `Default` -> the root variant (the only one always present).
        if let Some((rname, rpath, _)) = variants.iter().find(|(_, _, h)| *h == 0) {
            self.printer.print_ln_s(&format!(
                "impl Default for {ename} {{ fn default() -> Self {{ {ename}::{rname}({rpath}::default()) }} }}"
            ));
        }
        // Deref then DerefMut, via each variant's `.base` chain to the root.
        self.printer
            .print_ln_s(&format!("impl std::ops::Deref for {ename} {{ type Target = {name};"));
        self.printer.indent();
        self.printer.print_ln_s(&format!("fn deref(&self) -> &{name} {{ match self {{"));
        self.printer.indent();
        for (vname, _v, hops) in &variants {
            let access =
                if *hops == 0 { "x".to_string() } else { format!("&x{}", ".base".repeat(*hops)) };
            self.printer.print_ln_s(&format!("{ename}::{vname}(x) => {access},"));
        }
        self.printer.unindent();
        self.printer.print_ln_s("} }");
        self.printer.unindent();
        self.printer.print_ln_s("}");
        self.printer.print_ln_s(&format!("impl std::ops::DerefMut for {ename} {{"));
        self.printer.indent();
        self.printer.print_ln_s(&format!("fn deref_mut(&mut self) -> &mut {name} {{ match self {{"));
        self.printer.indent();
        for (vname, _v, hops) in &variants {
            let access = if *hops == 0 {
                "x".to_string()
            } else {
                format!("&mut x{}", ".base".repeat(*hops))
            };
            self.printer.print_ln_s(&format!("{ename}::{vname}(x) => {access},"));
        }
        self.printer.unindent();
        self.printer.print_ln_s("} }");
        self.printer.unindent();
        self.printer.print_ln_s("}");
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
                match self.arena.kind(gp) {
                    // A local (`T x = call()`) or a field (`static final T f = call()`)
                    // initializer — infer the call's return from the declared type.
                    Node::VariableDeclarationExpr { typ, .. }
                    | Node::FieldDeclaration { typ, .. } => self.rust_type_of(*typ),
                    _ => None,
                }
            }
            Node::ReturnStmt { expr: Some(e) } if *e == call => self.enclosing_method_ret_type(call),
            // A call used as a condition or logical operand returns `bool`.
            Node::WhileStmt { condition, .. }
            | Node::DoStmt { condition, .. }
            | Node::IfStmt { condition, .. }
            | Node::ConditionalExpr { condition, .. }
                if *condition == call =>
            {
                Some("bool".to_string())
            }
            Node::ForStmt { compare: Some(c), .. } if *c == call => Some("bool".to_string()),
            Node::UnaryExpr { op: UnaryOp::Not, expr } if *expr == call => Some("bool".to_string()),
            Node::BinaryExpr { op: BinaryOp::And | BinaryOp::Or, .. } => Some("bool".to_string()),
            // `<call> <arith/cmp> other` -> the call shares the other operand's
            // numeric type (gated to a non-literal operand, so a bare `2` doesn't
            // mis-pin a `double`-returning stub to `i32`).
            Node::BinaryExpr { left, op, right }
                if matches!(
                    op,
                    BinaryOp::Plus
                        | BinaryOp::Minus
                        | BinaryOp::Times
                        | BinaryOp::Divide
                        | BinaryOp::Remainder
                        | BinaryOp::Less
                        | BinaryOp::Greater
                        | BinaryOp::LessEquals
                        | BinaryOp::GreaterEquals
                ) =>
            {
                let other = if *left == call {
                    *right
                } else if *right == call {
                    *left
                } else {
                    return None;
                };
                if self.is_numeric_literal(other) {
                    return None;
                }
                self.ty(other).numeric_rust().map(str::to_string)
            }
            // `target = <call>` -> the call returns the target's declared type.
            Node::AssignExpr { target, value, .. } if *value == call => {
                self.assign_target_rust_type(*target)
            }
            // `f(<call>)` -> the call returns the type of the parameter it flows
            // into (when `f` resolves to a known method with a typed param). Only
            // when `call` is an *argument* (not the receiver — `position` is
            // `None` then).
            Node::MethodCallExpr { scope, name, args, .. } => {
                // `<call>.<m>(..)`: a distinctive String/StringBuilder method on
                // the result pins it to `String` (e.g. Java `append` -> our
                // `push_str`, which a bare `Unknown` receiver can't satisfy).
                //
                // B5 (front this name-guess with `self.ty(call)`) is DEFERRED behind
                // B4: the useful version returns the resolver's *precise* type when
                // it knows the receiver concretely, which needs the `Ty -> Rust-
                // string` renderer (B4). The renderer-free approximation (return
                // `None` instead of mis-pinning to `String` for a concrete non-`Str`
                // receiver) measured net-zero on all 12 corpora — these positions
                // rarely resolve to a concrete non-String type — so it wasn't worth
                // the extra `self.ty` call. Revisit when B4 lands.
                if *scope == Some(call) {
                    return matches!(
                        name.as_str(),
                        "append" | "charAt" | "substring" | "toLowerCase" | "toUpperCase"
                    )
                    .then(|| "String".to_string());
                }
                let idx = args.iter().position(|&a| a == call)?;
                let m = match scope {
                    Some(_) => self.resolve_linked_callee(*scope, name, args),
                    None => self.resolve_self_callee(name, args.len()),
                }?;
                Self::java_simple_to_rust_static(&m.params.get(idx)?.java_type).map(str::to_string)
            }
            // `new Foo(<call>)` -> the type of the constructor parameter `call`
            // flows into.
            Node::ObjectCreationExpr { typ, args, .. } => {
                let idx = args.iter().position(|&a| a == call)?;
                let t = self.resolve_type_sym(&self.type_simple_name(*typ)?)?;
                let m = t
                    .methods
                    .get(&format!("new#{}", args.len()))
                    .or_else(|| t.methods.get("new"))?;
                Self::java_simple_to_rust_static(&m.params.get(idx)?.java_type).map(str::to_string)
            }
            _ => None,
        }
    }

    /// Convert a *simple* Java type name to the Rust type the translator emits,
    /// for the clean cases only (primitive/boxed/String/`char[]`). Returns `None`
    /// for generics/collections/user types — there a stub return is left
    /// `Unknown` rather than guessed.
    fn java_simple_to_rust_static(s: &str) -> Option<&'static str> {
        Some(match s {
            "double" | "Double" => "f64",
            "float" | "Float" => "f32",
            "int" | "Integer" => "i32",
            "long" | "Long" => "i64",
            "short" | "Short" => "i16",
            "byte" | "Byte" => "i8",
            "boolean" | "Boolean" => "bool",
            "char" | "Character" => "char",
            "String" => "String",
            "char[]" => "Vec<char>",
            _ => return None,
        })
    }

    /// The Rust type of an assignment target (a name or `self.field`), for stub
    /// return-type inference of `target = <stub call>`.
    /// If the assignment target's declared type is a project *interface* — which
    /// renders as an owned `Box<dyn Trait>` field — the trait's simple name. Lets
    /// `self.field = <concrete>` box the RHS. (`assign_target_rust_type` returns
    /// the bare type name via `rust_type_of`, never the `Box<dyn …>` form, so the
    /// interface must be detected from the symbol map here.)
    fn assign_target_trait(&self, target: NodeId) -> Option<String> {
        let name = match self.arena.kind(target) {
            Node::NameExpr { name } => name.clone(),
            Node::FieldAccessExpr { field, .. } => field.clone(),
            _ => return None,
        };
        let (_, decl) = self.id.find_declaration_node_for(self.arena, &name, target)?;
        let parent = self.arena.parent(decl)?;
        let typ = match self.arena.kind(parent) {
            Node::Parameter { typ, .. } => (*typ)?,
            _ => match self.arena.parent(parent).map(|g| self.arena.kind(g)) {
                Some(Node::FieldDeclaration { typ, .. })
                | Some(Node::VariableDeclarationExpr { typ, .. }) => *typ,
                _ => return None,
            },
        };
        let simple = self.type_simple_name(typ)?;
        let t = self.resolve_type_sym(&simple)?;
        (t.kind == "trait").then_some(simple)
    }

    /// The `<Root>Kind` enum path of an assignment TARGET that is an enum-routed
    /// slot — so a concrete RHS is construction-wrapped into the variant (the
    /// target-side parallel of `param_enum_path`). Covers `arr[i]` (the array's
    /// enum'd element) and a plain (non-array) hierarchy-typed `name`/`field`.
    fn assign_target_enum_path(&self, target: NodeId) -> Option<String> {
        // `arr[i] = …`: route via the array's element type (`type_of(arr).elem()`),
        // mapping the concrete element type to its enum (declared `Geometry` →
        // `GeometryKind`), not relying on the resolver to have routed it.
        if let Node::ArrayAccessExpr { name, .. } = self.arena.kind(target) {
            if let Some(crate::types::Type::Named { path, .. }) = self.ty(*name).elem() {
                let simple = path.rsplit(['.', ':']).next().unwrap_or(path);
                return self.slot_enum_name(simple);
            }
            return None;
        }
        // `name`/`field = …`: the declared type, but only a PLAIN (non-array)
        // hierarchy type — an array-typed var assigns the whole `Vec<…Kind>`, not
        // a single variant, so it must not be single-wrapped.
        let name = match self.arena.kind(target) {
            Node::NameExpr { name } => name.clone(),
            Node::FieldAccessExpr { field, .. } => field.clone(),
            _ => return None,
        };
        let (_, decl) = self.id.find_declaration_node_for(self.arena, &name, target)?;
        let parent = self.arena.parent(decl)?;
        let typ = match self.arena.kind(parent) {
            Node::Parameter { typ, .. } => (*typ)?,
            _ => match self.arena.parent(parent).map(|g| self.arena.kind(g)) {
                Some(Node::FieldDeclaration { typ, .. })
                | Some(Node::VariableDeclarationExpr { typ, .. }) => *typ,
                _ => return None,
            },
        };
        if matches!(self.arena.kind(typ), Node::ReferenceType { array_count, .. } if *array_count > 0)
        {
            return None;
        }
        let simple = self.type_simple_name(typ)?;
        self.slot_enum_name(&simple)
    }

    fn assign_target_rust_type(&self, target: NodeId) -> Option<String> {
        let name = match self.arena.kind(target) {
            Node::NameExpr { name } => name.clone(),
            Node::FieldAccessExpr { field, .. } => field.clone(),
            _ => return None,
        };
        let (_, decl) = self.id.find_declaration_node_for(self.arena, &name, target)?;
        let parent = self.arena.parent(decl)?;
        let typ = match self.arena.kind(parent) {
            Node::Parameter { typ, .. } => (*typ)?,
            _ => match self.arena.parent(parent).map(|g| self.arena.kind(g)) {
                Some(Node::FieldDeclaration { typ, .. })
                | Some(Node::VariableDeclarationExpr { typ, .. }) => *typ,
                _ => return None,
            },
        };
        self.rust_type_of(typ)
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

    /// Does the enclosing method's return type render as an owned trait object
    /// (`Box<dyn T>`)? Returns the trait's simple name, so a concrete return
    /// value (`new Concrete()` or a value of an implementing struct type) can be
    /// `Box::new(..)`-coerced into it.
    fn enclosing_ret_box_dyn(&mut self, mut n: NodeId) -> Option<String> {
        let typ = loop {
            match self.arena.parent(n) {
                Some(p) => {
                    if let Node::MethodDeclaration { typ, .. } = self.arena.kind(p) {
                        break *typ;
                    }
                    n = p;
                }
                None => return None,
            }
        };
        Self::box_dyn_trait_simple(self.accept_and_cut(typ, None).trim())
    }

    /// If `rust_ty` is an owned trait object `Box<dyn Trait…>`, the trait's simple
    /// name (`Box<dyn module::Trimmer + 'static>` → `Trimmer`).
    fn box_dyn_trait_simple(rust_ty: &str) -> Option<String> {
        let inner = rust_ty.trim().strip_prefix("Box<dyn ")?;
        let inner = inner.strip_suffix('>').unwrap_or(inner);
        let head = inner.split(['+', '<']).next().unwrap_or(inner).trim();
        Some(head.rsplit("::").next().unwrap_or(head).trim().to_string())
    }

    /// The simple name of `expr`'s concrete project *struct* type, if it has one
    /// and is provably **not** already a trait object — either the resolver types
    /// it as a `Named` struct, or it's a (possibly static) method call whose
    /// recorded return type is a concrete struct (a return type that is itself a
    /// `Box<dyn …>` yields `None`, so a factory already returning a boxed value is
    /// never re-boxed). `None` for `new Concrete()` (handled separately) and for
    /// anything of unknown type.
    fn expr_concrete_struct(&self, expr: NodeId) -> Option<String> {
        let simple_of = |s: &str| {
            let s = s.split(['<', '[']).next().unwrap_or(s);
            s.rsplit(['.', ':']).next().unwrap_or(s).trim().to_string()
        };
        if let crate::types::Type::Named { path, .. } = self.ty(expr) {
            return Some(simple_of(&path));
        }
        if let Node::MethodCallExpr { scope, name, args, .. } = self.arena.kind(expr) {
            let m = self.resolve_linked_callee(*scope, name, args)?;
            let ret = m.ret.as_ref()?;
            if ret.contains("dyn ") {
                return None; // already a trait object — don't re-box
            }
            return Some(simple_of(ret));
        }
        None
    }

    /// Does `struct_simple` implement `trait_simple` (directly, or via a
    /// superclass/interface chain in the symbol map)?
    fn struct_impls_trait(&self, struct_simple: &str, trait_simple: &str) -> bool {
        fn simple_of(s: &str) -> &str {
            s.rsplit(['.', ':']).next().unwrap_or(s)
        }
        let mut cur = self.resolve_type_sym(struct_simple);
        let mut seen: Vec<&str> = Vec::new();
        while let Some(ty) = cur {
            if ty.kind != "struct" {
                return false;
            }
            if ty.interfaces.iter().any(|i| simple_of(i) == trait_simple) {
                return true;
            }
            match ty.parent.as_deref() {
                Some(p) => {
                    if simple_of(p) == trait_simple {
                        return true;
                    }
                    if seen.contains(&p) {
                        break;
                    }
                    seen.push(p);
                    cur = self.link.lookup(p);
                }
                None => break,
            }
        }
        false
    }

    /// Does `expr` resolve to a concrete struct implementing `trait_simple`, so
    /// flowing it into a `Box<dyn trait_simple>` slot needs `Box::new(..)`? Never
    /// true for a value already a trait object (so it never double-boxes).
    fn expr_impls_trait(&self, expr: NodeId, trait_simple: &str) -> bool {
        self.expr_concrete_struct(expr)
            .is_some_and(|s| self.struct_impls_trait(&s, trait_simple))
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
            // Preserve array dimensions: `byte[]` -> `Vec<i8>`, not `i8`.
            Node::ReferenceType { typ, array_count } => {
                let inner = self.rust_type_of(*typ)?;
                Some((0..*array_count).fold(inner, |t, _| format!("Vec<{t}>")))
            }
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
                // Preserve prior behavior for a chained method-call receiver:
                // before return-type tracking, `callee_recv_type` returned `None`
                // here (→ no stub). Recording a stub against the newly-resolved
                // receiver type changes stub shapes and regresses (jts +9, vs
                // fastq/jsoup −3 — measured NO-GO), so keep stub recording off it.
                if matches!(self.arena.kind(s), Node::MethodCallExpr { .. }) {
                    return;
                }
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
            // A chained method-call receiver (`a.foo().bar()`): resolve `foo()`'s
            // return type via the rich type resolver to its Java simple name, so
            // `bar()` can resolve in the linked maps. (Stub recording deliberately
            // does NOT use this arm — see `record_missing_call`.)
            Node::MethodCallExpr { .. } => match self.ty(scope) {
                crate::types::Type::Named { path, .. } if !path.is_empty() => Some(path),
                crate::types::Type::Str => Some("String".to_string()),
                _ => None,
            },
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
        args: &[NodeId],
    ) -> Option<&'a crate::symbol_map::MethodSym> {
        if self.link.is_empty() {
            return None;
        }
        let simple = self.callee_recv_type(scope?)?;
        // Walk the receiver type's superclass chain (mirrors `resolve_self_callee`):
        // an *inherited* method (e.g. `get_id` declared on a base `VCFHeaderLine`,
        // called on a `VCFInfoHeaderLine`) isn't in the subtype's own `methods`, so
        // a direct lookup misses it and the call site loses the recorded signature
        // (argument borrowing AND return nullability — a nullable inherited getter
        // used as a plain value would not be `.unwrap()`'d).
        // Overloaded methods are keyed `name#arity`; the base overload keeps the
        // bare name. The key is loop-invariant — build it once (not per ancestor).
        let arity_key = format!("{name}#{args_len}", args_len = args.len());
        let mut t = self.resolve_type_sym(&simple);
        // Guard against a cyclic `parent` chain in the project map (a recorded
        // superclass that loops back) — otherwise this walk never terminates.
        let mut seen: Vec<&str> = Vec::new();
        while let Some(ty) = t {
            // 1. exact `name#arity`. 2. argument-TYPE-directed pick among
            //    same-name/same-arity overloads (when several share an arity, no
            //    `name#arity` key exists and the bare-name fallback would wrongly
            //    pick the base overload — possibly of a different arity entirely).
            //    3. the bare-name fallback (preserved for everything else).
            if let Some(m) = ty.methods.get(&arity_key) {
                return Some(m);
            }
            if let Some(m) = self.pick_overload(ty, name, args) {
                return Some(m);
            }
            if let Some(m) = ty.methods.get(name) {
                return Some(m);
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

    /// Pick among `ty`'s same-name, same-arity overloads by matching argument
    /// types — used when several overloads share an arity (so no `name#arity`
    /// key exists). Conservative/monotone: disqualifies a candidate on a
    /// collection-vs-single shape mismatch, then returns the *unique* highest
    /// exact-type-match; on a tie or no candidates returns `None` (the caller's
    /// existing bare-name fallback then applies, unchanged).
    fn pick_overload(
        &self,
        ty: &'a crate::symbol_map::TypeSym,
        name: &str,
        args: &[NodeId],
    ) -> Option<&'a crate::symbol_map::MethodSym> {
        let snake = self.to_snake_if_necessary(name);
        let family = |key: &str| -> bool {
            key == name
                || key == snake
                || key
                    .strip_prefix(&format!("{snake}_"))
                    .map(|r| !r.is_empty() && r.bytes().all(|b| b.is_ascii_digit() || b == b'_'))
                    .unwrap_or(false)
        };
        let mut best: Option<(i32, &crate::symbol_map::MethodSym)> = None;
        let mut tie = false;
        for (key, m) in &ty.methods {
            if m.params.len() != args.len() || !family(key) {
                continue;
            }
            let mut score = 0i32;
            let mut ok = true;
            for (p, &a) in m.params.iter().zip(args) {
                match self.param_arg_score(a, &p.java_type) {
                    Some(s) => score += s,
                    None => {
                        ok = false;
                        break;
                    }
                }
            }
            if !ok {
                continue;
            }
            match best {
                Some((bs, _)) if score == bs => tie = true,
                Some((bs, _)) if score < bs => {}
                _ => {
                    best = Some((score, m));
                    tie = false;
                }
            }
        }
        if tie {
            return None;
        }
        best.map(|(_, m)| m)
    }

    /// Score how well argument `arg` matches a parameter of Java type
    /// `java_type`: `None` = incompatible (collection vs single shape mismatch);
    /// `Some(3)` exact, `Some(1)` weak (`Object`/same-shape), `Some(0)` neutral
    /// (unknown arg type — no information).
    fn param_arg_score(&self, arg: NodeId, java_type: &str) -> Option<i32> {
        use crate::types::Type;
        if java_type.is_empty() {
            return Some(0);
        }
        let pt = crate::types::parse_java_type(java_type);
        let simple = |s: &str| s.rsplit(['.', ':']).next().unwrap_or(s).to_string();
        // `Object` (and unparsed) params accept anything (weak).
        if matches!(&pt, Type::Named { path, .. } if simple(path) == "Object")
            || matches!(pt, Type::Unknown)
        {
            return Some(1);
        }
        let at = self.ty(arg);
        if matches!(at, Type::Unknown) {
            return Some(0);
        }
        let coll = |t: &Type| matches!(t, Type::Vec(_) | Type::Set(_) | Type::Map(_, _));
        if coll(&pt) != coll(&at) {
            return None; // collection-vs-single shape mismatch
        }
        Some(match (&pt, &at) {
            (Type::Prim(a), Type::Prim(b)) => {
                if a == b {
                    3
                } else {
                    1
                }
            }
            (Type::Str, Type::Str) => 3,
            (Type::Named { path: pp, .. }, Type::Named { path: ap, .. }) => {
                let (ps, as_) = (simple(pp), simple(ap));
                if ps == as_ || as_.strip_suffix("Kind") == Some(&ps) {
                    3
                } else {
                    1
                }
            }
            _ => 1,
        })
    }

    /// Resolve a bare self-call (`name(args)`) to its `MethodSym` in the current
    /// class or an ancestor, so its parameter signature drives argument borrowing
    /// (bare calls otherwise miss the linked-callee path and under-/over-borrow).
    fn resolve_self_callee(&self, name: &str, arity: usize) -> Option<&'a crate::symbol_map::MethodSym> {
        if self.link.is_empty() {
            return None;
        }
        let arity_key = format!("{name}#{arity}");
        let mut t = self.link.lookup(self.current_class_fqn.as_deref()?);
        // Cycle guard (see `resolve_linked_callee`): a looping `parent` chain in
        // the project map would otherwise spin here forever.
        let mut seen: Vec<&str> = Vec::new();
        while let Some(ty) = t {
            if let Some(m) = ty.methods.get(&arity_key).or_else(|| ty.methods.get(name)) {
                return Some(m);
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

    /// Resolve the constructor of `type_simple` matching `arity`. Project/linked
    /// types record constructors under `new` (the base overload) and
    /// `new#arity` (the rest); returns `None` for unknown/unrecorded types so
    /// the caller emits a plain `::new`.
    fn resolve_ctor(&self, type_simple: &str, arity: usize) -> Option<&'a crate::symbol_map::MethodSym> {
        if self.link.is_empty() {
            return None;
        }
        let t = self.resolve_type_sym(type_simple)?;
        t.methods
            .get(&format!("new#{arity}"))
            .or_else(|| t.methods.get("new"))
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
                    } else if let Some(ep) = self.param_enum_path(p) {
                        // A concrete arg into a `&<Root>Kind` param → wrap.
                        self.emit_enum_wrapped(e, &ep, arg);
                    } else {
                        self.visit(e, arg);
                    }
                }
                Some(p) if p.nullable => {
                    let pep = self.param_enum_path(p);
                    self.emit_into_option_enum(e, pep.as_deref(), arg);
                }
                // A by-value (scalar) param: widen a numeric arg to the param's
                // type — Java auto-widens `int`->`float`/`long` at the call
                // boundary, Rust does not. Non-numeric params fall through to a
                // plain moved value.
                Some(p) => {
                    if let Some(ep) = self.param_enum_path(p) {
                        self.emit_enum_wrapped(e, &ep, arg);
                    } else {
                        self.emit_numeric_arg(e, &p.rust_type, arg);
                    }
                }
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
        let mut borrowed = false;
        if let Node::NameExpr { name } = self.arena.kind(e) {
            if let Some((Some(left), _)) = self.id.find_declaration_node_for(self.arena, name, e) {
                if !left.is_primitive || left.array_count > 0 {
                    self.printer.print("&");
                    borrowed = true;
                }
            }
        }
        self.visit(e, arg);
        // An array element read passed by value moves out of the `Vec`; clone it
        // (unless it was borrowed above).
        if !borrowed && self.is_array_read(e) {
            self.printer.print(".clone()/* TODO(translation): validate added clone */");
        }
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

    /// Is `field` a nullable instance field of the class enclosing `at`? Resolves
    /// against the class's fields directly, so a same-named param/local (which
    /// would shadow under general name resolution) can't mislead.
    fn this_field_nullable(&self, field: &str, at: NodeId) -> bool {
        let mut n = at;
        while let Some(p) = self.arena.parent(n) {
            if let Node::ClassOrInterfaceDeclaration { members, .. } = self.arena.kind(p) {
                for &m in members {
                    if let Node::FieldDeclaration { variables, .. } = self.arena.kind(m) {
                        for &v in variables {
                            if let Node::VariableDeclarator { id: vid, .. } = self.arena.kind(v) {
                                if let Node::VariableDeclaratorId { name } = self.arena.kind(*vid) {
                                    if name == field {
                                        return self.nullable.contains(vid);
                                    }
                                }
                            }
                        }
                    }
                }
                return false;
            }
            n = p;
        }
        false
    }

    /// Mirror of `nullability::expr_nullable` for the dumper.
    fn expr_nullable(&self, e: NodeId) -> bool {
        match self.arena.kind(e) {
            Node::NullLiteralExpr => true,
            // A local/param/own-field by name, else (unresolved) an inherited
            // superclass field — consistent with how `visit_name_expr` emits it.
            Node::NameExpr { name } => {
                if self.id.find_declaration_node_for(self.arena, name, e).is_some() {
                    self.name_decl_nullable(name, e)
                } else {
                    self.inherited_field_nullable(name)
                }
            }
            // `readLine()` yields `Option<String>` (its stub/shim is nullable);
            // mark it so an `Option`-target assignment isn't double-wrapped.
            Node::MethodCallExpr { name, args, .. } if name == "readLine" && args.is_empty() => true,
            Node::MethodCallExpr { scope: None, name, .. } => self.name_decl_nullable(name, e),
            // `this.field` where the field is nullable. Resolved against the
            // enclosing class's *fields* specifically (not general name
            // resolution, which a same-named param/local would shadow — e.g.
            // `this.version = version`).
            Node::FieldAccessExpr { scope, field, .. }
                if matches!(self.arena.kind(*scope), Node::ThisExpr { .. }) =>
            {
                self.this_field_nullable(field, e)
            }
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
            if let Some((td, _)) = self.id.find_declaration_node_for(self.arena, name, e) {
                // An array maps to a `Vec` (non-Copy) even when its *element* is
                // primitive (`char[]`'s descriptor reports `char`, which is
                // Copy) — reading it out of a borrow still needs `.clone()`.
                if self.decl_is_array(name, e) {
                    return true;
                }
                if let Some(td) = td {
                    return !td.is_primitive;
                }
                // No type descriptor (e.g. a parameter): fall back to the
                // declared Java type. A non-scalar (class/array, emitted as a
                // borrow) is non-Copy, so moving/returning it needs `.clone()`.
                if let Some(jt) = self.decl_java_type_name(name, e) {
                    return !is_scalar_java_type(&jt);
                }
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
        // A non-Copy name, field read, or array element read can't be moved out
        // of its borrow — clone it to produce an owned value. EXCEPT an owned
        // local at its last (only, non-loop) read, which can be *moved* instead
        // (§6 use-site borrow: an eager clone is over-owning).
        let needs_owned =
            self.is_non_copy_name(e) || self.is_field_read(e) || self.is_array_read(e);
        if needs_owned && !self.is_movable_last_use(e) {
            self.printer.print(".clone()/* TODO(translation): validate added clone */");
        }
    }

    /// Can the value `e` be MOVED (not cloned) here? Sound conditions: (a) `e` is a
    /// `NameExpr` resolving to a local declared in the current method (not a
    /// param/field/array-element — those are behind a borrow and can't be moved
    /// out); (b) `name` is *read* exactly once textually in the method (so this use
    /// is that read — over-counting under shadowing only makes it more
    /// conservative); (c) `e` is not inside a loop body (which could re-execute the
    /// read → use-after-move). When any fails, the caller keeps the clone.
    fn is_movable_last_use(&self, e: NodeId) -> bool {
        let Node::NameExpr { name } = self.arena.kind(e) else {
            return false;
        };
        let name = name.clone();
        // (a) a local: decl's grandparent is a local `VariableDeclarationExpr`
        // (a param's parent is `Parameter`; a field's grandparent is
        // `FieldDeclaration`).
        let Some((_, decl)) = self.id.find_declaration_node_for(self.arena, &name, e) else {
            return false;
        };
        let is_local = self
            .arena
            .parent(decl)
            .and_then(|p| self.arena.parent(p))
            .map(|gp| matches!(self.arena.kind(gp), Node::VariableDeclarationExpr { .. }))
            .unwrap_or(false);
        if !is_local {
            return false;
        }
        let Some(method) = self.enclosing_callable(e) else {
            return false;
        };
        // (c) not inside a loop within the method.
        if self.within_loop_of(e, method) {
            return false;
        }
        // (b) exactly one textual read of `name` in the method.
        let mut reads = 0usize;
        for i in 0..self.arena.node_count() {
            let n = NodeId(i as u32);
            if let Node::NameExpr { name: nm } = self.arena.kind(n) {
                if *nm == name && self.is_descendant_of(n, method) && !self.is_assign_target(n) {
                    reads += 1;
                    if reads > 1 {
                        return false;
                    }
                }
            }
        }
        reads == 1
    }

    /// The enclosing method/constructor declaration node, if any.
    fn enclosing_callable(&self, mut n: NodeId) -> Option<NodeId> {
        loop {
            let p = self.arena.parent(n)?;
            if matches!(
                self.arena.kind(p),
                Node::MethodDeclaration { .. } | Node::ConstructorDeclaration { .. }
            ) {
                return Some(p);
            }
            n = p;
        }
    }

    fn is_descendant_of(&self, mut n: NodeId, anc: NodeId) -> bool {
        while let Some(p) = self.arena.parent(n) {
            if p == anc {
                return true;
            }
            n = p;
        }
        false
    }

    /// Is `n` the *target* (write side) of an assignment (so not a read)?
    fn is_assign_target(&self, n: NodeId) -> bool {
        self.arena
            .parent(n)
            .map(|p| matches!(self.arena.kind(p), Node::AssignExpr { target, .. } if *target == n))
            .unwrap_or(false)
    }

    /// Is `e` the *receiver* (scope) of a method call whose method is a known
    /// read-only (`&self`) operation? Such a call only needs to *borrow* the
    /// value, so a nullable read here can be unwrapped through `.as_ref()`
    /// (yielding `&T`) instead of cloning out an owned `T` (§6 use-site borrow —
    /// the call autorefs `&T` identically). The whitelist is intentionally
    /// conservative: only universally read-only Java methods (and the Rust
    /// intrinsics the translator lowers them to) — a mutating/consuming method
    /// (`add`/`put`/`set`/`close`/…) is NOT listed, so the clone stands there.
    fn is_readonly_method_receiver(&self, e: NodeId) -> bool {
        let Some(p) = self.arena.parent(e) else { return false };
        if let Node::MethodCallExpr { scope, name, .. } = self.arena.kind(p) {
            return *scope == Some(e) && is_readonly_java_method(name);
        }
        false
    }

    /// A use-site that only needs to *borrow* the value — currently just a
    /// read-only method receiver. (Index-base `arr[i]` was investigated as a
    /// borrow site too — `x.as_ref().unwrap()[i]` — and gives a large clone win,
    /// but for a non-Copy element struct read in a numeric-coercion context it
    /// reshuffles/leaks type-coercion errors (jhlabs +3, jts +4); the bad cases
    /// can't be told from the ~500 good ones by a local predicate — they need
    /// element-Copy-type + coercion-context info. Parked; see TODO §4.1 slice b.)
    fn use_is_read_borrow(&self, e: NodeId) -> bool {
        self.is_readonly_method_receiver(e)
    }

    /// Is `n` inside a loop body between it and `method` (exclusive of `method`)?
    fn within_loop_of(&self, mut n: NodeId, method: NodeId) -> bool {
        while let Some(p) = self.arena.parent(n) {
            if p == method {
                return false;
            }
            if matches!(
                self.arena.kind(p),
                Node::WhileStmt { .. }
                    | Node::DoStmt { .. }
                    | Node::ForStmt { .. }
                    | Node::ForeachStmt { .. }
            ) {
                return true;
            }
            n = p;
        }
        false
    }

    /// An array element read (`a[i]`) of a *non-Copy* element. Indexing a `Vec`
    /// of non-Copy elements moves the element out, so an owned position must
    /// clone; a scalar element is Copy and needs no clone (avoids noise).
    fn is_array_read(&self, e: NodeId) -> bool {
        if let Node::ArrayAccessExpr { name, .. } = self.arena.kind(e) {
            if let Node::NameExpr { name: n } = self.arena.kind(*name) {
                if let Some(jt) = self.decl_java_type_name(n, *name) {
                    return !is_scalar_java_type(&jt);
                }
            }
            // Unknown element type: don't clone (avoid spurious clones on Copy
            // elements; a genuine move error here is left for follow-up).
        }
        false
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
        self.emit_into_option_enum(value, None, arg);
    }

    /// Like `emit_into_option`, but `enum_path` is the target slot's `<Root>Kind`
    /// when it is enum-routed — so the overlays *compose* (`Option(Route(v))`): a
    /// concrete hierarchy member into a nullable routed slot becomes
    /// `Some(Kind::Variant(v))`, not `Some(v)` (which would be `expected Kind,
    /// found Concrete`). Read-gated via `enum_variant_for_expr`, so an already-enum
    /// value is not double-wrapped.
    fn emit_into_option_enum(&mut self, value: NodeId, enum_path: Option<&str>, arg: Arg) {
        if self.expr_nullable(value) {
            let saved = self.expect_option;
            self.expect_option = true;
            self.visit(value, arg);
            self.expect_option = saved;
        } else {
            self.printer.print("Some(");
            let saved = self.expect_option;
            self.expect_option = false;
            let wrap = enum_path
                .and_then(|ep| self.enum_variant_for_expr(value).filter(|(p, _)| p == ep));
            match wrap {
                Some((ep, vn)) => {
                    self.printer.print(&format!("{ep}::{vn}("));
                    // The Option owns its value, so clone a non-Copy borrow rather
                    // than moving out of it.
                    self.emit_moved_value(value, arg);
                    self.printer.print(")");
                }
                None => self.emit_moved_value(value, arg),
            }
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

    /// Render a type node's own type-args as a `<A, B>` string (empty if none).
    /// Used to re-attach a generic parent's args to a map-resolved path.
    fn render_type_args_of(&mut self, t: NodeId) -> String {
        let args = match self.arena.kind(t) {
            Node::ClassOrInterfaceType { type_args, .. } => type_args.clone(),
            Node::ReferenceType { typ, .. } => return self.render_type_args_of(*typ),
            _ => return String::new(),
        };
        if args.is_empty() {
            return String::new();
        }
        let parts: Vec<String> =
            args.iter().map(|&a| self.accept_and_cut(a, None).trim().to_string()).collect();
        format!("<{}>", parts.join(", "))
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
                // `expect_option` (set around an assignment target) must not leak
                // into the base/index sub-expressions — they're plain values.
                let elem_opt = self.array_access_elem_nullable(id) && !self.expect_option;
                let saved = self.expect_option;
                self.expect_option = false;
                self.visit(name, arg);
                // Rust indices are usize; Java's are int.
                self.printer.print("[(");
                self.visit(index, arg);
                self.printer.print(") as usize]");
                self.expect_option = saved;
                // A read of a nullable element (`Vec<Option<T>>`) in a value
                // position yields the owned `T`.
                if elem_opt {
                    self.printer.print(".clone()/* TODO(translation): validate added clone */.unwrap()");
                }
            }
            AssignExpr { .. } => self.visit_assign(id, arg),
            BinaryExpr { .. } => self.visit_binary(id, arg),
            CastExpr { typ, expr } => {
                self.print_java_comment(id, arg);
                // R4 cast-extract: `(Sub) x` where `x` is an enum'd hierarchy
                // root and `Sub` is one of its variants → extract the variant
                // (`match &x { Kind::Sub(v) => v.clone(), _ => unreachable!() }`),
                // a cloned owned `Sub` (matches Java's cast-yields-value).
                let extract = self
                    .type_simple_name(typ)
                    .and_then(|n| self.enum_variant_for_type(&n))
                    .filter(|(ep, _)| self.expr_enum_root(expr).as_ref() == Some(ep));
                if let Some((ep, vname)) = extract {
                    self.printer.print(&format!("(match &("));
                    self.visit(expr, arg);
                    self.printer.print(&format!(") {{ {ep}::{vname}(v) => v.clone()/* TODO(translation): validate added clone */, _ => unreachable!() }})"));
                    return;
                }
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
                    // matching `Self::new*`. `__self` is a value (`let mut
                    // __self: T = …`), not a pointer — reassign it directly (a
                    // `*__self` deref is E0614).
                    let nm = self.call_emitted_name(None, "new", args.len());
                    self.printer.print(&format!("__self = Self::{nm}"));
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
                let throws = self.id.has_throws();
                if let Some(e) = expr {
                    self.printer.print(" ");
                    if throws {
                        self.printer.print("Ok(");
                    }
                    let ret_box_trait = self.enclosing_ret_box_dyn(id);
                    let ret_enum = self.enclosing_ret_enum(id);
                    if self.enclosing_method_nullable(id) {
                        self.emit_into_option_enum(e, ret_enum.as_deref(), arg);
                    } else if let Some(ep) = ret_enum {
                        // `return <concrete>` into an enum'd hierarchy method:
                        // construction-wrap into the variant (plain emit if it's
                        // already the enum).
                        self.emit_enum_wrapped(e, &ep, arg);
                    } else if ret_box_trait
                        .as_deref()
                        .is_some_and(|tr| self.is_object_creation(e) || self.expr_impls_trait(e, tr))
                    {
                        // `return <concrete>` into a `Box<dyn T>` method: a
                        // `new Concrete()`, or any value of an implementing struct.
                        self.printer.print("Box::new(");
                        self.emit_moved_value(e, arg);
                        self.printer.print(")");
                    } else {
                        // Coerce a numeric return value to the method's return type.
                        self.emit_numeric_coerced(e, self.enclosing_method_ret_type(id), arg);
                    }
                    if throws {
                        self.printer.print(")");
                    }
                } else if self.id.is_in_constructor() {
                    // A bare `return;` in a constructor early-exits; the ctor
                    // returns the instance it is building (`-> Type`, not Result).
                    self.printer.print(" __self");
                } else if throws {
                    // A bare `return;` in a `throws` (Result-returning) method.
                    self.printer.print(" Ok(())");
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
                // `while ((line = in.readLine()) != null)` -> `while let
                // Some(mut line) = in.read_line()` (read_line yields Option).
                if let Some((nm, value)) = self.as_readline_assign(condition) {
                    self.printer.print(&format!("while let Some(mut {nm}) = "));
                    self.visit(value, arg);
                    self.printer.print(" ");
                } else {
                    self.printer.print("while ");
                    self.visit(condition, arg);
                    self.printer.print(" ");
                }
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
                self.printer.print(".clone()/* TODO(translation): validate added clone */ ");
                self.encapsulate_if_not_block(body, arg);
            }
            ForStmt { .. } => self.visit_for(id, arg),
            ThrowStmt { expr } => {
                self.print_java_comment(id, arg);
                // `throw new SomeException(msg)` -> `panic!("{:?}", msg)`. Using the
                // constructor argument (not the exception type) keeps it compiling
                // even though the exception type isn't defined.
                self.printer.print("panic!(\"{:?}\", ");
                // Peel nested exception wrappers (`new UncheckedIOException(new
                // IOException(msg))`) down to the innermost ctor argument so the
                // panic carries the *message*, not a stub exception value (which
                // has no `Debug`). A plain ctor uses its first arg; a no-arg ctor
                // falls back to a literal.
                match self.throw_payload(expr) {
                    Some(payload) => self.visit(payload, arg),
                    None => self.printer.print("\"exception\""),
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
                    let body = sanitize_block_comment(&content);
                    self.printer.print("/*");
                    self.printer.print(&body);
                    // A body ending in `/` (e.g. the Java `//*/` close-comment
                    // idiom leaves a trailing `//`) would join the closing `*/`
                    // wrapper into `/*/` → a spurious `/*` nest-open (Rust block
                    // comments nest, unlike Java), leaving the comment
                    // unterminated. Separate them.
                    if body.ends_with('/') {
                        self.printer.print(" ");
                    }
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

    /// The expression a `throw` lowers into the `panic!` message slot. For
    /// `throw new Exc(msg)` it is `msg`; nested exception wrappers
    /// (`new UncheckedIOException(new IOException(msg))`) are peeled to the
    /// innermost ctor argument (the message), since a stub exception value has
    /// no `Debug`. `None` for a no-arg ctor (caller emits a literal). A non-ctor
    /// throw (e.g. `throw e`) is returned as-is.
    fn throw_payload(&self, expr: NodeId) -> Option<NodeId> {
        match self.arena.kind(expr) {
            Node::ObjectCreationExpr { args, .. } if !args.is_empty() => {
                let first = args[0];
                // Peel a nested exception ctor to reach the message it wraps.
                if matches!(self.arena.kind(first), Node::ObjectCreationExpr { .. }) {
                    return self.throw_payload(first);
                }
                Some(first)
            }
            Node::ObjectCreationExpr { .. } => None,
            _ => Some(expr),
        }
    }

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
                // A nullable inherited field read in a value position is unwrapped.
                if !self.expect_option && self.inherited_field_nullable(name) {
                    if self.use_is_read_borrow(id) {
                        // Read-only use-site (method receiver or index base):
                        // borrow through the Option (`&T`) instead of cloning
                        // (§6 use-site borrow).
                        self.printer.print(".as_ref().unwrap()");
                    } else {
                        self.printer.print(".clone()/* TODO(translation): validate added clone */.unwrap()");
                    }
                }
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
        // A `LazyLock<T>` associated const read in a value position must be
        // dereferenced (`*Self::X`), and owned out of the deref via `.clone()`
        // for a non-Copy inner type.
        let lazy_const = decl.map(|(_, r)| self.is_lazylock_field(r)).unwrap_or(false);
        if lazy_const {
            self.printer.print("(*");
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
                // A static field is an associated const. It's `Self::F` when the
                // field belongs to the current type, but an inner class reaching
                // an *outer* class's static must qualify by that type's name
                // (`Outer::F`) — `Self::` would name the inner class.
                let field_owner = self.owner_type_decl(right);
                let here_owner = self.owner_type_decl(id);
                match field_owner {
                    Some(fo) if Some(fo) != here_owner => {
                        if let Some(o) = self.type_decl_name(fo) {
                            self.printer.print(&o.replace('$', "_"));
                            self.printer.print("::");
                        } else {
                            self.printer.print("Self::");
                        }
                    }
                    _ => self.printer.print("Self::"),
                }
            }
        }
        // A nullable read unwrapped in place would move the `Option` out of its
        // borrow (`&self` for a field, `&Option<T>` for a by-ref param); clone it
        // first. A param read multiple times (`p.unwrap().a(); p.unwrap().b()`)
        // also needs the clone. (`Option<Copy>` clones trivially.)
        let s = self.to_snake_if_necessary(name);
        self.printer.print(&s);
        if lazy_const {
            self.printer.print(")");
            // `(*Self::X)` is already a borrow of the static's value; a read-only
            // use-site (method receiver or index base) works on it directly, so
            // skip the owning clone there (§6 use-site borrow). Otherwise own it
            // out of the deref via `.clone()`.
            if self.is_non_copy_name(id) && !self.use_is_read_borrow(id) {
                self.printer.print(".clone()/* TODO(translation): validate added clone */");
            }
        }
        // A nullable value used where the plain value is expected gets unwrapped.
        // An owned local at its last read can be *moved* through the unwrap
        // (`x.unwrap()`) rather than cloned first (§6 use-site borrow — same
        // last-use move applied to the plain-clone site in `emit_moved_value`).
        if nullable && !self.expect_option {
            if self.is_movable_last_use(id) {
                self.printer.print(".unwrap()");
            } else if self.is_non_copy_name(id) && self.use_is_read_borrow(id) {
                // Read-only use-site (method receiver or index base): borrow
                // through the Option (`&T`) instead of cloning out an owned `T`
                // — the call/index work on `&T` identically (§6 use-site borrow).
                self.printer.print(".as_ref().unwrap()");
            } else {
                self.printer.print(".clone()/* TODO(translation): validate added clone */.unwrap()");
            }
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

        // An own type param used by no field (nor the `extends` type) would be
        // E0392 ("never used") and, via `#[derive(Clone, Default)]`, force an
        // `O: Clone`/`O: Default` bound the using code can't satisfy. Phantom such
        // params AND emit manual (unbounded) `Clone`/`Default` impls instead.
        let phantom_params: Vec<NodeId> = {
            let mut v = extra_params.clone();
            for &p in &type_parameters {
                let n = self.type_param_name(p).unwrap_or_default();
                let used = members.iter().any(|&m| {
                    matches!(self.arena.kind(m), Node::FieldDeclaration { modifiers, .. } if !modifiers::is_static(*modifiers))
                        && self.subtree_uses_type(m, &n)
                }) || extends.first().map(|&e| self.subtree_uses_type(e, &n)).unwrap_or(false);
                if !used {
                    v.push(p);
                }
            }
            v
        };
        let manual_impls = phantom_params.len() > extra_params.len();

        // ---- struct ----
        // Clone: so field values can be cloned out from behind `&self`.
        // Default: so generated `new(...) -> Self` can start from a default value.
        // (A struct with an unused own type param uses manual impls instead — a
        // derive would bound that param `Clone + Default` spuriously.)
        // Equality (`PartialEq`/`Eq`/`Hash`) is added only when *every* field is
        // an unconditionally-derivable type (primitive, `String`, `Unknown`, or a
        // `Vec`/`Option`/`Box` of those). A field that is another struct/enum, a
        // trait object, or a float excludes the struct (conservative: no
        // cross-struct dependency, so no cascade — an earlier local guard that
        // allowed struct fields regressed badly). Classes with *synthesized*
        // fields not in `members` are also excluded: an inherited `base`
        // (`extends`) and a capturing inner class's `__outer: Rc<RefCell<…>>`.
        let eligible =
            !manual_impls && extends.is_empty() && self.enclosing_class_fqn.is_none();
        let (mut can_partial_eq, mut can_eq_hash) = (eligible, eligible);
        for &m in &members {
            if let Node::FieldDeclaration { typ, modifiers, .. } = self.arena.kind(m) {
                if modifiers::is_static(*modifiers) {
                    continue;
                }
                let typ = *typ;
                let ty = self.accept_and_cut(typ, None).trim().to_string();
                let (pe, eh) = Self::type_derives_eq(&ty);
                can_partial_eq &= pe;
                can_eq_hash &= eh;
            }
        }
        if !manual_impls {
            let mut d = String::from("Clone, Default");
            if can_partial_eq {
                d.push_str(", PartialEq");
            }
            if can_eq_hash {
                d.push_str(", Eq, Hash");
            }
            self.printer.print_ln_s(&format!("#[derive({d})]"));
        }
        self.print_modifiers(modifiers_v);
        self.printer.print("struct ");
        self.printer.print(&name);
        self.print_type_parameters(&combined, arg);
        let _ = &implements; // `implements` -> traits is Stage 2; not modelled here.
        // Single inheritance via composition: embed the superclass as `base`.
        // Prefer the symbol map's resolved parent path — bare-name resolution of
        // the `extends` type misses a *nested* (inner-class) parent and would
        // pick the empty stub, but `resolve_parent` records the real project FQN.
        let map_parent_path = self
            .current_class_fqn
            .as_deref()
            .and_then(|f| self.link.lookup(f))
            .and_then(|t| t.parent.clone())
            .and_then(|p| self.link.lookup(&p).map(|pt| self.crate_relativize(&pt.rust_path)));
        let parent_rust = if let Some(p) = &map_parent_path {
            // The map path is the bare rust path (no type args); re-attach the
            // `extends` clause's args so a generic parent stays `Hmm<O>`.
            let p = p.clone();
            let targs = extends.first().map(|&e| self.render_type_args_of(e)).unwrap_or_default();
            Some(format!("{p}{targs}"))
        } else {
            extends.first().map(|&e| self.accept_and_cut(e, arg).trim().to_string())
        };
        // External (stub) superclass? Then bare inherited fields go through `base`.
        // A parent resolved via the map is a known project type, so inherited
        // members resolve through it (not the external-base path).
        let saved_ext_base = self.current_external_base.take();
        self.current_external_base = if map_parent_path.is_some() {
            None
        } else {
            extends.first().and_then(|&e| {
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
            })
        };
        self.printer.print_ln_s(" {");
        self.printer.indent();
        if let Some(p) = &parent_rust {
            self.printer.print_ln_s(&format!("pub base: {p},"));
        }
        // Carried outer type params + unused own params must appear in a field
        // (E0392) — `PhantomData`.
        if !phantom_params.is_empty() {
            let names: Vec<String> =
                phantom_params.iter().filter_map(|&p| self.type_param_name(p)).collect();
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
        // Synthesized `impl PartialEq`/`Eq`/`Hash` for a struct that can't
        // `#[derive]` them (a subtype via `base`, or a `Map`/`Set`-bearing value
        // type) but whose fields are all comparable/hashable (per the
        // `crate_layout` capability fixpoint). Field-wise `==`; `Hash` folds a
        // top-level map/set order-independently (mirrors Java `Map.hashCode()`).
        self.emit_synth_eq_impls(&name, &members, parent_rust.is_some(), can_partial_eq, can_eq_hash, manual_impls, combined.is_empty());
        // R4: if this struct is a project-hierarchy root, emit its `<Root>Kind`
        // tagged enum (+ Deref to the root) so supertype slots can carry subtypes.
        if let Some(root_fqn) = self.current_class_fqn.clone() {
            self.emit_hierarchy_enum(&name, &root_fqn);
        }
        // Manual (unbounded) `Default`/`Clone` for a struct with an unused own
        // type param — replaces the derive, which would bound that param.
        if manual_impls {
            let mut field_names: Vec<String> = Vec::new();
            if parent_rust.is_some() {
                field_names.push("base".to_string());
            }
            if !phantom_params.is_empty() {
                field_names.push("__phantom".to_string());
            }
            if self.enclosing_class_fqn.as_deref().and_then(|f| self.link.lookup(f)).is_some() {
                field_names.push("__outer".to_string());
            }
            for &m in &members {
                if let Node::FieldDeclaration { modifiers, variables, .. } = self.arena.kind(m) {
                    if !modifiers::is_static(*modifiers) {
                        for &var in variables {
                            field_names.push(self.field_var_name(var));
                        }
                    }
                }
            }
            let field_init = |f: &str, src: &str| {
                if f == "__phantom" {
                    format!("            {f}: std::marker::PhantomData,")
                } else {
                    format!("            {f}: {src},")
                }
            };
            for (trait_name, body) in [("Default", "Default::default()"), ("Clone", "")] {
                self.printer.print("impl");
                self.print_type_parameters(&combined, arg);
                self.printer.print(&format!(" {trait_name} for {name}"));
                self.print_type_param_names(&combined);
                self.printer.print_ln_s(" {");
                if trait_name == "Default" {
                    self.printer.print_ln_s("    fn default() -> Self {");
                } else {
                    self.printer.print_ln_s("    fn clone(&self) -> Self {");
                }
                self.printer.print_ln_s("        Self {");
                for f in &field_names {
                    let src =
                        if body.is_empty() { format!("self.{f}.clone()") } else { body.to_string() };
                    self.printer.print_ln_s(&field_init(f, &src));
                }
                self.printer.print_ln_s("        }");
                self.printer.print_ln_s("    }");
                self.printer.print_ln_s("}");
            }
        }
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
            // A project class extending a Java reader/stream carrier (e.g.
            // `class SmartFileReader extends FileReader`) must itself `impl Read`
            // so it composes into the IO factory fns (which bound their arg
            // `R: std::io::Read`); `Deref` alone doesn't satisfy a generic bound.
            if p == "crate::java_runtime::JavaReader" || p == "crate::java_runtime::JavaInputStream" {
                self.printer.print("impl");
                self.print_type_parameters(&combined, arg);
                self.printer.print(" std::io::Read for ");
                self.printer.print(&name);
                self.print_type_param_names(&combined);
                self.printer.print_ln_s(" {");
                self.printer.print_ln_s("    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {");
                self.printer.print_ln_s("        std::io::Read::read(&mut &self.base, buf)");
                self.printer.print_ln_s("    }");
                self.printer.print_ln_s("}");
            }
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
            // C-style trailing dims (`long pack[];`) attach to the declarator,
            // not the shared field type — wrap the rendered type in `Vec<>`.
            let array_count = match self.arena.kind(var) {
                Node::VariableDeclarator { array_count, .. } => *array_count,
                _ => 0,
            };
            // `pub` so fields accessed cross-module (`x.field`) resolve.
            self.printer.print(&format!("pub {name}: "));
            if nullable {
                self.printer.print("Option<");
            }
            let ty = self.accept_and_cut(typ, None).trim().to_string();
            let ty = (0..array_count).fold(ty, |t, _| format!("Vec<{t}>"));
            let ty = if self.var_decl_id(var).map(|d| self.decl_elem_nullable(d)).unwrap_or(false) {
                Self::wrap_elem_option(&ty)
            } else {
                ty
            };
            self.printer.print(&ty);
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
                    self.printer.print(&format!("pub const {name}: &'static str = "));
                    let saved = self.raw_string;
                    self.raw_string = true;
                    self.visit(i, None);
                    self.raw_string = saved;
                    self.printer.print_ln_s(";");
                }
                // Other const-evaluable literal (numeric / bool / char).
                Some(i) if self.is_const_literal(i) => {
                    self.printer.print(&format!("pub const {name}: "));
                    self.visit(typ, None);
                    self.printer.print(" = ");
                    self.visit(i, None);
                    self.printer.print_ln_s(";");
                }
                // Non-const initializer (constructor, Vec::new, method call):
                // wrap in `LazyLock`. `LazyLock::new` is a `const fn`, so this is
                // a valid associated `const` (a `static` is forbidden in `impl`).
                Some(i) => {
                    self.printer.print(&format!("pub const {name}: std::sync::LazyLock<"));
                    self.visit(typ, None);
                    self.printer.print("> = std::sync::LazyLock::new(|| ");
                    // A concrete value into a `Box<dyn Trait>` field needs boxing.
                    let box_it = Self::box_dyn_trait_simple(&type_str)
                        .is_some_and(|tr| self.is_object_creation(i) || self.expr_impls_trait(i, &tr));
                    if box_it {
                        self.printer.print("Box::new(");
                        self.visit(i, None);
                        self.printer.print(")");
                    } else {
                        self.visit(i, None);
                    }
                    self.printer.print_ln_s(");");
                }
                // No initializer: fall back to a const default.
                None => {
                    self.printer.print(&format!("pub const {name}: "));
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
        // R4: at a value-storage slot, a supertype with a synthesized enum renders
        // as that enum (so subtype values dispatch through it). `slot_enum_name`
        // returns `None` until enum synthesis (step 2) populates it, so this is
        // currently a no-op.
        let resolved = match self.slot_enum_name(&name).filter(|_| self.is_slot_type(id)) {
            Some(enum_name) => self.crate_relativize(&enum_name),
            None => self.resolve_type_name(&name),
        };
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
        } else if type_args.is_empty() {
            // Tier-2: a RAW collection whose element was inferred (same map the
            // resolver uses, keyed by this type node) → emit `Vec<Elem>` so the
            // declaration and its `.get()`/iteration agree on the element.
            if let Some(elem) = self.collection_elem_map().get(&id).cloned() {
                self.printer.print(&format!("<{}>", self.elem_type_to_rust(&elem)));
            } else {
                self.print_type_args(&type_args, arg);
            }
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
        let (vid, init, array_count) = match self.kind(id) {
            Node::VariableDeclarator { id: vid, init, array_count } => (vid, init, array_count),
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
            // `pub` so the associated const resolves cross-module (`Type::NAME`);
            // Java's `static final` are effectively public for our purposes, and
            // over-exposing a private one is harmless.
            self.printer.print("pub const ");
            is_constant = true;
        } else {
            self.printer.print("let ");
            // A linked `&mut self` call on this local also requires `let mut`.
            let java_name = match self.arena.kind(vid) {
                Node::VariableDeclaratorId { name } => name.clone(),
                _ => name.clone(),
            };
            // `is_changed` is keyed by the *Java* name (the change-tracker records
            // the original identifier), not the snake-cased Rust name.
            if self.id.is_changed(self.arena, &java_name, id)
                || self.mut_borrow_params.contains(&java_name)
            {
                self.printer.print("mut ");
            }
        }
        self.printer.print(&name);
        let nullable = self.decl_nullable(vid);
        // The declared type's trait, if it's an owned trait object (`Box<dyn
        // T>`): a concrete initializer then needs `Box::new(..)` to coerce.
        let mut box_dyn_target: Option<String> = None;
        // The declared type's `<Root>Kind` enum path, if it's a plain enum'd
        // hierarchy slot (not array/nullable) — a concrete initializer is then
        // construction-wrapped into the variant.
        let mut enum_target: Option<String> = None;
        if self.is_type(arg) {
            if array_count == 0 && !nullable {
                if let Some(s) = self.type_simple_name(arg.unwrap()) {
                    enum_target = self.slot_enum_name(&s);
                }
            }
            let tmp = self.accept_and_cut(arg.unwrap(), None);
            let tmp = tmp.trim().to_string();
            // C-style trailing dims (`String tokens[]`) wrap the shared type.
            let tmp = (0..array_count).fold(tmp, |t, _| format!("Vec<{t}>"));
            let tmp = if self.decl_elem_nullable(vid) {
                Self::wrap_elem_option(&tmp)
            } else {
                tmp
            };
            box_dyn_target = Self::box_dyn_trait_simple(&tmp);
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
            // Java widens a narrower numeric to the declared type (`long x =
            // intExpr`); Rust needs an explicit cast.
            let widen = arg
                .and_then(|t| self.rust_type_of(t))
                .filter(|t| is_numeric_rust(t))
                .filter(|t| {
                    self.expr_num_type(i).map(|s| num_rank(&s) < num_rank(t)).unwrap_or(false)
                });
            if nullable {
                self.emit_into_option_enum(i, enum_target.as_deref(), arg);
            } else if char_from_int {
                self.visit(i, arg);
                self.printer.print(" as u8 as char");
            } else if let Some(t) = widen {
                self.printer.print("(");
                self.visit(i, arg);
                self.printer.print(&format!(") as {t}"));
            } else if let Some(ep) = enum_target.clone() {
                // `<Root>Kind x = <concrete>` -> wrap the value in its variant
                // (plain emit if it's already the enum).
                self.emit_enum_wrapped(i, &ep, arg);
            } else if box_dyn_target
                .as_deref()
                .is_some_and(|tr| self.is_object_creation(i) || self.expr_impls_trait(i, tr))
            {
                // `Box<dyn T> x = <concrete>` -> `Box::new(..)`. Fires for a
                // `new Concrete()` (can't already be boxed) or any value of a
                // struct implementing `T`. A method/factory value of unknown
                // type stays unwrapped (its return may already be `Box<dyn T>`,
                // so wrapping would double-box).
                self.printer.print("Box::new(");
                self.emit_moved_value(i, arg);
                self.printer.print(")");
            } else {
                self.emit_moved_value(i, arg);
            }
        }
    }

    /// Is `e` a `new Concrete(...)` expression (through parentheses)? The
    /// unambiguous "concrete value" case for `Box::new` coercion into a
    /// `Box<dyn T>` slot — unlike a method value, it can't already be boxed.
    fn is_object_creation(&self, e: NodeId) -> bool {
        match self.arena.kind(e) {
            Node::ObjectCreationExpr { .. } => true,
            Node::EnclosedExpr { inner: Some(i) } => self.is_object_creation(*i),
            _ => false,
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
        // Elements are owned, so a non-Copy name/field read is cloned out of its
        // borrow (`vec![chrom1, chrom2]` where the elements are `&String`).
        self.printer.print("vec![");
        for &val in &values {
            self.emit_moved_value(val, None);
            self.printer.print(", ");
        }
        self.printer.print("]");
    }

    fn default_value(&self, ty: &str) -> String {
        match ty {
            "f64" | "f32" => "0.0",
            "u64" | "u32" | "u16" | "u8" | "usize" | "i64" | "i32" | "i16" | "i8" => "0",
            "bool" => "false",
            // Java zero-initializes `char[]` to `'\0'` (ascii 0), not null.
            "char" => "'\\0'",
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
            // `new T[n]` zero-fills. For a non-primitive element the right
            // default depends on the Rust representation (`Option`-wrapped or
            // not), so emit `Default::default()` — it is `None` for an
            // `Option<T>` element and the derived default otherwise, both
            // correct, where a bare `None` only fits the nullable case.
            let default = match self.default_value(&ty).as_str() {
                "None" => "Default::default()".to_string(),
                d => d.to_string(),
            };
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
        // Java chained assignment `a = b = v` is an expression yielding `v`; Rust
        // assignment yields `()`. Lower `a = b = … = v` to a block that evaluates
        // `v` once and assigns it to every target.
        if matches!(op, AssignOp::Assign)
            && matches!(self.arena.kind(value), Node::AssignExpr { op: AssignOp::Assign, .. })
        {
            self.emit_chained_assign(id, arg);
            return;
        }
        // Java `String += x` is concatenation, but Rust has no `AddAssign<String>`
        // for `String` -> `target.push_str(&(x).to_string())`. Gated on a String
        // target (numeric `+=` resolves to `i32`/`i64` and keeps `+=`).
        if matches!(op, AssignOp::Plus)
            && self.assign_target_rust_type(target).as_deref() == Some("String")
            && !self.expr_nullable(target)
        {
            self.visit(target, arg);
            self.printer.print(".push_str(&(");
            self.visit(value, arg);
            self.printer.print(").to_string())");
            return;
        }
        // Assigning to a nullable slot: keep the target as the bare Option (no
        // unwrap) and wrap the value with Some/None. Includes an element-nullable
        // array element (`arr[i] = x` -> `arr[i] = Some(x)`).
        let target_nullable = matches!(op, AssignOp::Assign)
            && (self.expr_nullable(target) || self.array_access_elem_nullable(target));
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
            let tep = self.assign_target_enum_path(target);
            self.emit_into_option_enum(value, tep.as_deref(), arg);
        } else if matches!(op, AssignOp::Assign) {
            // `<enum-routed target> = <concrete member>` -> wrap into the variant
            // (read-gated `enum_variant_for_expr` won't double-wrap an already-enum
            // RHS). Target-side parallel of the method-arg wrap.
            let enum_wrap = self.assign_target_enum_path(target).and_then(|ep| {
                self.enum_variant_for_expr(value).filter(|(vep, _)| *vep == ep)
            });
            let box_tr = self.assign_target_trait(target);
            if let Some((ep, vn)) = enum_wrap {
                self.printer.print(&format!("{ep}::{vn}("));
                self.emit_moved_value(value, arg);
                self.printer.print(")");
            } else if box_tr
                .as_deref()
                .is_some_and(|tr| self.is_object_creation(value) || self.expr_impls_trait(value, tr))
            {
                // `self.field = <concrete>` where the field is `Box<dyn Trait>`.
                self.printer.print("Box::new(");
                self.emit_moved_value(value, arg);
                self.printer.print(")");
            } else {
                // Coerce a numeric RHS to the target's type (Java widens/narrows;
                // e.g. `long x = intExpr`, `float f = doubleExpr`).
                self.emit_numeric_coerced(value, self.assign_target_rust_type(target), arg);
            }
        } else {
            // Compound assignment (`+=`, …): Java promotes the RHS to the target.
            let target_ty = self.assign_target_rust_type(target);
            // `f64 += 1`: an integer literal RHS won't infer as the float target
            // (numeric_coercion skips literals), so cast it explicitly. The
            // target's type comes from the resolver (robust for locals).
            let cast = self.numeric_coercion(target_ty, value).or_else(|| {
                match self.ty(target).numeric_rust() {
                    Some(t @ ("f64" | "f32")) if self.is_integer_literal(value) => {
                        Some(t.to_string())
                    }
                    _ => None,
                }
            });
            match cast {
                Some(t) => {
                    self.printer.print("(");
                    self.visit(value, arg);
                    self.printer.print(&format!(") as {t}"));
                }
                None => self.visit(value, arg),
            }
        }
    }

    /// Lower a Java chained assignment (`a = b = … = v`) to a block:
    /// `{ let __chain = v; a = __chain.clone(); …; __chain }`. The block yields
    /// the assigned value, so it works in both statement and expression position.
    fn emit_chained_assign(&mut self, id: NodeId, arg: Arg) {
        let mut targets = Vec::new();
        let mut cur = id;
        let rhs = loop {
            match self.arena.kind(cur) {
                Node::AssignExpr { target, op: AssignOp::Assign, value } => {
                    targets.push(*target);
                    cur = *value;
                }
                _ => break cur,
            }
        };
        self.printer.print("{ let __chain = ");
        self.emit_moved_value(rhs, arg);
        self.printer.print("; ");
        for &t in &targets {
            self.visit(t, arg);
            self.printer.print(" = __chain.clone()/* TODO(translation): validate added clone */; ");
        }
        self.printer.print("__chain }");
    }

    /// A `getClass()` call (any receiver, no args), possibly parenthesized.
    fn is_get_class_call(&self, n: NodeId) -> bool {
        match self.arena.kind(n) {
            Node::MethodCallExpr { name, args, .. } => name == "getClass" && args.is_empty(),
            Node::EnclosedExpr { inner: Some(i) } => self.is_get_class_call(*i),
            _ => false,
        }
    }

    /// Recognize the Java read-loop idiom `(name = recv.readLine()) != null` as a
    /// `while`/`if` condition. Returns the assigned variable's snake-cased name
    /// and the `readLine()` call node, so the loop can lower to
    /// `while let Some(mut name) = recv.read_line()`. Gated to `readLine` (whose
    /// stub return is overridden to `Option<String>`) — a different call would
    /// not typecheck under `while let Some`. The whole condition must be the
    /// comparison (a `&&` conjunct is a different `BinaryExpr` and is excluded).
    fn as_readline_assign(&self, cond: NodeId) -> Option<(String, NodeId)> {
        let cond = match self.arena.kind(cond) {
            Node::EnclosedExpr { inner: Some(i) } => *i,
            _ => cond,
        };
        let Node::BinaryExpr { left, op, right } = self.arena.kind(cond) else {
            return None;
        };
        if !matches!(op, BinaryOp::NotEquals) {
            return None;
        }
        let (left, right) = (*left, *right);
        let l_null = matches!(self.arena.kind(left), Node::NullLiteralExpr);
        let r_null = matches!(self.arena.kind(right), Node::NullLiteralExpr);
        if l_null == r_null {
            return None;
        }
        let assign = if l_null { right } else { left };
        let assign = match self.arena.kind(assign) {
            Node::EnclosedExpr { inner: Some(i) } => *i,
            _ => assign,
        };
        let Node::AssignExpr { target, op, value } = self.arena.kind(assign) else {
            return None;
        };
        if !matches!(op, AssignOp::Assign) {
            return None;
        }
        let Node::NameExpr { name } = self.arena.kind(*target) else {
            return None;
        };
        let Node::MethodCallExpr { name: m, args, .. } = self.arena.kind(*value) else {
            return None;
        };
        if m != "readLine" || !args.is_empty() {
            return None;
        }
        Some((self.to_snake_if_necessary(name), *value))
    }

    /// The type node of a `Foo.class` (`ClassExpr`) scope, seeing through
    /// parentheses, else `None`.
    fn class_expr_type(&self, n: NodeId) -> Option<NodeId> {
        match self.arena.kind(n) {
            Node::ClassExpr { typ } => Some(*typ),
            Node::EnclosedExpr { inner: Some(i) } => self.class_expr_type(*i),
            _ => None,
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
                // If the non-null operand has a KNOWN concrete (non-`Option`,
                // non-`Unknown`) Rust type, it can never be null — fold to a
                // constant rather than emitting `.is_some()`/`.is_none()` on a
                // non-`Option` type (E0599). Genuinely nullable values resolve to
                // `Option` (or `Unknown`) and keep the Option check. Mirrors the
                // `getClass()`-idiom constant fold below.
                //
                // NO-GO (measured 2026-06): gating this on `!expr_nullable(other)`
                // (the `N` overlay) instead of `self.ty`-concrete — the SEMANTICS
                // §12-item-7 prescription (docs/tech-debt.md A1) — REGRESSES
                // (total +68; jaligner +1, jahmm +1, jhlabs +6, jsoup +49,
                // jts +11). The only behavior change is `(self.ty concrete ∧
                // expr_nullable)` flipping a compiling constant-fold into
                // `.is_some()/.is_none()`; for the many locals/params whose
                // `nullable` flag is TRUE yet whose emission is concrete (the ~32
                // is_some/unwrap-on-concrete inconsistency, e.g. `is_none` on a
                // concrete `FormatFactory`) that yields E0599. There is NO
                // error-reducing cell — the fold→Option-check fix is purely
                // semantic. Real fix needs the nullability analysis made
                // consistent (nullable-flagged ⇒ emitted `Option<T>`) first; see
                // TODO.md §1 and the tier2 frontier.
                let ty = self.ty(other);
                let never_null = !matches!(
                    ty,
                    crate::types::Type::Opt(_) | crate::types::Type::Unknown
                );
                if never_null {
                    self.print_java_comment(id, arg);
                    self.printer
                        .print(if matches!(op, BinaryOp::Equals) { "false" } else { "true" });
                    return;
                }
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
        // Java promotes mixed-width numeric operands (`float * int` -> both
        // `float`); Rust requires matching types. When both operand types are
        // known and differ, cast the narrower to the wider — exactly Java's rule.
        let (cast_l, cast_r) = self.numeric_promotion(op, left, right);
        self.emit_operand(left, cast_l.as_deref(), arg);
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
        self.emit_operand(right, cast_r.as_deref(), arg);
    }

    /// Emit an operand, optionally cast to `cast` (`(<expr> as <cast>)`).
    fn emit_operand(&mut self, e: NodeId, cast: Option<&str>, arg: Arg) {
        match cast {
            Some(t) => {
                self.printer.print("(");
                self.visit(e, arg);
                self.printer.print(&format!(" as {t})"));
            }
            None => self.visit(e, arg),
        }
    }

    /// For an arithmetic binary op, returns the cast (if any) each operand needs
    /// so both share Java's promoted type. Casts only when *both* operand types
    /// are known numerics — otherwise nothing (leaving the code unchanged).
    fn numeric_promotion(
        &self,
        op: BinaryOp,
        left: NodeId,
        right: NodeId,
    ) -> (Option<String>, Option<String>) {
        use BinaryOp::*;
        // Arithmetic/bitwise ops and ordering comparisons require matching
        // operand types (Java promotes the narrower); shifts do not (the right
        // operand is a count), so they are excluded.
        let is_arith = matches!(op, Plus | Minus | Times | Divide | Remainder | BinOr | BinAnd | Xor);
        let is_cmp = matches!(op, Less | Greater | LessEquals | GreaterEquals);
        if !is_arith && !is_cmp {
            return (None, None);
        }
        // Java promotes `char` to `int` in *arithmetic* (`ch - 'A'`); Rust has no
        // char arithmetic. Char *comparisons* (`ch < 'Z'`) stay char-vs-char.
        if is_arith {
            let l_char = self.expr_is_char(left);
            let r_char = self.expr_is_char(right);
            if l_char || r_char {
                return (
                    l_char.then(|| "i32".to_string()),
                    r_char.then(|| "i32".to_string()),
                );
            }
        }
        let (Some(l), Some(r)) = (self.expr_num_type(left), self.expr_num_type(right)) else {
            return (None, None);
        };
        if l == r {
            return (None, None);
        }
        let t = if num_rank(&l) >= num_rank(&r) { l.clone() } else { r.clone() };
        let t_is_float = t.starts_with('f');
        // A numeric literal usually needs no cast (Rust infers its type from
        // context). EXCEPTION: an integer literal promoted to a float type that
        // the float-literal emission won't already float (`is_float_in_history`)
        // — e.g. `f32_arr[i][j] > 0`, where the sibling's float-ness isn't
        // tracked — needs an explicit `(0) as f32`.
        let needs_cast = |me: &Self, side: NodeId, side_ty: &str| -> bool {
            if side_ty == t {
                return false;
            }
            if !me.is_numeric_literal(side) {
                return true;
            }
            t_is_float && me.is_integer_literal(side) && !me.is_float_in_history(Some(side))
        };
        let cast_l = needs_cast(self, left, &l).then(|| t.clone());
        let cast_r = needs_cast(self, right, &r).then(|| t.clone());
        (cast_l, cast_r)
    }

    fn is_numeric_literal(&self, e: NodeId) -> bool {
        match self.arena.kind(e) {
            Node::IntegerLiteralExpr { .. }
            | Node::LongLiteralExpr { .. }
            | Node::DoubleLiteralExpr { .. } => true,
            Node::EnclosedExpr { inner: Some(i) } => self.is_numeric_literal(*i),
            _ => false,
        }
    }

    /// An *integer* literal (`5`/`5L`), excluding float literals. Used to decide
    /// when a literal operand still needs a cast: an int literal promoted to a
    /// float type can't be inferred by Rust (`f32_expr > 0` needs `0 as f32`).
    fn is_integer_literal(&self, e: NodeId) -> bool {
        match self.arena.kind(e) {
            Node::IntegerLiteralExpr { .. } | Node::LongLiteralExpr { .. } => true,
            Node::EnclosedExpr { inner: Some(i) } => self.is_integer_literal(*i),
            _ => false,
        }
    }

    /// If a numeric `value` must be coerced to a known numeric `target_ty` (both
    /// known and *different*, and `value` isn't a bare literal Rust would infer),
    /// return the target type to cast to. Mirrors the declarator-widening guards.
    fn numeric_coercion(&self, target_ty: Option<String>, value: NodeId) -> Option<String> {
        let t = target_ty.filter(|t| is_numeric_rust(t))?;
        let s = self.expr_num_type(value)?;
        (s != t && !self.is_numeric_literal(value)).then_some(t)
    }

    /// Emit a call argument, casting a numeric value to the param's numeric type
    /// when they differ. Unlike [`Self::numeric_coercion`] this does *not* skip
    /// literals: `f(10)` for an `f32` param needs `(10) as f32` (Rust won't infer
    /// an integer literal as a float across a call boundary). Non-numeric params
    /// emit a plain moved value.
    fn emit_numeric_arg(&mut self, value: NodeId, target_ty: &str, arg: Arg) {
        let cast = is_numeric_rust(target_ty)
            && self.expr_num_type(value).is_some_and(|s| s != target_ty);
        if cast {
            self.printer.print("(");
            self.visit(value, arg);
            self.printer.print(&format!(") as {target_ty}"));
        } else {
            self.emit_moved_value(value, arg);
        }
    }

    /// Emit `value`, cast to `target_ty` if numeric coercion is needed, else as a
    /// moved value (clone for non-Copy).
    fn emit_numeric_coerced(&mut self, value: NodeId, target_ty: Option<String>, arg: Arg) {
        match self.numeric_coercion(target_ty, value) {
            Some(t) => {
                self.printer.print("(");
                self.visit(value, arg);
                self.printer.print(&format!(") as {t}"));
            }
            None => self.emit_moved_value(value, arg),
        }
    }

    /// Best-effort numeric Rust type of an expression (`f64`/`f32`/`i64`/`i32`/
    /// `i16`/`i8`), or `None` if not a determinable numeric.
    ///
    /// NOTE: deliberately *not* delegating to the unified resolver yet. The
    /// resolver resolves strictly more numerics (map values, boxed unboxing,
    /// method chains), and the numeric-coercion sites that consume this were
    /// tuned to the narrower coverage — switching here regresses (bjaaprop +7).
    /// Re-migrate together with `numeric_coercion`/`emit_numeric_*` (Phase 3).
    fn expr_num_type(&self, e: NodeId) -> Option<String> {
        self.ty(e).numeric_rust().map(str::to_string)
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
        // Boxed-primitive constants (`Integer.MAX_VALUE` -> `i32::MAX`, ...).
        if let Node::NameExpr { name: cls } = self.arena.kind(scope) {
            if self.id.find_declaration_node_for(self.arena, cls, scope).is_none() {
                if let Some(c) = boxed_constant(cls, field.as_str()) {
                    self.printer.print(c);
                    return;
                }
            }
        }
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
        // Record a static-final constant access on a stub type
        // (`TimeUnit.MILLISECONDS`) so the stub emits an associated `const`.
        if self.emit_stubs && self.is_static_class_ref(scope) {
            if let Node::NameExpr { name } = self.arena.kind(scope) {
                let is_const = field.chars().any(|c| c.is_ascii_uppercase())
                    && field.chars().all(|c| c.is_ascii_uppercase() || c == '_' || c.is_ascii_digit());
                if is_const {
                    if let Some(key) = self.missing_type_key(name) {
                        let rust_struct = map_type_name(name).replace('$', "_");
                        let c = self.to_snake_if_necessary(&field);
                        self.stubs.borrow_mut().add_static_const(&key, &rust_struct, &c);
                    }
                }
            }
        }
        // Array `.length` -> `(recv.len() as i32)`: Java `length` is an `int`, so
        // the cast keeps it comparable/assignable to the `i32`-typed surroundings
        // (mirrors the `.length()`/`.size()` method rewrites). Skipped for
        // `this.length` when the class declares a real `length` field — that's a
        // genuine field, not the array pseudo-field (the `.len()` form is also an
        // invalid assignment target, E0070).
        let this_real_length_field = field == "length"
            && matches!(self.arena.kind(scope), Node::ThisExpr { .. })
            && self.class_field_names.contains("length");
        if field == "length" && !self.is_static_class_ref(scope) && !this_real_length_field {
            self.printer.print("(");
            self.emit_scope(scope, arg);
            self.printer.print(".len() as i32)");
            return;
        }
        self.emit_scope(scope, arg);
        self.printer.print(if self.is_static_class_ref(scope) { "::" } else { "." });
        let f = self.to_snake_if_necessary(&field);
        self.printer.print(&f);
        // A nullable `this.field` read in a value position yields the owned `T`
        // (mirrors `visit_name_expr`). `expect_option` suppresses it for
        // lvalue / `Option`-slot / null-compare contexts.
        if !self.expect_option
            && matches!(self.arena.kind(scope), Node::ThisExpr { .. })
            && self.this_field_nullable(&field, id)
        {
            if self.use_is_read_borrow(id) {
                // Read-only use-site (method receiver or index base): borrow
                // `&self.field` through the Option (`&T`) instead of cloning
                // (§6 use-site borrow); `&self` is held, so `.as_ref()` is sound.
                self.printer.print(".as_ref().unwrap()");
            } else {
                self.printer.print(".clone()/* TODO(translation): validate added clone */.unwrap()");
            }
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
                // Java `getName()` returns a `String`, so own the `&'static str`.
                self.print_java_comment(id, arg);
                self.printer.print("std::any::type_name::<Self>().to_string()");
                return;
            }
            // `Foo.class.getName()` -> `type_name::<Foo>()` (the scope is a
            // `ClassExpr`, which on its own emits a `TypeId` with no `getName`).
            if matches!(name.as_str(), "getName" | "getSimpleName" | "getCanonicalName") {
                if let Some(typ) = scope.and_then(|s| self.class_expr_type(s)) {
                    self.print_java_comment(id, arg);
                    self.printer.print("std::any::type_name::<");
                    self.visit(typ, arg);
                    self.printer.print(">().to_string()");
                    return;
                }
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
        let callee = self.resolve_linked_callee(scope, &name, &args);
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
            if self.try_emit_boxed_static(scope, &name, &args, arg) {
                return;
            }
            // `Optional.of/ofNullable/empty` and `IntStream/LongStream.range/
            // rangeClosed` are now `static_rule` entries (handled by
            // `try_emit_stdlib` below — nothing between here and there matches a
            // static `Optional`/`IntStream` receiver).
            if self.try_emit_string_format(scope, &name, &args, arg) {
                return;
            }
            if self.try_emit_known_method(scope, &name, &args, arg) {
                return;
            }
            if self.try_emit_stdlib(scope, &name, &args, arg) {
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
                            // A static method: `Self::name` for the current type,
                            // but `Outer::name` when an inner class calls an
                            // enclosing class's static (mirrors the static-field
                            // qualification in `visit_name_expr`).
                            let owner = self.owner_type_decl(right);
                            let here = self.owner_type_decl(id);
                            match owner {
                                Some(o) if Some(o) != here => {
                                    match self.type_decl_name(o) {
                                        Some(n) => {
                                            self.printer.print(&n.replace('$', "_"));
                                            self.printer.print("::");
                                        }
                                        None => self.printer.print("Self::"),
                                    }
                                }
                                _ => self.printer.print("Self::"),
                            }
                        } else {
                            self.printer.print(recv);
                        }
                    }
                    // A non-callable shadow (e.g. a same-named local): in a static
                    // method a bare call is still `Self::` (no `self`).
                    _ => self.printer.print(if self.in_static_method { "Self::" } else { recv }),
                }
            } else if self.inherited_method(&name) {
                // Inherited method. From an instance context, `self.m()` dispatches
                // through Deref. From a static context the method is necessarily a
                // *static* inherited one, which Rust associated fns don't inherit
                // through Deref -> qualify by the declaring parent (`Parent::m`).
                if self.in_static_method {
                    match self.inherited_method_owner(&name) {
                        Some(p) => self.printer.print(&format!("{p}::")),
                        None => self.printer.print("Self::"),
                    }
                } else {
                    self.printer.print(recv);
                }
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
                // A `throws` method returns `Result<_, String>`; unwrap to the
                // value (before any nullable unwrap, which peels the inner Option).
                if m.throws {
                    self.printer.print(".unwrap()");
                }
                if m.ret_nullable && !self.expect_option {
                    self.printer.print(".unwrap()");
                }
            }
            None => {
                self.record_missing_call(scope, &name, &args, id);
                let s = self.call_emitted_name(scope, &name, args.len());
                self.printer.print(&s);
                // Resolve a bare self-call's callee (current class or an
                // ancestor) once: its signature drives argument borrowing AND the
                // throws/nullable unwraps below — `has_throws_name`/
                // `name_decl_nullable` only see the *current file*, so an
                // inherited `throws`/nullable method would otherwise be missed.
                let callee = if scope.is_none() {
                    self.resolve_self_callee(&name, args.len())
                        .map(|m| (m.params.clone(), m.throws, m.ret_nullable))
                } else {
                    None
                };
                match &callee {
                    Some((params, _, _)) if !params.is_empty() => {
                        self.print_arguments_linked(&args, params, arg)
                    }
                    _ => self.print_arguments(&args, arg),
                }
                let c_throws = callee.as_ref().map(|c| c.1).unwrap_or(false);
                let c_nullable = callee.as_ref().map(|c| c.2).unwrap_or(false);
                // A `throws` method returns `Result<_, String>` -> unwrap.
                if scope.is_none() && (self.id.has_throws_name(&name) || c_throws) {
                    self.printer.print(".unwrap()");
                }
                // A nullable-returning method used as a plain value is unwrapped.
                if scope.is_none()
                    && !self.expect_option
                    && (self.name_decl_nullable(&name, id) || c_nullable)
                {
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

    /// Is the variable `name` declared as an array (`T[]`)? Arrays map to a
    /// non-Copy `Vec`, which `type_simple_name`/`is_primitive` can't see (they
    /// look through to the element type).
    fn decl_is_array(&self, name: &str, at: NodeId) -> bool {
        let Some((_, decl)) = self.id.find_declaration_node_for(self.arena, name, at) else {
            return false;
        };
        let Some(parent) = self.arena.parent(decl) else { return false };
        let grand = self.arena.parent(parent);
        let typ = match self.arena.kind(parent) {
            Node::Parameter { typ, .. } => *typ,
            _ => match grand.map(|g| self.arena.kind(g)) {
                Some(Node::FieldDeclaration { typ, .. })
                | Some(Node::VariableDeclarationExpr { typ, .. }) => Some(*typ),
                _ => None,
            },
        };
        if let Some(t) = typ {
            if let Node::ReferenceType { array_count, .. } = self.arena.kind(t) {
                return *array_count > 0;
            }
        }
        false
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
        // Arity/arg-type overload dispatch for mapped runtime carriers (`BitSet`,
        // the `java.util.zip` `CRC32`/`Deflater`/`Inflater`, `Random`, and the
        // char-writer family). Java overloads these methods by arity (or, for
        // `CRC32.update`, by arg type); Rust needs distinct names. The data table
        // `stdlib::runtime_method_overload` holds the per-(type,name,arity)
        // verdict. For `BitSet` and the zip carriers — whose OTHER methods are real
        // inherent methods that must NOT reach the generic collection/String
        // rewrites in the `match` below (e.g. `BitSet.get(i)` -> `[i]` indexing,
        // `Writer.append` -> `push_str`) — a non-tabled method short-circuits to
        // default snake-case emission (`return false`). `Random` and the writers
        // fall through to the general match (their non-tabled methods are safe
        // there). Mirrors the runtime's `name`/`name_N` convention.
        if let Some(tn) = self.recv_type_name(recv) {
            use crate::stdlib::Overload;
            if let Some(ov) = crate::stdlib::runtime_method_overload(&tn, name, args.len()) {
                let emitted: Option<String> = match ov {
                    Overload::Bare => Some(camel_to_snake_case(name)),
                    Overload::Suffix => {
                        Some(format!("{}_{}", camel_to_snake_case(name), args.len()))
                    }
                    Overload::Rename(s) => Some(s.to_string()),
                    // `CRC32.update(byte[])` -> `update_1`; the `int` form keeps the
                    // base `update` (None -> short-circuit to default emit below).
                    Overload::ByArgVec => {
                        let t = self.infer_expr_rust_type(args[0]);
                        if t.starts_with("Vec") || t.ends_with("]") || t.contains("i8") {
                            Some(format!("{}_1", camel_to_snake_case(name)))
                        } else {
                            None
                        }
                    }
                };
                if let Some(m) = emitted {
                    self.visit(recv, arg);
                    self.printer.print(".");
                    self.printer.print(&m);
                    self.print_arguments(args, arg);
                    return true;
                }
            }
            // `BitSet` / zip carriers: their inherent methods must default-emit, not
            // fall through to the generic collection/String match below.
            if matches!(tn.as_str(), "BitSet" | "CRC32" | "Deflater" | "Inflater") {
                return false;
            }
        }
        match (name, args.len()) {
            // `collection.iterator()`/`listIterator()` -> a `JavaIter` over a
            // snapshot. `has_next()`/`has_previous()` then resolve by default
            // emission (snake-cased); `next()`/`previous()` are handled below.
            ("iterator", 0) | ("listIterator", 0) => {
                self.printer.print("crate::java_runtime::JavaIter::new((");
                self.visit(recv, arg);
                self.printer.print(").iter().cloned()/* TODO(translation): validate added clone */)");
                true
            }
            // `it.next()`/`it.previous()` on a `JavaIter` return `Option<T>`;
            // unwrap in a plain-value context, keep the `Option` in an
            // `expect_option` one (a `?:`/null-compared slot). Gated on the
            // receiver's declared type so a user cursor's `next()` is untouched.
            ("next", 0) | ("previous", 0)
                if matches!(
                    self.recv_type_name(recv).as_deref(),
                    Some("Iterator" | "ListIterator")
                ) =>
            {
                self.visit(recv, arg);
                self.printer.print(if name == "next" { ".next()" } else { ".previous()" });
                if !self.expect_option {
                    self.printer.print(".unwrap()");
                }
                true
            }
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
            // String `.equals(x)` -> compare as `&str` via the `&(..)[..]` slice
            // form (borrow-depth agnostic: `&String`/`String`/`&str` all coerce).
            // `String ==` doesn't hold for `&String == String` (E0277), and
            // `.as_str()` is the *nightly-unstable* `str::as_str` on an existing
            // `&str` (E0658) — the slice form is stable for both.
            ("equals", 1)
                if self.recv_is_string(recv) || self.is_string_literal(args[0]) =>
            {
                // The slice form needs BOTH operands sliceable; when one side is
                // not string-like (e.g. an `Unknown` stub field reached via a
                // now-String-typed receiver), `&(x)[..]` is E0608. Fall back to a
                // `Display`-based compare, valid for `String` and `Unknown` both.
                let recv_str = self.recv_is_string(recv);
                let arg_str = self.is_string_literal(args[0])
                    || matches!(self.ty(args[0]), crate::types::Type::Str);
                if recv_str && arg_str {
                    self.printer.print("(&(");
                    self.visit(recv, arg);
                    self.printer.print(")[..] == &(");
                    self.visit(args[0], arg);
                    self.printer.print(")[..])");
                } else {
                    self.printer.print("((");
                    self.visit(recv, arg);
                    self.printer.print(").to_string() == (");
                    self.visit(args[0], arg);
                    self.printer.print(").to_string())");
                }
                true
            }
            ("equalsIgnoreCase", 1) => {
                self.printer.print("((");
                self.visit(recv, arg);
                self.printer.print(").to_lowercase() == (");
                self.visit(args[0], arg);
                self.printer.print(").to_lowercase())");
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
                // R4: a concrete subtype added to an enum'd collection
                // (`Set<Root>`/`List<Root>`) wraps in the enum variant.
                let elem_enum = self.ty(recv).elem().and_then(|e| self.enum_path_of_type(e));
                self.visit(recv, arg);
                self.printer.print(if is_set { ".insert(" } else { ".push(" });
                match elem_enum {
                    Some(ep) => self.emit_enum_wrapped(args[0], &ep, arg),
                    None => self.visit(args[0], arg),
                }
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
                    self.printer.print(")).cloned()/* TODO(translation): validate added clone */.unwrap()");
                } else {
                    // List.get(i) -> indexed element (cloned to own it).
                    self.visit(recv, arg);
                    self.printer.print("[(");
                    self.visit(args[0], arg);
                    self.printer.print(") as usize].clone()/* TODO(translation): validate added clone */");
                }
                true
            }
            ("put", 2) => {
                let val_enum = self.ty(recv).map_value().and_then(|v| self.enum_path_of_type(v));
                self.visit(recv, arg);
                self.printer.print(".insert(");
                self.visit(args[0], arg);
                self.printer.print(", ");
                match val_enum {
                    Some(ep) => self.emit_enum_wrapped(args[1], &ep, arg),
                    None => self.visit(args[1], arg),
                }
                self.printer.print(")");
                true
            }
            ("contains", 1) => {
                let is_string = matches!(self.recv_type_name(recv).as_deref(), Some("String"));
                self.visit(recv, arg);
                if is_string {
                    // String.contains accepts a `&str` or a `char` pattern.
                    self.printer.print(".contains(");
                    self.emit_string_pattern(args[0], arg);
                    self.printer.print(")");
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
            // StringBuilder.append(x) -> push_str. The builder maps to `String`,
            // so any appendable (char/&str/String/number) goes through
            // `to_string()`. Chaining returns the unit value, but the source
            // never chains across statements, so that is unobservable.
            ("append", 1) => {
                self.visit(recv, arg);
                self.printer.print(".push_str(&(");
                self.visit(args[0], arg);
                // A `char[]` (`Vec<char>`) has no `Display`; Java appends its
                // chars, so collect into a `String` instead of `.to_string()`.
                if self.expr_is_char_vec(args[0]) {
                    self.printer.print(").iter().collect::<String>())");
                } else {
                    self.printer.print(").to_string())");
                }
                true
            }
            // ---- more String ops (arg is a String -> &str) ----
            // NOTE: `startsWith`/`endsWith`/`indexOf(1)`/`lastIndexOf(1)`/`split`
            // migrated to `instance_rule("String", …)` (category-gated). The 2-arg
            // `indexOf`/`lastIndexOf` keep their bespoke offset logic below.
            ("replace", 2) => {
                // `str::replace(from: Pattern, to: &str)`. `from` may be a char or
                // a String (char-aware pattern); `to` coerces to `&str`. (Java's
                // `replace(char,char)` would break a hardcoded `.as_str()`.)
                self.visit(recv, arg);
                self.printer.print(".replace(");
                self.emit_string_pattern(args[0], arg);
                self.printer.print(", &(");
                self.visit(args[1], arg);
                self.printer.print(").to_string())");
                true
            }
            // `String.indexOf(pat, fromIndex)` -> search the suffix from
            // `fromIndex`, re-offsetting the found position. (Byte≈char for ASCII.)
            ("indexOf", 2)
                if !matches!(self.recv_category(recv), Some("List" | "Set" | "Map" | "Option")) =>
            {
                self.printer.print("(");
                self.visit(recv, arg);
                self.printer.print("[(");
                self.visit(args[1], arg);
                self.printer.print(") as usize..].find(");
                self.emit_string_pattern(args[0], arg);
                self.printer.print(").map(|i| (i as i32) + (");
                self.visit(args[1], arg);
                self.printer.print(")).unwrap_or(-1))");
                true
            }
            // `String.lastIndexOf(pat, fromIndex)` -> best-effort: search the
            // prefix up to `fromIndex` backwards.
            ("lastIndexOf", 2)
                if !matches!(self.recv_category(recv), Some("List" | "Set" | "Map" | "Option")) =>
            {
                self.visit(recv, arg);
                self.printer.print(".rfind(");
                self.emit_string_pattern(args[0], arg);
                self.printer.print(").map(|i| i as i32).unwrap_or(-1)");
                let _ = args[1];
                true
            }
            // `String.toCharArray()` -> `Vec<char>`. String-specific in practice
            // (often on a `toString()` result, so the receiver type is unknown).
            ("toCharArray", 0) => {
                self.visit(recv, arg);
                self.printer.print(".chars().collect::<Vec<char>>()");
                true
            }
            // `compareTo`/`compareToIgnoreCase` return an int whose *sign* callers
            // use; `(a>b) - (a<b)` gives -1/0/1 portably (enum discriminants of
            // `Ordering` aren't guaranteed). `compareTo` gated to String only
            // (it's the generic `Comparable` name).
            ("compareToIgnoreCase", 1) => {
                self.printer.print("{ let __a = (");
                self.visit(recv, arg);
                self.printer.print(").to_lowercase(); let __b = (");
                self.visit(args[0], arg);
                self.printer
                    .print(").to_lowercase(); (__a > __b) as i32 - (__a < __b) as i32 }");
                true
            }
            ("compareTo", 1) if self.recv_is_string(recv) => {
                self.printer.print("{ let __a = (");
                self.visit(recv, arg);
                self.printer.print("); let __b = (");
                self.visit(args[0], arg);
                self.printer.print("); (__a > __b) as i32 - (__a < __b) as i32 }");
                true
            }
            // `replaceAll` on a String: best-effort literal replace (NOT regex).
            ("replaceAll", 2) if self.recv_is_string(recv) => {
                self.visit(recv, arg);
                self.printer.print(".replace(/* replaceAll: literal, not regex */ ");
                self.emit_string_pattern(args[0], arg);
                self.printer.print(", &(");
                self.visit(args[1], arg);
                self.printer.print(")[..])");
                true
            }
            ("containsKey", 1) => {
                self.visit(recv, arg);
                self.printer.print(".contains_key(&(");
                self.visit(args[0], arg);
                self.printer.print("))");
                true
            }
            // `Map.keySet()` -> an owned `Vec<K>` (matches how `.iterator()`/
            // foreach over the key set is handled).
            ("keySet", 0) => {
                self.visit(recv, arg);
                self.printer.print(".keys().cloned()/* TODO(translation): validate added clone */.collect::<Vec<_>>()");
                true
            }
            // ---- streams ----
            ("stream", 0) => {
                // Owned-value iterator so map/forEach closures see `T`, not `&T`.
                self.visit(recv, arg);
                self.printer.print(".iter().cloned()/* TODO(translation): validate added clone */");
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
            .print(&format!("(|{p}| {{ let {p} = {p}.clone()/* TODO(translation): validate added clone */; "));
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

    /// Emit a string search/pattern argument: a `char` is a valid `Pattern` on
    /// its own; anything else is coerced to `&str` via `&(..)[..]` (works for
    /// both `String` and an existing `&str`).
    fn emit_string_pattern(&mut self, s: NodeId, arg: Arg) {
        if self.expr_is_char(s) {
            self.printer.print("(");
            self.visit(s, arg);
            self.printer.print(")");
        } else {
            self.printer.print("&(");
            self.visit(s, arg);
            self.printer.print(")[..]");
        }
    }

    /// Is `e` a `char`-typed expression (literal, `char` name/field, or a
    /// `char[]` element read — the latter so `arr[i] - 'A'` promotes correctly)?
    fn expr_is_char(&self, e: NodeId) -> bool {
        self.ty(e).is_char()
    }

    /// A short-lived [`crate::types::TypeResolver`] over this unit (the dumper
    /// holds `&mut IdTracker`, so the resolver — which needs `&IdTracker` — is
    /// built per query rather than stored). The per-call memo still caches the
    /// recursion within a single `type_of`.
    fn ty(&self, e: NodeId) -> crate::types::Type {
        crate::types::TypeResolver::with_coll_elem(
            self.arena,
            &*self.id,
            self.link,
            self.current_class_fqn.clone(),
            Some(self.collection_elem_map()),
        )
        .type_of(e)
    }

    /// Tier-2: the per-file map (lazily built) of RAW collection declaration
    /// type-node → inferred element `Type`. Shared (cheap `Rc` clone) with every
    /// resolver this dumper creates, so a raw `List` field and its uses agree.
    fn collection_elem_map(&self) -> std::rc::Rc<std::collections::HashMap<NodeId, crate::types::Type>> {
        self.collection_elem.get_or_init(|| std::rc::Rc::new(self.infer_collection_elems())).clone()
    }

    /// Tier-2 Phase 1 inference: for each RAW arity-1 collection declaration in
    /// this file, gather element evidence from `.add(x)`-family calls on it and
    /// keep the element type ONLY when every site agrees on a single concrete
    /// type (conservative/monotone — conflict or unknown leaves it bare).
    fn infer_collection_elems(&self) -> std::collections::HashMap<NodeId, crate::types::Type> {
        use crate::types::Type;
        // A plain resolver (no `coll_elem`) — typing `.add` args must not recurse
        // through the map being built.
        let resolver = crate::types::TypeResolver::new(
            self.arena,
            &*self.id,
            self.link,
            self.current_class_fqn.clone(),
        );
        // Leaf gate: only an element type with NO project subtypes is safe.
        // Inferring a subtype element makes `.get()` yield the subtype where the
        // Java supertype is expected (Rust has no struct subtyping → cascade; the
        // +119 per-decl regression was dominated by exactly this). Primitives/
        // String are always leaves. A name that is some type's `parent` (has a
        // subtype) is excluded.
        let has_subtype: std::collections::HashSet<String> = self
            .link
            .iter()
            .filter_map(|(_, t)| t.parent.clone())
            .map(|p| p.rsplit('.').next().unwrap_or(&p).to_string())
            .collect();
        let is_leaf = |t: &Type| -> bool {
            match t {
                Type::Prim(_) | Type::Str => true,
                Type::Named { path, .. } => {
                    let simple = path.rsplit(['.', ':']).next().unwrap_or(path);
                    // `Object`/`Class` map to the `Unknown` placeholder, not a real
                    // Rust type — `Vec<Object>` would be `cannot find type Object`.
                    !matches!(simple, "Object" | "Class")
                        && !has_subtype.contains(simple)
                        && self.slot_enum_name(simple).is_none()
                }
                _ => false,
            }
        };
        let mut ev: std::collections::HashMap<NodeId, Vec<Type>> = std::collections::HashMap::new();
        for i in 0..self.arena.node_count() {
            let id = NodeId(i as u32);
            let Node::MethodCallExpr { scope: Some(recv), name, args, .. } = self.arena.kind(id)
            else {
                continue;
            };
            if args.is_empty()
                || !matches!(
                    name.as_str(),
                    "add" | "addElement" | "offer" | "offerLast" | "offerFirst" | "push"
                        | "addLast" | "addFirst"
                )
            {
                continue;
            }
            let Some(tn) = self.decl_type_node_of(*recv) else { continue };
            if !self.is_raw_arity1_collection(tn) {
                continue;
            }
            // `add(E)` and `add(int, E)` both take the element as the LAST arg.
            let elem = resolver.type_of(*args.last().unwrap());
            if is_leaf(&elem) {
                let v = ev.entry(tn).or_default();
                if !v.contains(&elem) {
                    v.push(elem);
                }
            }
        }
        ev.into_iter()
            .filter_map(|(k, mut v)| (v.len() == 1).then(|| (k, v.pop().unwrap())))
            .collect()
    }

    /// The declaration type-node of a `.add` receiver (field/local/param), or
    /// `None`. Mirrors the resolver's `decl_type_node`.
    fn decl_type_node_of(&self, recv: NodeId) -> Option<NodeId> {
        let name = match self.arena.kind(recv) {
            Node::NameExpr { name } => name.clone(),
            Node::FieldAccessExpr { field, .. } => field.clone(),
            _ => return None,
        };
        let (_, decl) = self.id.find_declaration_node_for(self.arena, &name, recv)?;
        let parent = self.arena.parent(decl)?;
        // LOCALS ONLY (no fields/params): a local's element evidence and all its
        // uses live in one method, so an inferred element can't cross-flow into a
        // bare field/param/return slot elsewhere (failure mode #1, which regressed
        // bioformats by +1 even with the leaf gate). Fields/params need the full
        // cross-slot union-find — deferred.
        match self.arena.kind(parent) {
            _ => match self.arena.parent(parent).map(|g| self.arena.kind(g)) {
                Some(Node::VariableDeclarationExpr { typ, .. }) => Some(*typ),
                _ => None,
            },
        }
    }

    /// Is `tn` a RAW (no-type-arg) arity-1 collection type node?
    fn is_raw_arity1_collection(&self, tn: NodeId) -> bool {
        match self.arena.kind(tn) {
            Node::ClassOrInterfaceType { name, type_args, .. } => {
                type_args.is_empty()
                    && matches!(
                        name.rsplit('.').next().unwrap_or(name),
                        "List" | "ArrayList" | "LinkedList" | "Vector" | "Stack" | "Collection"
                            | "Queue" | "Deque" | "ArrayDeque" | "Iterable" | "Set" | "HashSet"
                            | "LinkedHashSet" | "TreeSet" | "SortedSet" | "NavigableSet"
                    )
            }
            _ => false,
        }
    }

    /// Render an inferred element `Type` to its Rust type string (for emitting
    /// `Vec<T>`). Mirrors the codebase's type rendering so it agrees with the
    /// resolver's `Type`.
    fn elem_type_to_rust(&self, t: &crate::types::Type) -> String {
        use crate::types::{Prim, Type};
        match t {
            Type::Str => "String".to_string(),
            Type::Prim(p) => match p {
                Prim::I8 => "i8",
                Prim::I16 => "i16",
                Prim::I32 => "i32",
                Prim::I64 => "i64",
                Prim::Usize => "usize",
                Prim::F32 => "f32",
                Prim::F64 => "f64",
                Prim::Bool => "bool",
                Prim::Char => "char",
            }
            .to_string(),
            Type::Named { path, .. } => {
                let simple = path.rsplit(['.', ':']).next().unwrap_or(path);
                self.resolve_type_name(simple)
            }
            _ => crate::stubs::UNKNOWN.to_string(),
        }
    }

    /// Does `e` resolve to a `Vec<char>` (a Java `char[]`)? Such values have no
    /// `Display`, so string append/concat must `.iter().collect::<String>()`
    /// rather than `.to_string()`.
    fn expr_is_char_vec(&self, e: NodeId) -> bool {
        self.ty(e).is_char_vec()
    }

    fn emit_recv_method(&mut self, recv: NodeId, method: &str, arg: Arg) -> bool {
        self.visit(recv, arg);
        self.printer.print(".");
        self.printer.print(method);
        self.printer.print("()");
        true
    }

    fn recv_type_name(&self, recv: NodeId) -> Option<String> {
        match self.arena.kind(recv) {
            Node::NameExpr { name } => self.decl_java_type_name(name, recv),
            // A field-access receiver (`Constraints.aa2X`, `this.cache`) — resolve
            // its declared type too, so map/string rewrites fire on it (a `.get(k)`
            // on a `Map`-typed field must index by key, not by `as usize`). An
            // in-file field resolves via the AST; a cross-class static field
            // (`OtherClass.field`) via the symbol map.
            Node::FieldAccessExpr { scope, field, .. } => self
                .decl_java_type_name(field, recv)
                .or_else(|| self.static_field_java_type(*scope, field)),
            // A method-call receiver (`a.foo().bar()`): fall back to the rich
            // type resolver so the bespoke method handlers dispatch on the
            // chained result. Only the confidently-typed cases (String / a named
            // type — the latter's `path` is already a Java simple name) yield a
            // name; collections/Option flow through `recv_category` instead, and
            // an `Unknown` stays `None` (preserves prior best-effort behavior).
            Node::MethodCallExpr { .. } => match self.ty(recv) {
                crate::types::Type::Str => Some("String".to_string()),
                _ => None,
            },
            _ => None,
        }
    }

    /// The Java simple type name of a cross-class static field `Class.field`,
    /// from the symbol map (the field's `rust_type` slot stores it). Lets the
    /// receiver-category rewrites fire on a static map/collection field defined
    /// in another class.
    fn static_field_java_type(&self, scope: NodeId, field: &str) -> Option<String> {
        let Node::NameExpr { name: cls } = self.arena.kind(scope) else {
            return None;
        };
        let t = self.resolve_type_sym(cls)?;
        let f = t.static_fields.get(field).or_else(|| t.fields.get(field))?;
        // The stored type may carry generics/array (`Map<K, V>`, `String[]`);
        // `recv_type_name`'s consumers compare simple names, so strip to that.
        let simple = f.rust_type.split(['<', '[']).next().unwrap_or(&f.rust_type).trim();
        (!simple.is_empty()).then(|| simple.to_string())
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

    /// Normalize a receiver's declared Java type to a stdlib *category* used to
    /// key [`crate::stdlib::instance_rule`] (`String`/`Map`/`Set`/`List`/
    /// `Option`), or `None` when the type is unknown or a user type. The
    /// category gate keeps a user class's same-named method from being hijacked.
    fn recv_category(&self, recv: NodeId) -> Option<&'static str> {
        use crate::types::Category;
        Some(match self.ty(recv).category()? {
            Category::String => "String",
            Category::List => "List",
            Category::Map => "Map",
            Category::Set => "Set",
            Category::Option => "Option",
        })
    }

    /// Single predicate for the bespoke String-method gates (`equals`/`compareTo`/
    /// `replaceAll`), replacing scattered `recv_type_name(recv) == Some("String")`
    /// checks. Uses `recv_category` (`Type::Str`) so it also covers
    /// `StringBuilder`/`CharSequence` (all mapped to a Rust `String`, hence
    /// sliceable/comparable the same way) — the standardization the consistency
    /// audit called for.
    fn recv_is_string(&self, recv: NodeId) -> bool {
        self.recv_category(recv) == Some("String")
    }

    /// Apply the declarative JDK rewrite table ([`crate::stdlib`]). Runs after
    /// the bespoke handlers, so it only fires for table entries those don't
    /// cover. Static-class calls (`Character.isDigit`) match by class name;
    /// instance calls match by receiver category.
    fn try_emit_stdlib(&mut self, scope: Option<NodeId>, name: &str, args: &[NodeId], arg: Arg) -> bool {
        let Some(recv) = scope else { return false };
        if self.is_static_class_ref(recv) {
            if let Node::NameExpr { name: cls } = self.arena.kind(recv) {
                // A local/param/field shadowing the class name is a value.
                if self.id.find_declaration_node_for(self.arena, cls, recv).is_none() {
                    if let Some(rule) = crate::stdlib::static_rule(cls, name, args.len()) {
                        self.emit_template(rule.template, None, args, arg);
                        return true;
                    }
                }
            }
            return false;
        }
        if let Some(cat) = self.recv_category(recv) {
            if let Some(rule) = crate::stdlib::instance_rule(cat, name, args.len()) {
                self.emit_template(rule.template, Some(recv), args, arg);
                return true;
            }
        }
        false
    }

    /// Evaluate a [`crate::stdlib`] template, substituting `${…}` placeholders
    /// (`recv`, arg indices, and `:str`/`:usize`/`:ref`/`:move` coercions). A
    /// literal `{…}` (e.g. inside a `format!`) is emitted untouched.
    fn emit_template(&mut self, tmpl: &str, recv: Option<NodeId>, args: &[NodeId], arg: Arg) {
        let mut rest = tmpl;
        while let Some(pos) = rest.find("${") {
            let (lit, after) = rest.split_at(pos);
            self.printer.print(lit);
            let after = &after[2..];
            let end = after.find('}').expect("unterminated ${ in stdlib template");
            self.emit_template_token(&after[..end], recv, args, arg);
            rest = &after[end + 1..];
        }
        self.printer.print(rest);
    }

    fn emit_template_token(&mut self, tok: &str, recv: Option<NodeId>, args: &[NodeId], arg: Arg) {
        let (idx, kind) = match tok.split_once(':') {
            Some((a, b)) => (a, Some(b)),
            None => (tok, None),
        };
        let node = if idx == "recv" {
            recv.expect("`${recv}` in template with no receiver")
        } else {
            args[idx.parse::<usize>().expect("bad arg index in stdlib template")]
        };
        match kind {
            None => self.visit(node, arg),
            Some("str") => self.emit_string_pattern(node, arg),
            Some("usize") => {
                self.printer.print("(");
                self.visit(node, arg);
                self.printer.print(") as usize");
            }
            Some("ref") => {
                self.printer.print("&(");
                self.visit(node, arg);
                self.printer.print(")");
            }
            Some("move") => self.emit_moved_value(node, arg),
            Some(other) => panic!("unknown stdlib template directive `:{other}`"),
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

    /// Map static methods on the boxed primitive wrappers
    /// (`Integer.parseInt`, `Long.bitCount`, `Double.toString`, ...) to their
    /// Rust equivalents. The generic path would otherwise emit nonsense like
    /// `i32::parse_int(..)` (snake-cased Java name on the mapped primitive).
    fn try_emit_boxed_static(
        &mut self,
        scope: Option<NodeId>,
        name: &str,
        args: &[NodeId],
        arg: Arg,
    ) -> bool {
        let Some(s) = scope else { return false };
        let Node::NameExpr { name: cls } = self.arena.kind(s) else {
            return false;
        };
        // A local/param/field shadowing the class name is a value, not the box.
        if self.id.find_declaration_node_for(self.arena, cls, s).is_some() {
            return false;
        }
        let prim = match cls.as_str() {
            "Integer" => "i32",
            "Long" => "i64",
            "Short" => "i16",
            "Byte" => "i8",
            "Double" => "f64",
            "Float" => "f32",
            _ if cls == "Boolean" || cls == "Character" => "",
            _ => return false,
        };
        match (cls.as_str(), name, args.len()) {
            // X.parseX(s) -> (s).parse::<prim>().unwrap()
            ("Integer", "parseInt", 1)
            | ("Long", "parseLong", 1)
            | ("Short", "parseShort", 1)
            | ("Byte", "parseByte", 1)
            | ("Double", "parseDouble", 1)
            | ("Float", "parseFloat", 1) => {
                self.printer.print("(");
                self.visit(args[0], arg);
                self.printer.print(&format!(").parse::<{prim}>().unwrap()"));
                true
            }
            // X.parseX(s, radix) -> prim::from_str_radix((s), (radix) as u32).unwrap()
            ("Integer", "parseInt", 2) | ("Long", "parseLong", 2) => {
                self.printer.print(&format!("{prim}::from_str_radix(("));
                self.visit(args[0], arg);
                self.printer.print(").as_str(), (");
                self.visit(args[1], arg);
                self.printer.print(") as u32).unwrap()");
                true
            }
            ("Boolean", "parseBoolean", 1) => {
                self.printer.print("(");
                self.visit(args[0], arg);
                self.printer.print(").parse::<bool>().unwrap()");
                true
            }
            // Integer/Long.bitCount(x) -> ((x).count_ones() as i32)
            ("Integer" | "Long", "bitCount", 1) => {
                self.printer.print("((");
                self.visit(args[0], arg);
                self.printer.print(").count_ones() as i32)");
                true
            }
            // Double/Float.isNaN(x)/isInfinite(x) -> (x).is_nan()/is_infinite().
            ("Double" | "Float", "isNaN", 1) | ("Double" | "Float", "isInfinite", 1) => {
                self.printer.print("(");
                self.visit(args[0], arg);
                self.printer.print(if name == "isNaN" { ").is_nan()" } else { ").is_infinite()" });
                true
            }
            // X.valueOf(s)/X.toString(v) -> parse / to_string.
            ("Integer" | "Long" | "Short" | "Byte" | "Double" | "Float", "valueOf", 1) => {
                self.printer.print("(");
                self.visit(args[0], arg);
                self.printer.print(&format!(").parse::<{prim}>().unwrap()"));
                true
            }
            (_, "toString", 1) => {
                self.printer.print("(");
                self.visit(args[0], arg);
                self.printer.print(").to_string()");
                true
            }
            _ => false,
        }
    }

    fn try_emit_math(&mut self, scope: Option<NodeId>, name: &str, args: &[NodeId], arg: Arg) -> bool {
        // `Math.x(..)`, or a bare `x(..)` when `java.lang.Math` is statically
        // imported (`import static java.lang.Math.*`).
        let is_math = match scope {
            // Bare `Math.x(..)` or fully-qualified `java.lang.Math.x(..)` (a
            // `QualifiedNameExpr`/`FieldAccessExpr` whose last segment is `Math`).
            Some(s) => match self.arena.kind(s) {
                Node::NameExpr { name } | Node::QualifiedNameExpr { name, .. } => name == "Math",
                Node::FieldAccessExpr { field, .. } => field == "Math",
                _ => false,
            },
            None => self.math_statically_imported(),
        };
        if !is_math {
            return false;
        }
        // (receiver-method, arity)
        let m = match name {
            "abs" | "sqrt" | "cbrt" | "floor" | "ceil" | "round" | "signum" | "sin" | "cos"
            | "tan" | "asin" | "acos" | "atan" | "exp" | "sinh" | "cosh" | "tanh" => (name, 1),
            "log" => ("ln", 1),
            "log10" => ("log10", 1),
            "log1p" => ("ln_1p", 1),
            "expm1" => ("exp_m1", 1),
            "rint" => ("round", 1),
            "toRadians" => ("to_radians", 1),
            "toDegrees" => ("to_degrees", 1),
            "max" | "min" => (name, 2),
            "atan2" | "hypot" => (name, 2),
            "pow" => ("powf", 2),
            _ => return false,
        };
        if args.len() != m.1 {
            return false;
        }
        // The inherently-float functions need their args as `f64` (Rust has no
        // `i32::sqrt`/`.ln()`, and an int/literal arg would be ambiguous).
        // `abs/round/min/max/signum` keep the arg type — Java overloads those on
        // `int`, so casting would wrongly turn `Math.abs(int)` into `f64`.
        let float_args = !matches!(name, "abs" | "round" | "min" | "max" | "signum");
        self.printer.print("(");
        self.visit(args[0], arg);
        self.printer.print(if float_args { " as f64)." } else { ")." });
        self.printer.print(m.0);
        self.printer.print("(");
        if m.1 == 2 {
            self.visit(args[1], arg);
            if float_args {
                self.printer.print(" as f64");
            }
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
                    snakes.iter().map(|s| format!("{s}: {s}.clone()/* TODO(translation): validate added clone */")).collect();
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
        // StringBuilder/StringBuffer map to `String`. The capacity ctor
        // `new StringBuilder(int)` -> `String::new()`; the copy ctor
        // `new StringBuilder(CharSequence)` -> the argument's string value.
        if base == "String" {
            if let Node::ClassOrInterfaceType { name, .. } = self.arena.kind(typ) {
                let simple = name.rsplit('.').next().unwrap_or(name);
                if matches!(simple, "StringBuilder" | "StringBuffer") {
                    match args.first() {
                        Some(&a)
                            if !matches!(
                                self.infer_expr_rust_type(a).as_str(),
                                "i32" | "i64"
                            ) =>
                        {
                            self.printer.print("(");
                            self.visit(a, arg);
                            self.printer.print(").to_string()");
                        }
                        _ => self.printer.print("String::new()"),
                    }
                    return;
                }
            }
        }
        // Boxed-primitive constructors: `new Integer(x)` (mapped to `i32::new`,
        // which doesn't exist) -> parse a string arg, else unbox/cast the value.
        if let Node::ClassOrInterfaceType { name, .. } = self.arena.kind(typ) {
            let simple = name.rsplit('.').next().unwrap_or(name);
            let prim = match simple {
                "Integer" => Some("i32"),
                "Long" => Some("i64"),
                "Short" => Some("i16"),
                "Byte" => Some("i8"),
                "Double" => Some("f64"),
                "Float" => Some("f32"),
                "Boolean" => Some("bool"),
                "Character" => Some("char"),
                _ => None,
            };
            if let (Some(prim), Some(&a)) = (prim, args.first()) {
                if self.infer_expr_rust_type(a) == "String" && prim != "char" && prim != "bool" {
                    self.printer.print("(");
                    self.visit(a, arg);
                    self.printer.print(&format!(").parse::<{prim}>().unwrap()"));
                } else {
                    self.printer.print("((");
                    self.visit(a, arg);
                    self.printer.print(&format!(") as {prim})"));
                }
                return;
            }
        }
        // I/O constructors: route `new <IoType>(..)` to a family factory free fn
        // (src/runtime/io_read.rs / io_write.rs). The read/write families collapse
        // to shared carriers, so per-type ctors collide by arity
        // (`FileInputStream(path)` vs `BufferedInputStream(stream)`); the factory
        // fns disambiguate by name and each return the carrier (their generic
        // `impl Read/Write` params let nested concrete carriers compose). Runs
        // before the generic `::new_N` emission below.
        if let Node::ClassOrInterfaceType { name, .. } = self.arena.kind(typ) {
            let simple = name.rsplit('.').next().unwrap_or(name).to_string();
            if let Some(factory) = crate::stdlib::io_ctor_factory(&simple, args.len()) {
                self.printer.print(factory);
                self.print_arguments(&args, arg);
                return;
            }
            // `new PrintWriter/PrintStream(File|String)` opens a path; the
            // `(OutputStream|Writer)` overload wraps — disambiguate by arg type.
            if args.len() == 1 && matches!(simple.as_str(), "PrintWriter" | "PrintStream") {
                let t = self.infer_expr_rust_type(args[0]);
                let path_like = t == "String" || t.ends_with("JavaFile");
                let f = match (simple.as_str(), path_like) {
                    ("PrintWriter", true) => "crate::java_runtime::java_print_writer_path",
                    ("PrintWriter", false) => "crate::java_runtime::java_print_writer",
                    ("PrintStream", true) => "crate::java_runtime::java_print_stream_path",
                    _ => "crate::java_runtime::java_print_stream",
                };
                self.printer.print(f);
                self.print_arguments(&args, arg);
                return;
            }
        }
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
        let is_inner =
            inner_simple.as_deref().map(|n| self.current_inner_classes.contains(n)).unwrap_or(false);
        self.printer.print(&base);
        if !is_rust_collection(&base) && !is_inner {
            // Resolve the constructor overload by arity (project/linked types
            // record `new`/`new#arity`); plain `::new` for everything else.
            if let Some(m) = inner_simple
                .as_deref()
                .and_then(|s| self.resolve_ctor(s, args.len()))
            {
                self.printer.print("::");
                self.printer.print(&m.rust);
                self.print_arguments_linked(&args, &m.params, arg);
                return;
            }
            // A mapped runtime type (`crate::java_runtime::…`) is not in the
            // symbol map, so `resolve_ctor` can't disambiguate its overloaded
            // constructors. General convention the runtime fragments expose: the
            // arity-0 ctor is `new`, every higher arity is `new_<arity>` (so
            // `new Random()` -> `::new`, `new Random(seed)` -> `::new_1(seed)`,
            // `new File(p)` -> `::new_1(p)`, `new File(a,b)` -> `::new_2(a,b)`).
            if base.starts_with("crate::java_runtime::") && !args.is_empty() {
                self.printer.print(&format!("::new_{}", args.len()));
            } else {
                self.printer.print("::new");
            }
            self.print_arguments(&args, arg);
            return;
        }
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
                // A float operand needs a float step (`f64 += 1.0`); an integer
                // `1` won't add-assign to `f64`.
                let one = match self.ty(expr).numeric_rust() {
                    Some("f64") | Some("f32") => "1.0",
                    _ => "1",
                };
                let dec = matches!(op, PreDecrement | PosDecrement);
                let opstr = format!(" {} {one}", if dec { "-=" } else { "+=" });
                let post = matches!(op, PosIncrement | PosDecrement);
                if self.is_embedded_in_stmt(id) {
                    // Used as a value: lower to a block expression.
                    if post {
                        // x++ : yield old value
                        self.printer.print("{ let __v = ");
                        self.visit(expr, arg);
                        self.printer.print("; ");
                        self.visit(expr, arg);
                        self.printer.print(&opstr);
                        self.printer.print("; __v }");
                    } else {
                        // ++x : increment then yield
                        self.printer.print("{ ");
                        self.visit(expr, arg);
                        self.printer.print(&opstr);
                        self.printer.print("; ");
                        self.visit(expr, arg);
                        self.printer.print(" }");
                    }
                } else {
                    // Statement context: a plain compound assignment.
                    self.visit(expr, arg);
                    self.printer.print(&opstr);
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
        // Borrow inference also applies to constructor params (`&mut`/`mut`).
        let borrow = crate::borrow::analyze_method(self.arena, self.id, id);
        self.mut_borrow_params.extend(borrow.mut_params.iter().cloned());
        self.reassigned_params = borrow.reassigned.clone();
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
        // A generic class's constructor returns/builds the parameterized type
        // (`-> Hmm<O>`, `let __self: Hmm<O>`), not the bare name.
        let type_suffix = if self.impl_param_names.is_empty() {
            String::new()
        } else {
            format!("<{}>", self.impl_param_names.join(", "))
        };
        self.printer.print(") -> ");
        self.printer.print(&format!("{name}{type_suffix}"));
        // Build the value in `__self` (`this` maps to it), then return it.
        self.printer.print_ln_s(" {");
        self.printer.indent();
        self.printer
            .print_ln_s(&format!("let mut __self: {name}{type_suffix} = Default::default();"));
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
        self.reassigned_params.clear();
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
        // Borrow inference: reference params mutated through (`p.f = …`,
        // `p.add(..)`) need `&mut`; a body that mutates `self` needs `&mut self`.
        let borrow = crate::borrow::analyze_method(self.arena, self.id, id);
        self.mut_borrow_params.extend(borrow.mut_params.iter().cloned());
        self.reassigned_params = borrow.reassigned.clone();
        // `&mut self` includes propagation through self-calls (a method calling a
        // `&mut self` sibling on `self`), computed at class granularity.
        self.method_recv_mut = self
            .owner_type_decl(id)
            .map(|cls| crate::borrow::class_mut_methods(self.arena, self.id, cls).contains(&name))
            .unwrap_or(borrow.recv_mut);
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
        // A method named `clone` is renamed (see `rust_member_name`) to avoid
        // colliding with the derived `Clone::clone`.
        let snake = if name == "clone" {
            "clone_java".to_string()
        } else {
            self.to_snake_if_necessary(&name)
        };
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
            let needs_mut =
                self.method_recv_mut || body.map(|b| self.mutates_self(b)).unwrap_or(false);
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
                // A `void` method that `throws` returns `Result<(), String>`; its
                // body needs an `Ok(())` tail (and bare `return;` -> `return Ok(());`,
                // handled in `ReturnStmt`).
                if type_string == "void" && !throws.is_empty() {
                    self.pending_tail = Some("Ok(())".to_string());
                }
                self.visit(b, arg);
                self.pending_tail = None;
            }
        }
        self.id.set_current_method(None);
        self.in_static_method = false;
        self.mut_borrow_params.clear();
        self.reassigned_params.clear();
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
        // A parameter reassigned in the body needs a `mut` binding.
        let reassigned = matches!(self.arena.kind(vid),
            Node::VariableDeclaratorId { name } if self.reassigned_params.contains(name));
        if reassigned {
            self.printer.print("mut ");
        }
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
        // An element-nullable array param (`T[]` null-compared in the body) is a
        // `&Vec<Option<T>>` (rendered from a string so the element wraps).
        if !nullable && self.decl_elem_nullable(vid) {
            if let Some(t) = typ {
                let ty = Self::wrap_elem_option(self.accept_and_cut(t, arg).trim());
                let needs_mut = matches!(self.arena.kind(vid),
                    Node::VariableDeclaratorId { name } if self.mut_borrow_params.contains(name));
                self.printer.print(if needs_mut { "&mut " } else { "&" });
                self.printer.print(&ty);
                return;
            }
        }
        let is_primitive = typ
            .map(|t| matches!(self.arena.kind(t), Node::PrimitiveType { .. }))
            .unwrap_or(false);
        if nullable {
            // Option<T> owns its value; no borrow. An element-nullable array also
            // wraps the element: `Option<Vec<Option<T>>>`.
            self.printer.print("Option<");
            if let Some(t) = typ {
                if self.decl_elem_nullable(vid) {
                    let ty = Self::wrap_elem_option(self.accept_and_cut(t, arg).trim());
                    self.printer.print(&ty);
                } else {
                    self.visit(t, arg);
                }
            }
            self.printer.print(">");
        } else {
            // An interface-typed parameter becomes `&dyn Trait`: implementors
            // coerce at the call site (`&concrete` -> `&dyn Trait`), since we now
            // generate `impl Trait for Class`. Only a *direct* interface param
            // takes `&dyn` — an array of interface (`Trimmer[]`) must keep its
            // element boxed (`Vec<Box<dyn Trimmer>>`), so don't set the `&dyn`
            // flag (which would leak `dyn` into the unsized Vec element).
            let is_array = typ
                .map(|t| matches!(self.arena.kind(t), Node::ReferenceType { array_count, .. } if *array_count > 0))
                .unwrap_or(false);
            let is_trait = !is_array
                && typ
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
        // Take any pending tail (e.g. `Ok(())`) so nested blocks don't inherit it.
        let tail = self.pending_tail.take();
        self.printer.print_ln_s("{");
        self.printer.indent();
        for &s in &stmts {
            self.visit(s, arg);
            self.printer.print_ln();
        }
        if let Some(t) = tail {
            self.printer.print_ln_s(&t);
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
        let mut had_default = false; // any `default` label seen across the switch
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
            had_default |= pending_default;
            pending.clear();
            pending_default = false;
        }
        // Trailing labels with no body (e.g. an empty final case).
        if pending_default || !pending.is_empty() {
            self.emit_switch_patterns(&pending, pending_default, arg);
            self.printer.print_ln_s(" => {}");
            had_default |= pending_default;
        }
        // Rust `match` must be exhaustive: a Java `switch` with no `default` over
        // a non-enumerable selector (char/int/String) needs a catch-all arm.
        if !had_default {
            self.printer.print_ln_s("_ => {}");
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
        // Variants are unit-only (Java enum bodies are dropped below), so these
        // all derive trivially — and they're needed pervasively: `.clone()`,
        // `==`/`!=` against a variant, and use as map keys. `Default` (with the
        // first variant marked `#[default]`) lets an enum-typed struct field
        // participate in the struct's derived `Default` — but only when the enum
        // has at least one variant (a variant-less enum can't derive `Default`).
        if entries.is_empty() {
            self.printer.print_ln_s("#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]");
        } else {
            self.printer.print_ln_s("#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Default)]");
        }
        self.print_modifiers(modifiers_v);
        self.printer.print("enum ");
        self.printer.print(&name);
        let _ = &implements; // `implements` has no Rust enum equivalent; dropped.
        self.printer.print_ln_s(" {");
        self.printer.indent();
        // Variants only. Java enum fields/constructors/methods have no direct
        // Rust enum equivalent and are dropped.
        let _ = &members;
        for (i, &e) in entries.iter().enumerate() {
            if i == 0 {
                self.printer.print_ln_s("#[default]");
            }
            self.visit(e, arg);
            self.printer.print_ln_s(",");
        }
        self.printer.unindent();
        self.printer.print_ln_s("}");
        // Java enums are routinely used as strings (`enum.toString()`, string
        // concatenation, `name()`). A bare Rust enum has no `Display`/`to_string`
        // — which the capable `Unknown` stub *did* provide, so resolving a value
        // to its real project enum (e.g. nested-type resolution) otherwise loses
        // that capability. Forward `Display` to the derived `Debug` (variants are
        // unit-only, so `{:?}` prints the bare variant name, matching Java's
        // default `toString`).
        self.printer.print_ln_s(&format!(
            "impl std::fmt::Display for {name} {{ fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {{ write!(f, \"{{:?}}\", self) }} }}"
        ));
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
        // `if ((line = in.readLine()) != null)` -> `if let Some(mut line) = …`.
        if let Some((nm, value)) = self.as_readline_assign(condition) {
            self.printer.print(&format!("if let Some(mut {nm}) = "));
            self.visit(value, arg);
        } else {
            self.printer.print("if ");
            self.visit(condition, arg);
        }
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

    /// Is `n` (a field's `VariableDeclaratorId`) a static field emitted as a
    /// `LazyLock<T>` associated const? Mirrors `emit_const_field`: a static field
    /// with a *non-const* initializer (not a numeric/bool/char literal, nor a
    /// String-literal `String`). Such a const reads as `LazyLock<T>`, so a value
    /// position must deref it (`*Self::X` / `(*Self::X).clone()`).
    fn is_lazylock_field(&self, n: NodeId) -> bool {
        if !self.is_static_field_declaration(n) {
            return false;
        }
        let Some(var) = self.arena.parent(n) else { return false };
        let Node::VariableDeclarator { init: Some(i), .. } = self.arena.kind(var) else {
            return false;
        };
        let i = *i;
        let Some(field) = self.arena.parent(var) else { return false };
        let Node::FieldDeclaration { typ, .. } = self.arena.kind(field) else {
            return false;
        };
        let is_string = self.type_simple_name(*typ).as_deref() == Some("String");
        !(is_string && self.is_string_literal(i)) && !self.is_const_literal(i)
    }

    fn is_non_static_method_declaration(&self, n: NodeId) -> bool {
        if let Node::MethodDeclaration { modifiers, .. } = self.arena.kind(n) {
            !modifiers::is_static(*modifiers)
        } else {
            false
        }
    }

    fn stop_history_search(&self, n: NodeId) -> bool {
        use crate::ast::BinaryOp::{BinAnd, BinOr, LShift, RSignedShift, RUnsignedShift, Xor};
        // A bitwise/shift expression yields an *integer* regardless of any float
        // context above it (Java bitwise/shift operands are integral). Stop the
        // upward float-history walk here so an integer literal operand isn't
        // float-coerced — e.g. `f * ((rgb >> 24) & 0xff)` must keep `24`/`0xff`
        // as ints (a `.0` on a hex literal yields the invalid Rust `0xff.0`), and
        // the float multiply casts the whole integer subexpression instead.
        matches!(
            self.arena.kind(n),
            Node::VariableDeclarator { .. }
                | Node::MethodCallExpr { .. }
                | Node::ArrayAccessExpr { .. }
                | Node::BinaryExpr {
                    op: BinAnd | BinOr | Xor | LShift | RSignedShift | RUnsignedShift,
                    ..
                }
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
        // Java's external iterators map to a runtime shim with Java-shaped
        // `has_next()`/`next()`/`has_previous()`/`previous()` methods.
        "Iterator" | "ListIterator" => "crate::java_runtime::JavaIter",
        // `java.io.File` -> a real `PathBuf`-backed handle (runtime support),
        // so `exists()`/`length()`/`getName()`/… do real filesystem work
        // instead of an opaque stub.
        "File" => "crate::java_runtime::JavaFile",
        // `java.util.BitSet` -> a real word-array bit vector (runtime support),
        // so `get`/`set`/`cardinality`/`nextSetBit`/… do real bit work.
        "BitSet" => "crate::java_runtime::JavaBitSet",
        // `java.util.Random` -> a JDK-bit-compatible 48-bit LCG.
        "Random" => "crate::java_runtime::JavaRandom",
        // `java.util.StringTokenizer` -> eager tokenizer over a delimiter set.
        "StringTokenizer" => "crate::java_runtime::JavaStringTokenizer",
        // I/O read stack (src/runtime/io_read.rs): each FAMILY collapses to ONE
        // carrier so a Java abstract supertype and its concrete subtypes share a
        // Rust type; `new X(..)` ctors route to factory free fns in
        // visit_object_creation (arity can't disambiguate `FileInputStream(path)`
        // from `BufferedInputStream(stream)`).
        "InputStream" | "FileInputStream" | "BufferedInputStream" | "ByteArrayInputStream"
        | "DataInputStream"
        // java.util.zip decompressing read streams (src/runtime/zip.rs, flate2):
        // a gzip/inflate stream IS-A InputStream, so it collapses to the same
        // carrier and its `new X(in)` routes to a factory yielding a JavaInputStream
        // (clears the vcf residual where GZIPInputStream was a non-Read named stub).
        | "GZIPInputStream" | "InflaterInputStream"
            => "crate::java_runtime::JavaInputStream",
        "Reader" | "BufferedReader" | "FileReader" | "InputStreamReader" | "StringReader"
        | "LineNumberReader" => "crate::java_runtime::JavaReader",
        // I/O write stack (src/runtime/io_write.rs). ByteArrayOutputStream/StringWriter
        // are own-typed (need to_byte_array/to_string); the rest collapse to carriers.
        "OutputStream" | "FileOutputStream" | "BufferedOutputStream" | "DataOutputStream"
        | "FilterOutputStream"
        // java.util.zip compressing write streams (src/runtime/zip.rs, flate2).
        | "GZIPOutputStream" | "DeflaterOutputStream"
            => "crate::java_runtime::JavaOutputStream",
        "ByteArrayOutputStream" => "crate::java_runtime::JavaByteArrayOutputStream",
        "Writer" | "OutputStreamWriter" | "BufferedWriter" | "FileWriter" | "PrintWriter"
        | "PrintStream" => "crate::java_runtime::JavaWriter",
        "StringWriter" => "crate::java_runtime::JavaStringWriter",
        // `java.util.concurrent.atomic.*` (src/runtime/atomic.rs) — STILL PARKED.
        // Because nothing maps these types, `atomic.rs` is NO LONGER shipped in the
        // `JAVA_RUNTIME` concat (crate_layout.rs) — it's kept only in the
        // `java_runtime_compiles` compile-check so it stays sound for resurrection.
        // Re-tried 2026-06 with a no-op `unwrap(&self)->Self` overlay: mapping still
        // regresses (trim +14, jsoup +1) with a DIFFERENT root cause than the
        // documented `.clone().unwrap()` one: `expected bool, found JavaAtomicBoolean`
        // / `expected i64, found JavaAtomicLong` — the atomic carrier flows into a
        // primitive position without `.get()` (the field's *type* is inferred as the
        // primitive from `.get()` usage, but its *value* is the carrier). Needs
        // value-vs-primitive reconciliation, not just an unwrap overlay. Arms
        // intentionally omitted.
        // `java.text` number formatters -> real `format!`-based shims (a `JavaNum`
        // arg trait accepts `&f64`/i64/…; `NumberFormat.getInstance(Locale)` is
        // routed to the 0-arg `get_instance()` by a static_rule arm).
        // `java.util.zip` own-typed runtime structs (src/runtime/zip.rs). CRC32 is
        // pure-std; Inflater/Deflater are flate2-backed. Constants
        // (`Deflater.DEFAULT_COMPRESSION`/`.DEFLATED`/`.NO_FLUSH`/…) emit as
        // associated `const`s on the mapped type via the static-field path.
        "CRC32" => "crate::java_runtime::JavaCRC32",
        "Inflater" => "crate::java_runtime::JavaInflater",
        "Deflater" => "crate::java_runtime::JavaDeflater",
        "DecimalFormat" => "crate::java_runtime::JavaDecimalFormat",
        "NumberFormat" => "crate::java_runtime::JavaNumberFormat",
        "DecimalFormatSymbols" => "crate::java_runtime::JavaDecimalFormatSymbols",
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

/// A numeric Rust primitive the widening/promotion logic operates on.
fn is_numeric_rust(t: &str) -> bool {
    matches!(t, "f64" | "f32" | "i64" | "i32" | "i16" | "i8")
}

/// A scalar (Copy) Java type — a primitive or its boxed wrapper. Such values are
/// passed/returned by value; everything else is a class/array (non-Copy).
fn is_scalar_java_type(name: &str) -> bool {
    let simple = name.rsplit('.').next().unwrap_or(name);
    matches!(
        simple,
        "int" | "long" | "short" | "byte" | "char" | "boolean" | "float" | "double"
            | "Integer" | "Long" | "Short" | "Byte" | "Character" | "Boolean" | "Float" | "Double"
    )
}

/// Java methods that are universally read-only (`&self`): calling one only needs
/// to *borrow* the receiver. Used to decide whether a nullable read in receiver
/// position can be unwrapped through `.as_ref()` instead of cloning (§6 use-site
/// borrow). Deliberately conservative — only names that never mutate or consume
/// the receiver across String/Number/Collection/Object and conventional getters;
/// a mutating method (`add`/`put`/`set`/`remove`/`close`/`append`/…) is absent so
/// its clone is kept. (The call autorefs `&T`, so the *return* ownership of these
/// methods is irrelevant to receiver-borrow safety.)
fn is_readonly_java_method(name: &str) -> bool {
    matches!(
        name,
        // Object / general
        "equals" | "equalsIgnoreCase" | "hashCode" | "toString" | "compareTo"
            | "compareToIgnoreCase"
        // String / CharSequence reads
            | "length" | "isEmpty" | "isBlank" | "charAt" | "codePointAt"
            | "indexOf" | "lastIndexOf" | "substring" | "contains" | "startsWith"
            | "endsWith" | "trim" | "strip" | "toLowerCase" | "toUpperCase"
            | "matches" | "split" | "getBytes" | "toCharArray" | "chars"
            | "concat" | "replace" | "replaceAll" | "intern"
        // Collection / Map reads
            | "size" | "get" | "containsKey" | "containsValue" | "getOrDefault"
            | "keySet" | "values" | "entrySet" | "iterator" | "listIterator"
            | "toArray" | "stream" | "subList"
        // Number reads
            | "intValue" | "longValue" | "doubleValue" | "floatValue"
            | "shortValue" | "byteValue" | "isNaN" | "isInfinite"
        // Logging (SLF4J / java.util.logging) — emit-only, never mutate the
        // logger; distinctive names unlikely to collide with a mutating method.
            | "trace" | "debug" | "info" | "warn" | "error" | "fatal" | "log"
            | "fine" | "finer" | "finest" | "config" | "severe" | "warning"
            | "isDebugEnabled" | "isTraceEnabled" | "isInfoEnabled"
            | "isWarnEnabled" | "isErrorEnabled"
    )
}

/// Java numeric-promotion rank: `double` > `float` > `long` > `int` > `short` >
/// `byte`. Mixed arithmetic promotes both operands to the higher rank.
fn num_rank(t: &str) -> u8 {
    match t {
        "f64" => 5,
        "f32" => 4,
        "i64" => 3,
        "i32" => 2,
        "i16" => 1,
        _ => 0,
    }
}

/// Map a boxed-primitive constant (`Integer.MAX_VALUE`) to its Rust path
/// (`i32::MAX`). Returns `None` for non-constant fields or non-boxed classes.
fn boxed_constant(cls: &str, field: &str) -> Option<&'static str> {
    Some(match (cls, field) {
        ("Integer", "MAX_VALUE") => "i32::MAX",
        ("Integer", "MIN_VALUE") => "i32::MIN",
        ("Long", "MAX_VALUE") => "i64::MAX",
        ("Long", "MIN_VALUE") => "i64::MIN",
        ("Short", "MAX_VALUE") => "i16::MAX",
        ("Short", "MIN_VALUE") => "i16::MIN",
        ("Byte", "MAX_VALUE") => "i8::MAX",
        ("Byte", "MIN_VALUE") => "i8::MIN",
        ("Double", "MAX_VALUE") => "f64::MAX",
        ("Double", "MIN_VALUE") => "f64::MIN_POSITIVE",
        ("Float", "MAX_VALUE") => "f32::MAX",
        ("Float", "MIN_VALUE") => "f32::MIN_POSITIVE",
        ("Double", "POSITIVE_INFINITY") => "f64::INFINITY",
        ("Double", "NEGATIVE_INFINITY") => "f64::NEG_INFINITY",
        ("Double", "NaN") => "f64::NAN",
        ("Float", "POSITIVE_INFINITY") => "f32::INFINITY",
        ("Float", "NEGATIVE_INFINITY") => "f32::NEG_INFINITY",
        ("Float", "NaN") => "f32::NAN",
        ("Math", "PI") => "std::f64::consts::PI",
        ("Math", "E") => "std::f64::consts::E",
        ("Boolean", "TRUE") => "true",
        ("Boolean", "FALSE") => "false",
        _ => return None,
    })
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
