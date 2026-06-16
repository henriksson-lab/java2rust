//! Borrow inference.
//!
//! Decides, per method, whether the receiver (`self`) and each reference
//! parameter must be borrowed mutably (`&mut`) rather than immutably (`&`).
//! Follows a "start with `&`, upgrade to `&mut` only on evidence" model:
//!   - scalars are passed by value (handled by the type-directed default in
//!     `param_sym`/`visit_parameter`); only *reference* params can become `&mut`.
//!   - a binding is `&mut` when the body mutates *through* it: a field/element
//!     assignment (`b.f = …`, `b[i] = …`), an increment/decrement of such, or a
//!     mutating collection/`StringBuilder` call on it (`b.add(..)`, …).
//!
//! Stage 1 is intra-procedural (direct + builtin evidence only). Inter-procedural
//! propagation across the call graph (`&mut` flowing from callee params/receivers
//! to caller arguments) is layered on top of these per-method facts.

use std::collections::{HashMap, HashSet};

use crate::ast::{Node, NodeId, UnaryOp};
use crate::id_tracker::IdTracker;

/// Per-method borrow facts.
pub struct MethodBorrow {
    /// The method mutates `self` (a field assignment or a mutating call on a
    /// field) and so needs `&mut self`.
    pub recv_mut: bool,
    /// Names of reference parameters mutated through (so they need `&mut`).
    pub mut_params: HashSet<String>,
    /// Names of parameters reassigned in the body (`p = …`), which need a `mut`
    /// binding (independent of whether the type is a borrow).
    pub reassigned: HashSet<String>,
}

/// What a mutation site's root expression refers to.
enum Binding {
    SelfRecv,
    Param(String),
    Other,
}

/// Compute the intra-procedural borrow facts for a method or constructor.
pub fn analyze_method(arena: &crate::ast::Arena, id: &IdTracker, decl: NodeId) -> MethodBorrow {
    let (params, body) = match arena.kind(decl) {
        Node::MethodDeclaration { parameters, body, .. } => (parameters.clone(), *body),
        Node::ConstructorDeclaration { parameters, block, .. } => {
            (parameters.clone(), Some(*block))
        }
        _ => {
            return MethodBorrow {
                recv_mut: false,
                mut_params: HashSet::new(),
                reassigned: HashSet::new(),
            }
        }
    };
    let mut a = Analyzer {
        arena,
        id,
        param_decls: HashMap::new(),
        field_names: HashSet::new(),
        recv_mut: false,
        mut_params: HashSet::new(),
        reassigned: HashSet::new(),
    };
    for &p in &params {
        if let Node::Parameter { id: vid, .. } = arena.kind(p) {
            if let Node::VariableDeclaratorId { name } = arena.kind(*vid) {
                a.param_decls.insert(*vid, name.clone());
            }
        }
    }
    a.collect_field_names(decl);
    if let Some(b) = body {
        a.scan(b);
    }
    MethodBorrow { recv_mut: a.recv_mut, mut_params: a.mut_params, reassigned: a.reassigned }
}

/// The Java names of a class's instance methods that need `&mut self`, including
/// propagation through self-calls: a method that calls a `&mut self` sibling on
/// `self` also needs `&mut self`. Iterated to a fixpoint (handles mutual
/// recursion). This is the intra-class slice of "propagate `&mut` up the call
/// graph" — cross-type propagation flows through the symbol map's `refmut`
/// receiver instead.
pub fn class_mut_methods(
    arena: &crate::ast::Arena,
    id: &IdTracker,
    class_decl: NodeId,
) -> HashSet<String> {
    let members = match arena.kind(class_decl) {
        Node::ClassOrInterfaceDeclaration { members, .. } => members.clone(),
        _ => return HashSet::new(),
    };
    let mut methods: Vec<(String, NodeId)> = Vec::new();
    for &m in &members {
        if let Node::MethodDeclaration { name, modifiers, .. } = arena.kind(m) {
            if !crate::modifiers::is_static(*modifiers) {
                methods.push((name.clone(), m));
            }
        }
    }
    let names: HashSet<String> = methods.iter().map(|(n, _)| n.clone()).collect();
    let mut mut_set: HashSet<String> = HashSet::new();
    let mut self_calls: Vec<(String, HashSet<String>)> = Vec::new();
    for (name, decl) in &methods {
        if analyze_method(arena, id, *decl).recv_mut {
            mut_set.insert(name.clone());
        }
        self_calls.push((name.clone(), self_called_names(arena, *decl, &names)));
    }
    loop {
        let mut changed = false;
        for (name, calls) in &self_calls {
            if !mut_set.contains(name) && calls.iter().any(|c| mut_set.contains(c)) {
                mut_set.insert(name.clone());
                changed = true;
            }
        }
        if !changed {
            break;
        }
    }
    mut_set
}

/// Sibling-method names called on `self` (a bare call resolving to a sibling, or
/// an explicit `this.m()`) within a method body.
fn self_called_names(
    arena: &crate::ast::Arena,
    decl: NodeId,
    siblings: &HashSet<String>,
) -> HashSet<String> {
    let body = match arena.kind(decl) {
        Node::MethodDeclaration { body, .. } => *body,
        _ => None,
    };
    let mut out = HashSet::new();
    let Some(b) = body else { return out };
    let mut stack = vec![b];
    while let Some(n) = stack.pop() {
        if let Node::MethodCallExpr { scope, name, .. } = arena.kind(n) {
            let is_self = match scope {
                None => true,
                Some(s) => matches!(arena.kind(*s), Node::ThisExpr { .. }),
            };
            if is_self && siblings.contains(name) {
                out.insert(name.clone());
            }
        }
        for c in arena.children(n) {
            stack.push(c);
        }
    }
    out
}

struct Analyzer<'a> {
    arena: &'a crate::ast::Arena,
    id: &'a IdTracker,
    /// Parameter `VariableDeclaratorId` -> name.
    param_decls: HashMap<NodeId, String>,
    /// Instance field names of the enclosing class.
    field_names: HashSet<String>,
    recv_mut: bool,
    mut_params: HashSet<String>,
    reassigned: HashSet<String>,
}

impl<'a> Analyzer<'a> {
    /// Gather the instance-field names of the class immediately enclosing `decl`.
    fn collect_field_names(&mut self, decl: NodeId) {
        let mut n = decl;
        while let Some(p) = self.arena.parent(n) {
            if let Node::ClassOrInterfaceDeclaration { members, .. } = self.arena.kind(p) {
                for &m in members {
                    if let Node::FieldDeclaration { variables, .. } = self.arena.kind(m) {
                        for &v in variables {
                            if let Node::VariableDeclarator { id: vid, .. } = self.arena.kind(v) {
                                if let Node::VariableDeclaratorId { name } = self.arena.kind(*vid) {
                                    self.field_names.insert(name.clone());
                                }
                            }
                        }
                    }
                }
                return;
            }
            n = p;
        }
    }

    fn scan(&mut self, body: NodeId) {
        let mut stack = vec![body];
        while let Some(n) = stack.pop() {
            match self.arena.kind(n) {
                Node::AssignExpr { target, .. } => self.mark_assign_target(*target),
                Node::UnaryExpr { expr, op }
                    if matches!(
                        op,
                        UnaryOp::PreIncrement
                            | UnaryOp::PreDecrement
                            | UnaryOp::PosIncrement
                            | UnaryOp::PosDecrement
                    ) =>
                {
                    self.mark_assign_target(*expr)
                }
                Node::MethodCallExpr { scope: Some(s), name, .. }
                    if crate::id_tracker::is_mutating_method(name) =>
                {
                    self.mark_binding(*s)
                }
                _ => {}
            }
            for c in self.arena.children(n) {
                stack.push(c);
            }
        }
    }

    /// An assignment/inc-dec target. A field/element target mutates *through* its
    /// root binding; a bare-name target is a self-field mutation only (a param or
    /// local rebinding needs `let mut`, not `&mut`, so it is ignored here).
    fn mark_assign_target(&mut self, target: NodeId) {
        match self.arena.kind(target) {
            Node::FieldAccessExpr { .. } | Node::ArrayAccessExpr { .. } => self.mark_binding(target),
            Node::NameExpr { .. } => match self.binding_of(target) {
                // Bare `field = …` mutates self; bare `p = …` rebinds a param
                // (needs a `mut` binding, not `&mut`).
                Binding::SelfRecv => self.recv_mut = true,
                Binding::Param(name) => {
                    self.reassigned.insert(name);
                }
                Binding::Other => {}
            },
            _ => {}
        }
    }

    fn mark_binding(&mut self, expr: NodeId) {
        match self.binding_of(expr) {
            Binding::SelfRecv => self.recv_mut = true,
            Binding::Param(name) => {
                self.mut_params.insert(name);
            }
            Binding::Other => {}
        }
    }

    /// The root binding a (possibly nested) field/element/name expression refers
    /// to: `self`, a parameter, or something else (a local — out of scope here).
    fn binding_of(&self, expr: NodeId) -> Binding {
        match self.arena.kind(expr) {
            Node::ThisExpr { .. } => Binding::SelfRecv,
            Node::EnclosedExpr { inner: Some(i) } => self.binding_of(*i),
            Node::ArrayAccessExpr { name, .. } => self.binding_of(*name),
            Node::FieldAccessExpr { scope, .. } => {
                if matches!(self.arena.kind(*scope), Node::ThisExpr { .. }) {
                    Binding::SelfRecv
                } else {
                    self.binding_of(*scope)
                }
            }
            Node::NameExpr { name } => {
                if let Some((_, decl)) = self.id.find_declaration_node_for(self.arena, name, expr) {
                    if let Some(pname) = self.param_decls.get(&decl) {
                        return Binding::Param(pname.clone());
                    }
                }
                // A bare field name (not a param/local) mutates `self`.
                if self.field_names.contains(name) {
                    Binding::SelfRecv
                } else {
                    Binding::Other
                }
            }
            _ => Binding::Other,
        }
    }
}
