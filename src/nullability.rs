//! Nullability inference.
//!
//! Determines which declarations (locals, parameters, fields, and method
//! returns) can hold `null`, so the code generator can emit `Option<T>` only
//! where needed (rather than wrapping everything). Runs after `IdTracker`
//! (which provides scope resolution) and before code generation.
//!
//! The result is a set of declaration `NodeId`s:
//!   - a variable/parameter/field is keyed by its `VariableDeclaratorId`
//!   - a method return is keyed by its `MethodDeclaration`
//! These are exactly the nodes `IdTracker::find_declaration_node_for` returns,
//! so the dumper can test membership directly.

use std::collections::HashSet;

use crate::ast::{BinaryOp, Node, NodeId};
use crate::id_tracker::IdTracker;

pub fn analyze(arena: &crate::ast::Arena, root: NodeId, id: &IdTracker) -> HashSet<NodeId> {
    let mut a = Analyzer {
        arena,
        id,
        nullable: HashSet::new(),
    };
    a.seed(root);
    a.propagate();
    a.nullable
}

struct Analyzer<'a> {
    arena: &'a crate::ast::Arena,
    id: &'a IdTracker,
    nullable: HashSet<NodeId>,
}

impl<'a> Analyzer<'a> {
    fn node_count(&self) -> u32 {
        self.arena.nodes.len() as u32
    }

    /// Resolve a name used at `at` to its declaration node.
    fn decl_of_name(&self, name: &str, at: NodeId) -> Option<NodeId> {
        self.id
            .find_declaration_node_for(self.arena, name, at)
            .map(|(_, n)| n)
    }

    fn mark(&mut self, decl: NodeId) -> bool {
        self.nullable.insert(decl)
    }

    /// The enclosing method declaration of a node, if any.
    fn enclosing_method(&self, mut n: NodeId) -> Option<NodeId> {
        while let Some(p) = self.arena.parent(n) {
            if matches!(self.arena.kind(p), Node::MethodDeclaration { .. }) {
                return Some(p);
            }
            n = p;
        }
        None
    }

    // ---- seeding ----

    fn seed(&mut self, root: NodeId) {
        for i in 0..self.node_count() {
            let n = NodeId(i);
            match self.arena.kind(n) {
                Node::NullLiteralExpr => self.seed_null_sink(n),
                Node::BinaryExpr { left, op, right }
                    if matches!(op, BinaryOp::Equals | BinaryOp::NotEquals) =>
                {
                    // x == null / x != null  ->  x is nullable
                    let (l, r) = (*left, *right);
                    if matches!(self.arena.kind(r), Node::NullLiteralExpr) {
                        self.mark_target(l);
                    } else if matches!(self.arena.kind(l), Node::NullLiteralExpr) {
                        self.mark_target(r);
                    }
                }
                _ => {}
            }
        }
        let _ = root;
    }

    /// Mark the declaration that this expression refers to (if it is a name).
    fn mark_target(&mut self, expr: NodeId) {
        if let Node::NameExpr { name } = self.arena.kind(expr) {
            let name = name.clone();
            if let Some(d) = self.decl_of_name(&name, expr) {
                self.mark(d);
            }
        }
    }

    /// A `null` literal flows into some slot — mark that slot's declaration.
    fn seed_null_sink(&mut self, null_node: NodeId) {
        let parent = match self.arena.parent(null_node) {
            Some(p) => p,
            None => return,
        };
        match self.arena.kind(parent).clone() {
            // T x = null;
            Node::VariableDeclarator { id, init, .. } if init == Some(null_node) => {
                self.mark(id);
            }
            // x = null;
            Node::AssignExpr { target, value, .. } if value == null_node => {
                self.mark_target(target);
            }
            // return null;
            Node::ReturnStmt { .. } => {
                if let Some(m) = self.enclosing_method(null_node) {
                    self.mark(m);
                }
            }
            // f(..., null, ...)  ->  the corresponding parameter
            Node::MethodCallExpr { name, args, .. } => {
                if let Some(idx) = args.iter().position(|&a| a == null_node) {
                    self.mark_param(&name, parent, idx);
                }
            }
            _ => {}
        }
    }

    /// Mark parameter `idx` of the (intra-CU) method `name` resolves to.
    fn mark_param(&mut self, name: &str, call: NodeId, idx: usize) {
        if let Some(decl) = self.decl_of_name(name, call) {
            if let Node::MethodDeclaration { parameters, .. } = self.arena.kind(decl) {
                if let Some(&p) = parameters.get(idx) {
                    if let Node::Parameter { id, .. } = self.arena.kind(p) {
                        let id = *id;
                        self.mark(id);
                    }
                }
            }
        }
    }

    // ---- propagation (fixpoint) ----

    fn propagate(&mut self) {
        loop {
            let mut changed = false;
            for i in 0..self.node_count() {
                let n = NodeId(i);
                match self.arena.kind(n).clone() {
                    Node::VariableDeclarator { id, init: Some(v), .. } => {
                        if self.expr_nullable(v) && self.mark(id) {
                            changed = true;
                        }
                    }
                    Node::AssignExpr { target, value, .. } => {
                        if self.expr_nullable(value) {
                            if let Node::NameExpr { name } = self.arena.kind(target) {
                                let name = name.clone();
                                if let Some(d) = self.decl_of_name(&name, target) {
                                    if self.mark(d) {
                                        changed = true;
                                    }
                                }
                            }
                        }
                    }
                    Node::ReturnStmt { expr: Some(e) } => {
                        if self.expr_nullable(e) {
                            if let Some(m) = self.enclosing_method(n) {
                                if self.mark(m) {
                                    changed = true;
                                }
                            }
                        }
                    }
                    Node::MethodCallExpr { name, args, .. } => {
                        for (i, &a) in args.iter().enumerate() {
                            if self.expr_nullable(a) {
                                let before = self.nullable.len();
                                self.mark_param(&name, n, i);
                                if self.nullable.len() != before {
                                    changed = true;
                                }
                            }
                        }
                    }
                    _ => {}
                }
            }
            if !changed {
                break;
            }
        }
    }

    /// Is this expression's value possibly null (already an `Option`)?
    pub fn expr_nullable(&self, e: NodeId) -> bool {
        match self.arena.kind(e) {
            Node::NullLiteralExpr => true,
            Node::NameExpr { name } => self
                .decl_of_name(name, e)
                .map(|d| self.nullable.contains(&d))
                .unwrap_or(false),
            Node::MethodCallExpr { scope: None, name, .. } => self
                .decl_of_name(name, e)
                .map(|d| self.nullable.contains(&d))
                .unwrap_or(false),
            Node::EnclosedExpr { inner: Some(i) } => self.expr_nullable(*i),
            Node::CastExpr { expr, .. } => self.expr_nullable(*expr),
            Node::ConditionalExpr { then_expr, else_expr, .. } => {
                self.expr_nullable(*then_expr) || self.expr_nullable(*else_expr)
            }
            _ => false,
        }
    }
}
