use std::collections::HashMap;

use crate::java_ir::{Expr, JavaType, Member, Program, Stmt, TypeDecl};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct UsageFacts {
    pub methods: HashMap<UsageMethodKey, MethodUsageFacts>,
    pub constructors: HashMap<UsageConstructorKey, MethodUsageFacts>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct UsageMethodKey {
    pub type_name: String,
    pub method_name: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct UsageConstructorKey {
    pub type_name: String,
    pub rust_name: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MethodUsageFacts {
    pub bindings: HashMap<String, BindingUseFacts>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BindingUseFacts {
    pub name: String,
    pub java_ty: JavaType,
    pub uses: Vec<BindingUse>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BindingUse {
    ContentRead,
    PassedToCall { name: String, arg_index: usize },
    Stored,
    Returned,
    Captured,
    Mutated,
}

pub fn analyze_usage(program: &Program) -> UsageFacts {
    let mut facts = UsageFacts::default();
    for ty in &program.types {
        analyze_type_usage(ty, &mut facts);
    }
    facts
}

fn analyze_type_usage(ty: &TypeDecl, facts: &mut UsageFacts) {
    for member in &ty.members {
        match member {
            Member::Method(method) => {
                let mut method_facts = MethodUsageFacts::default();
                for param in &method.params {
                    method_facts.bindings.insert(
                        param.name.clone(),
                        BindingUseFacts {
                            name: param.name.clone(),
                            java_ty: param.ty.clone(),
                            uses: Vec::new(),
                        },
                    );
                }
                if let Some(body) = &method.body {
                    for stmt in &body.statements {
                        analyze_stmt(stmt, &mut method_facts);
                    }
                }
                facts.methods.insert(
                    UsageMethodKey {
                        type_name: ty.name.clone(),
                        method_name: method.name.clone(),
                    },
                    method_facts,
                );
            }
            Member::Constructor(ctor) => {
                let mut ctor_facts = MethodUsageFacts::default();
                for param in &ctor.params {
                    ctor_facts.bindings.insert(
                        param.name.clone(),
                        BindingUseFacts {
                            name: param.name.clone(),
                            java_ty: param.ty.clone(),
                            uses: Vec::new(),
                        },
                    );
                }
                if let Some(body) = &ctor.body {
                    for stmt in &body.statements {
                        analyze_stmt(stmt, &mut ctor_facts);
                    }
                }
                facts.constructors.insert(
                    UsageConstructorKey {
                        type_name: ty.name.clone(),
                        rust_name: ctor.rust_name.clone().unwrap_or_else(|| "new".to_string()),
                    },
                    ctor_facts,
                );
            }
            Member::Type(nested) => analyze_type_usage(nested, facts),
            _ => {}
        }
    }
}

fn analyze_stmt(stmt: &Stmt, facts: &mut MethodUsageFacts) {
    match stmt {
        Stmt::VarDecl {
            ty,
            name,
            initializer,
        } => {
            facts
                .bindings
                .entry(name.clone())
                .or_insert(BindingUseFacts {
                    name: name.clone(),
                    java_ty: ty.clone(),
                    uses: Vec::new(),
                });
            if let Some(initializer) = initializer {
                analyze_expr(initializer, facts);
            }
        }
        Stmt::Assign { target, value } => {
            if let Expr::Var(name) = target {
                record_use(facts, name, BindingUse::Mutated);
            }
            if matches!(target, Expr::FieldAccess(_, _)) {
                analyze_stored_expr(value, facts);
            }
            analyze_expr(target, facts);
            analyze_expr(value, facts);
        }
        Stmt::Return(Some(expr)) => {
            if let Expr::Var(name) = expr {
                record_use(facts, name, BindingUse::Returned);
            }
            analyze_expr(expr, facts);
        }
        Stmt::Return(None) => {}
        Stmt::Expr(expr) => analyze_expr(expr, facts),
        Stmt::If {
            condition,
            then_block,
            else_block,
        } => {
            analyze_expr(condition, facts);
            for stmt in &then_block.statements {
                analyze_stmt(stmt, facts);
            }
            if let Some(else_block) = else_block {
                for stmt in &else_block.statements {
                    analyze_stmt(stmt, facts);
                }
            }
        }
        Stmt::While { condition, body } => {
            analyze_expr(condition, facts);
            for stmt in &body.statements {
                analyze_stmt(stmt, facts);
            }
        }
        Stmt::For {
            init,
            condition,
            update,
            body,
            ..
        } => {
            for stmt in init {
                analyze_stmt(stmt, facts);
            }
            if let Some(condition) = condition {
                analyze_expr(condition, facts);
            }
            for expr in update {
                analyze_expr(expr, facts);
            }
            for stmt in &body.statements {
                analyze_stmt(stmt, facts);
            }
        }
        Stmt::EnhancedFor { iterable, body, .. } => {
            analyze_expr(iterable, facts);
            for stmt in &body.statements {
                analyze_stmt(stmt, facts);
            }
        }
        Stmt::Switch { selector, cases } => {
            analyze_expr(selector, facts);
            for case in cases {
                for stmt in &case.statements {
                    analyze_stmt(stmt, facts);
                }
            }
        }
        Stmt::Block(block) | Stmt::Try { body: block, .. } => {
            for stmt in &block.statements {
                analyze_stmt(stmt, facts);
            }
        }
        Stmt::Throw(expr) => analyze_expr(expr, facts),
        Stmt::ConstructorInvocation { args, .. } => {
            for (arg_index, arg) in args.iter().enumerate() {
                if let Expr::Var(name) = arg {
                    record_use(
                        facts,
                        name,
                        BindingUse::PassedToCall {
                            name: "constructor".to_string(),
                            arg_index,
                        },
                    );
                }
                analyze_expr(arg, facts);
            }
        }
        Stmt::LocalType(_) | Stmt::Break(_) | Stmt::Continue(_) | Stmt::Unsupported(_) => {}
    }
}

fn analyze_expr(expr: &Expr, facts: &mut MethodUsageFacts) {
    match expr {
        Expr::FieldAccess(receiver, field) => {
            if field == "length"
                && let Expr::Var(name) = receiver.as_ref()
            {
                record_use(facts, name, BindingUse::ContentRead);
            }
            analyze_expr(receiver, facts);
        }
        Expr::Call {
            receiver,
            name,
            args,
            ..
        } => {
            if matches!(name.as_str(), "length" | "isEmpty" | "trim")
                && let Some(receiver) = receiver
                && let Expr::Var(binding) = receiver.as_ref()
            {
                record_use(facts, binding, BindingUse::ContentRead);
            }
            for (arg_index, arg) in args.iter().enumerate() {
                if let Expr::Var(binding) = arg {
                    record_use(
                        facts,
                        binding,
                        BindingUse::PassedToCall {
                            name: name.clone(),
                            arg_index,
                        },
                    );
                }
                analyze_expr(arg, facts);
            }
            if let Some(receiver) = receiver {
                analyze_expr(receiver, facts);
            }
        }
        Expr::Binary { left, right, .. } => {
            analyze_expr(left, facts);
            analyze_expr(right, facts);
        }
        Expr::Unary { expr, .. }
        | Expr::Cast { expr, .. }
        | Expr::ArrayAccess(expr, _)
        | Expr::InstanceOf { expr, .. } => analyze_expr(expr, facts),
        Expr::Ternary {
            condition,
            then_expr,
            else_expr,
        } => {
            analyze_expr(condition, facts);
            analyze_expr(then_expr, facts);
            analyze_expr(else_expr, facts);
        }
        Expr::New { args, .. } => {
            for arg in args {
                analyze_expr(arg, facts);
            }
        }
        Expr::Lambda { .. } => {}
        Expr::Literal(_)
        | Expr::Var(_)
        | Expr::This
        | Expr::Super
        | Expr::MethodReference { .. }
        | Expr::ClassLiteral(_)
        | Expr::Raw(_) => {}
    }
}

fn analyze_stored_expr(expr: &Expr, facts: &mut MethodUsageFacts) {
    if let Expr::Var(name) = expr {
        record_use(facts, name, BindingUse::Stored);
    }
}

fn record_use(facts: &mut MethodUsageFacts, name: &str, usage: BindingUse) {
    if let Some(binding) = facts.bindings.get_mut(name) {
        binding.uses.push(usage);
    }
}
