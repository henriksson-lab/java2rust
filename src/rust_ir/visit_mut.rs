//! Structured value traversal. Visitors decide whether opaque leaves, bindings
//! and deferred bodies are safe for their transformation; the walker neither
//! parses raw Rust nor changes evaluation order.

use super::{RustClosureBody, RustExpr, RustStmt};

pub(crate) trait VisitMut {
    fn visit_expr(&mut self, expr: &mut RustExpr) {
        walk_expr(self, expr);
    }

    fn visit_stmt(&mut self, stmt: &mut RustStmt) {
        walk_stmt(self, stmt);
    }
}

pub(crate) fn walk_stmt<V: VisitMut + ?Sized>(visitor: &mut V, stmt: &mut RustStmt) {
    match stmt {
        RustStmt::Let { value, .. } | RustStmt::Return(value) => {
            if let Some(value) = value {
                visitor.visit_expr(value);
            }
        }
        RustStmt::Assign { target, value } | RustStmt::AssignOp { target, value, .. } => {
            visitor.visit_expr(target);
            visitor.visit_expr(value);
        }
        RustStmt::Expr(expr) | RustStmt::BreakValue { value: expr, .. } => {
            visitor.visit_expr(expr);
        }
        RustStmt::If {
            condition,
            then_body,
            else_body,
        }
        | RustStmt::IfLet {
            expr: condition,
            then_body,
            else_body,
            ..
        } => {
            visitor.visit_expr(condition);
            for stmt in then_body.iter_mut().chain(else_body) {
                visitor.visit_stmt(stmt);
            }
        }
        RustStmt::While { condition, body }
        | RustStmt::WhileLet {
            expr: condition,
            body,
            ..
        }
        | RustStmt::For {
            iterable: condition,
            body,
            ..
        } => {
            visitor.visit_expr(condition);
            for stmt in body {
                visitor.visit_stmt(stmt);
            }
        }
        RustStmt::Loop { body, .. } => {
            for stmt in body {
                visitor.visit_stmt(stmt);
            }
        }
        RustStmt::Raw(_) | RustStmt::Break(_) | RustStmt::Continue(_) => {}
    }
}

pub(crate) fn walk_expr<V: VisitMut + ?Sized>(visitor: &mut V, expr: &mut RustExpr) {
    match expr {
        RustExpr::Field(expr, _)
        | RustExpr::Grouped(expr)
        | RustExpr::Try(expr)
        | RustExpr::Unary { expr, .. }
        | RustExpr::Cast { expr, .. }
        | RustExpr::Shaped { expr, .. }
        | RustExpr::PostfixComment { expr, .. }
        | RustExpr::MarkedBlock { expr, .. } => visitor.visit_expr(expr),
        RustExpr::Index(left, right)
        | RustExpr::Binary { left, right, .. }
        | RustExpr::Range {
            start: left,
            end: right,
            ..
        }
        | RustExpr::VecRepeat {
            value: left,
            len: right,
        } => {
            visitor.visit_expr(left);
            visitor.visit_expr(right);
        }
        RustExpr::If {
            condition,
            then_expr,
            else_expr,
        } => {
            visitor.visit_expr(condition);
            visitor.visit_expr(then_expr);
            visitor.visit_expr(else_expr);
        }
        RustExpr::Tuple(items)
        | RustExpr::ArrayLiteral(items)
        | RustExpr::VecLiteral(items)
        | RustExpr::MacroCall { args: items, .. }
        | RustExpr::UnsupportedJavaArrayCreation {
            dimensions: items, ..
        } => {
            for item in items {
                visitor.visit_expr(item);
            }
        }
        RustExpr::Call { function, args }
        | RustExpr::MethodCall {
            receiver: function,
            args,
            ..
        } => {
            visitor.visit_expr(function);
            for arg in args {
                visitor.visit_expr(arg);
            }
        }
        RustExpr::Closure { body, .. } => match body {
            RustClosureBody::Expr(expr) => visitor.visit_expr(expr),
        },
        RustExpr::Match { expr, arms } => {
            visitor.visit_expr(expr);
            for arm in arms {
                visitor.visit_expr(&mut arm.body);
            }
        }
        RustExpr::Block { stmts, tail } => {
            for stmt in stmts {
                visitor.visit_stmt(stmt);
            }
            if let Some(tail) = tail {
                visitor.visit_expr(tail);
            }
        }
        RustExpr::StructLit { fields, .. } => {
            for (_, value) in fields {
                visitor.visit_expr(value);
            }
        }
        RustExpr::Literal(_)
        | RustExpr::Var(_)
        | RustExpr::Path(_)
        | RustExpr::GenericPath { .. }
        | RustExpr::TypeAssociatedPath { .. }
        | RustExpr::TraitAssociatedPath { .. }
        | RustExpr::Pattern(_)
        | RustExpr::Raw(_) => {}
    }
}
