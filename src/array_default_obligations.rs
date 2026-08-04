//! Which source types some `new T[n]` needs a zero value for.
//!
//! Java's `new T[n]` allocates `n` slots initialised to `null`. The value
//! representation has no `null`, so the closest faithful allocation is `n`
//! default values — which requires `T: Default`. Rather than adding
//! `#[derive(Default)]` to every struct that could support it, this records the
//! *obligation*: the types an array creation actually asks for.
//!
//! That is the shape [0034](../claude_audits/0034-object-array-allocation-produces-an-empty-vec.md)
//! asks for, and the same shape as the `Clone` obligation
//! [0002a](../claude_audits/0002-object-aliasing-has-no-representation.md)
//! needed — a capability is added because a use site requires it, not because
//! it happens to be possible.

use std::collections::BTreeSet;

use crate::java_ir::{
    Block, CatchClause, Expr, JavaType, LambdaBody, Member, Program, Stmt, SwitchCase, TypeDecl,
};

/// Erased names of every type used as the element type of an array creation.
///
/// Names are the Java type names as written, plus their simple form, because a
/// creation can spell a nested type either way.
pub fn object_array_element_type_names(program: &Program) -> BTreeSet<String> {
    let mut names = BTreeSet::new();
    for ty in &program.types {
        collect_in_type(ty, &mut names);
    }
    names
}

/// Whether any array creation in `program` allocates elements of `ty`.
pub fn program_allocates_object_array_of(program: &Program, ty: &TypeDecl) -> bool {
    let names = object_array_element_type_names(program);
    names.contains(&ty.name) || names.contains(simple_name(&ty.name))
}

fn simple_name(name: &str) -> &str {
    name.rsplit(['.', '$']).next().unwrap_or(name)
}

fn record_element_type(ty: &JavaType, names: &mut BTreeSet<String>) {
    match ty {
        JavaType::Named { name, .. } => {
            names.insert(name.clone());
            names.insert(simple_name(name).to_string());
        }
        // A multi-dimensional creation still bottoms out in an element type.
        JavaType::Array(inner) => record_element_type(inner, names),
        _ => {}
    }
}

fn collect_in_type(ty: &TypeDecl, names: &mut BTreeSet<String>) {
    for member in &ty.members {
        match member {
            Member::Method(method) => {
                if let Some(body) = &method.body {
                    collect_in_block(body, names);
                }
            }
            Member::Constructor(constructor) => {
                if let Some(body) = &constructor.body {
                    collect_in_block(body, names);
                }
            }
            Member::Field(field) => {
                if let Some(initializer) = &field.initializer {
                    collect_in_expr(initializer, names);
                }
            }
            Member::Type(nested) => collect_in_type(nested, names),
            Member::Unsupported { .. } => {}
        }
    }
    for block in &ty.static_init_blocks {
        collect_in_block(&block.block, names);
    }
}

fn collect_in_block(block: &Block, names: &mut BTreeSet<String>) {
    for stmt in &block.statements {
        collect_in_stmt(stmt, names);
    }
}

fn collect_in_stmt(stmt: &Stmt, names: &mut BTreeSet<String>) {
    match stmt {
        Stmt::VarDecl { initializer, .. } => {
            if let Some(initializer) = initializer {
                collect_in_expr(initializer, names);
            }
        }
        Stmt::Assign { target, value } => {
            collect_in_expr(target, names);
            collect_in_expr(value, names);
        }
        Stmt::Expr(expr) | Stmt::Throw(expr) => collect_in_expr(expr, names),
        Stmt::Return(expr) => {
            if let Some(expr) = expr {
                collect_in_expr(expr, names);
            }
        }
        Stmt::If {
            condition,
            then_block,
            else_block,
        } => {
            collect_in_expr(condition, names);
            collect_in_block(then_block, names);
            if let Some(else_block) = else_block {
                collect_in_block(else_block, names);
            }
        }
        Stmt::While {
            condition, body, ..
        }
        | Stmt::DoWhile {
            condition, body, ..
        } => {
            collect_in_expr(condition, names);
            collect_in_block(body, names);
        }
        Stmt::For {
            init,
            condition,
            update,
            body,
            ..
        } => {
            for stmt in init {
                collect_in_stmt(stmt, names);
            }
            if let Some(condition) = condition {
                collect_in_expr(condition, names);
            }
            for expr in update {
                collect_in_expr(expr, names);
            }
            collect_in_block(body, names);
        }
        Stmt::EnhancedFor { iterable, body, .. } => {
            collect_in_expr(iterable, names);
            collect_in_block(body, names);
        }
        Stmt::Block(body) | Stmt::LabeledBlock { body, .. } => collect_in_block(body, names),
        Stmt::Try {
            resources,
            body,
            catches,
            finally_block,
        } => {
            for resource in resources {
                collect_in_stmt(resource, names);
            }
            collect_in_block(body, names);
            for CatchClause { body, .. } in catches {
                collect_in_block(body, names);
            }
            if let Some(finally_block) = finally_block {
                collect_in_block(finally_block, names);
            }
        }
        Stmt::Switch { selector, cases } => {
            collect_in_expr(selector, names);
            for SwitchCase { statements, .. } in cases {
                for stmt in statements {
                    collect_in_stmt(stmt, names);
                }
            }
        }
        Stmt::LocalType(nested) => collect_in_type(nested, names),
        Stmt::ConstructorInvocation { args, .. } => {
            for arg in args {
                collect_in_expr(arg, names);
            }
        }
        Stmt::Break(_) | Stmt::Continue(_) | Stmt::Unsupported(_) => {}
    }
}

fn collect_in_expr(expr: &Expr, names: &mut BTreeSet<String>) {
    match expr {
        Expr::ArrayCreation {
            element_type,
            dimensions,
            initializer,
        } => {
            record_element_type(element_type, names);
            for dimension in dimensions {
                collect_in_expr(dimension, names);
            }
            if let Some(initializer) = initializer {
                for element in initializer {
                    collect_in_expr(element, names);
                }
            }
        }
        Expr::ArrayInitializer(elements) => {
            for element in elements {
                collect_in_expr(element, names);
            }
        }
        Expr::Binary { left, right, .. } => {
            collect_in_expr(left, names);
            collect_in_expr(right, names);
        }
        Expr::Unary { expr, .. }
        | Expr::Cast { expr, .. }
        | Expr::InstanceOf { expr, .. }
        | Expr::FieldAccess(expr, _) => collect_in_expr(expr, names),
        Expr::ArrayAccess(receiver, index) => {
            collect_in_expr(receiver, names);
            collect_in_expr(index, names);
        }
        Expr::Ternary {
            condition,
            then_expr,
            else_expr,
        } => {
            collect_in_expr(condition, names);
            collect_in_expr(then_expr, names);
            collect_in_expr(else_expr, names);
        }
        Expr::Switch { selector, cases } => {
            collect_in_expr(selector, names);
            for case in cases {
                collect_in_expr(&case.value, names);
            }
        }
        Expr::Call { receiver, args, .. } => {
            if let Some(receiver) = receiver {
                collect_in_expr(receiver, names);
            }
            for arg in args {
                collect_in_expr(arg, names);
            }
        }
        Expr::New { args, .. } => {
            for arg in args {
                collect_in_expr(arg, names);
            }
        }
        Expr::Lambda { body, .. } => match body {
            LambdaBody::Expr(expr) => collect_in_expr(expr, names),
            LambdaBody::Block(block) => collect_in_block(block, names),
        },
        Expr::Literal(_)
        | Expr::Var(_)
        | Expr::This
        | Expr::Super
        | Expr::MethodReference { .. }
        | Expr::ClassLiteral(_)
        | Expr::Unsupported { .. }
        | Expr::Raw(_) => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::parse_java_source;

    fn names_for(source: &str) -> BTreeSet<String> {
        object_array_element_type_names(&parse_java_source(source).program)
    }

    #[test]
    fn an_object_array_creation_records_its_element_type() {
        let names = names_for(
            "class Holder { static class Node { int v; } void m() { Node[] cell = new Node[1]; } }",
        );
        assert!(names.contains("Node"), "{names:?}");
    }

    #[test]
    fn a_primitive_array_records_nothing() {
        // Primitives already lower correctly and need no derive.
        assert!(names_for("class Holder { void m() { int[] n = new int[2]; } }").is_empty());
    }

    #[test]
    fn a_creation_nested_in_an_expression_is_found() {
        let names = names_for(
            "class Holder { static class Node {} void m() { take(new Node[3]); } void take(Node[] n) {} }",
        );
        assert!(names.contains("Node"), "{names:?}");
    }

    /// The obligation has to be found wherever the creation is, not only in a
    /// method body — a field initialiser is a common place for a lookup table.
    #[test]
    fn a_field_initializer_creation_is_found() {
        let names = names_for("class Holder { static class Node {} Node[] table = new Node[8]; }");
        assert!(names.contains("Node"), "{names:?}");
    }

    #[test]
    fn a_multi_dimensional_creation_records_the_element_type() {
        let names = names_for(
            "class Holder { static class Node {} void m() { Node[][] g = new Node[2][2]; } }",
        );
        assert!(names.contains("Node"), "{names:?}");
    }
}
