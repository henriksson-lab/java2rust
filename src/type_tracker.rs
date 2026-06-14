//! Port of `TypeTrackerVisitor`.

use crate::ast::{BinaryOp, Node, NodeId, UnaryOp};
use crate::ast::Arena;
use crate::ast::JClass;
use crate::id_tracker::IdTracker;

/// Mirrors `TypeTrackerVisitor.visit(CompilationUnit, null)`.
pub fn run(arena: &Arena, root: NodeId, t: &mut IdTracker) {
    let v = TypeVisitor { arena };
    v.visit(root, t);
}

struct TypeVisitor<'a> {
    arena: &'a Arena,
}

impl<'a> TypeVisitor<'a> {
    fn visit_children(&self, id: NodeId, t: &mut IdTracker) {
        for c in self.arena.children(id) {
            self.visit(c, t);
        }
    }

    /// Mirrors `propagateTypes`.
    fn propagate_types(&self, dest: NodeId, left: NodeId, right: NodeId, t: &mut IdTracker) {
        let l = t.get_type(left);
        let r = t.get_type(right);
        let is = |c: Option<JClass>, want: JClass| c == Some(want);
        if is(l, JClass::StringClass) || is(r, JClass::StringClass) {
            t.put_type(dest, JClass::StringClass);
        } else if is(l, JClass::DoubleType)
            || is(r, JClass::DoubleType)
            || is(l, JClass::DoubleClass)
            || is(r, JClass::DoubleClass)
            || is(l, JClass::FloatType)
            || is(r, JClass::FloatType)
            || is(l, JClass::FloatClass)
            || is(r, JClass::FloatClass)
        {
            t.put_type(dest, JClass::DoubleType);
        } else if is(l, JClass::BooleanType) || is(r, JClass::BooleanType) {
            t.put_type(dest, JClass::BooleanType);
        } else if is(l, JClass::IntType)
            || is(r, JClass::IntType)
            || is(l, JClass::ShortType)
            || is(r, JClass::ShortType)
            || is(l, JClass::LongType)
            || is(r, JClass::LongType)
            || is(l, JClass::ByteType)
            || is(r, JClass::ByteType)
            || is(l, JClass::IntegerClass)
            || is(r, JClass::IntegerClass)
            || is(l, JClass::ShortClass)
            || is(r, JClass::ShortClass)
            || is(l, JClass::LongClass)
            || is(r, JClass::LongClass)
            || is(l, JClass::ByteClass)
            || is(r, JClass::ByteClass)
        {
            t.put_type(dest, JClass::IntType);
        }
    }

    fn propagate_int_bool(&self, dest: NodeId, left: NodeId, right: NodeId, t: &mut IdTracker) {
        let l = t.get_type(left);
        let r = t.get_type(right);
        if l == Some(JClass::BooleanType) || r == Some(JClass::BooleanType) {
            t.put_type(dest, JClass::BooleanType);
        } else {
            t.put_type(dest, JClass::IntType);
        }
    }

    fn visit(&self, id: NodeId, t: &mut IdTracker) {
        use Node::*;
        match self.arena.kind(id).clone() {
            BinaryExpr { left, op, right } => {
                self.visit(left, t);
                self.visit(right, t);
                match op {
                    BinaryOp::Equals
                    | BinaryOp::NotEquals
                    | BinaryOp::And
                    | BinaryOp::Or
                    | BinaryOp::Less
                    | BinaryOp::Greater
                    | BinaryOp::LessEquals
                    | BinaryOp::GreaterEquals => t.put_type(id, JClass::BooleanType),
                    BinaryOp::BinOr | BinaryOp::BinAnd | BinaryOp::Xor => {
                        self.propagate_int_bool(id, left, right, t)
                    }
                    BinaryOp::LShift | BinaryOp::RSignedShift | BinaryOp::RUnsignedShift => {
                        t.put_type(id, JClass::IntType)
                    }
                    BinaryOp::Plus
                    | BinaryOp::Minus
                    | BinaryOp::Times
                    | BinaryOp::Divide
                    | BinaryOp::Remainder => self.propagate_types(id, left, right, t),
                }
            }
            IntegerLiteralExpr { .. }
            | IntegerLiteralMinValueExpr { .. }
            | LongLiteralExpr { .. }
            | LongLiteralMinValueExpr { .. } => {
                t.put_type(id, JClass::IntType);
                self.visit_children(id, t);
            }
            NameExpr { name } => {
                if let Some((Some(td), _)) = t.find_declaration_node_for(self.arena, &name, id) {
                    t.put_type(id, td.clazz);
                }
                self.visit_children(id, t);
            }
            StringLiteralExpr { .. } => {
                t.put_type(id, JClass::StringClass);
                self.visit_children(id, t);
            }
            BooleanLiteralExpr { .. } => {
                t.put_type(id, JClass::BooleanType);
                self.visit_children(id, t);
            }
            CharLiteralExpr { .. } => {
                t.put_type(id, JClass::CharType);
                self.visit_children(id, t);
            }
            DoubleLiteralExpr { .. } => {
                t.put_type(id, JClass::DoubleType);
                self.visit_children(id, t);
            }
            UnaryExpr { expr, op } => {
                match op {
                    UnaryOp::Positive | UnaryOp::Negative => {
                        self.propagate_types(id, expr, expr, t)
                    }
                    UnaryOp::Not => t.put_type(id, JClass::BooleanType),
                    UnaryOp::Inverse
                    | UnaryOp::PosIncrement
                    | UnaryOp::PosDecrement
                    | UnaryOp::PreIncrement
                    | UnaryOp::PreDecrement => t.put_type(id, JClass::IntType),
                }
                self.visit_children(id, t);
            }
            ArrayAccessExpr { name, index } => {
                if let Node::NameExpr { name: nm } = self.arena.kind(name) {
                    if let Some((Some(td), _)) =
                        t.find_declaration_node_for(self.arena, nm, name)
                    {
                        t.put_type(id, td.clazz);
                    }
                }
                t.put_type(index, JClass::IntType);
                // (the original also rewrites the index var's declared type; omitted —
                // it mutates a shared TypeDescription that does not affect output here)
                self.visit_children(id, t);
            }
            MethodCallExpr { scope, args, .. } => {
                if let Some(s) = scope {
                    self.visit(s, t);
                }
                for a in args {
                    self.visit(a, t);
                }
            }
            _ => self.visit_children(id, t),
        }
    }
}
