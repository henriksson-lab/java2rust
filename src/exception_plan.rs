use std::collections::BTreeMap;

use crate::facts::FactSet;
use crate::java_ir::{Expr, JavaType, Member, MethodDecl, Program, TypeDecl};

#[derive(Debug, Clone, PartialEq)]
pub enum ThrowExprPlan<'a> {
    NewException {
        class_name: String,
        args: &'a [Expr],
    },
    FallbackValue(&'a Expr),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct JavaExceptionSupportPlan {
    pub emit_support_item: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MethodExceptionPlan {
    pub wrap_returns_in_result: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProgramExceptionPlan {
    pub support: JavaExceptionSupportPlan,
    methods: BTreeMap<(String, String), MethodExceptionPlan>,
}

impl ProgramExceptionPlan {
    pub fn method_plan(&self, ty: &TypeDecl, method: &MethodDecl) -> MethodExceptionPlan {
        self.methods
            .get(&(ty.name.clone(), method.name.clone()))
            .copied()
            .unwrap_or(MethodExceptionPlan {
                wrap_returns_in_result: false,
            })
    }
}

impl Default for ProgramExceptionPlan {
    fn default() -> Self {
        Self {
            support: JavaExceptionSupportPlan {
                emit_support_item: false,
            },
            methods: BTreeMap::new(),
        }
    }
}

pub fn program_uses_java_exception_with_facts(_program: &Program, facts: &FactSet) -> bool {
    facts.has_method_throw_effects()
        || !facts.exception_facts.is_empty()
        || !facts.catch_facts.is_empty()
}

pub fn plan_java_exception_support(program: &Program, facts: &FactSet) -> JavaExceptionSupportPlan {
    JavaExceptionSupportPlan {
        emit_support_item: program_uses_java_exception_with_facts(program, facts),
    }
}

pub fn plan_method_exceptions(
    ty: &TypeDecl,
    method: &MethodDecl,
    facts: &FactSet,
) -> MethodExceptionPlan {
    MethodExceptionPlan {
        wrap_returns_in_result: facts.method_has_thrown_exceptions(&ty.name, &method.name),
    }
}

pub fn plan_program_exceptions(program: &Program, facts: &FactSet) -> ProgramExceptionPlan {
    let mut methods = BTreeMap::new();
    for ty in &program.types {
        collect_method_exception_plans_for_type(ty, facts, &mut methods);
    }
    ProgramExceptionPlan {
        support: plan_java_exception_support(program, facts),
        methods,
    }
}

fn collect_method_exception_plans_for_type(
    ty: &TypeDecl,
    facts: &FactSet,
    methods: &mut BTreeMap<(String, String), MethodExceptionPlan>,
) {
    for member in &ty.members {
        match member {
            Member::Method(method) => {
                methods.insert(
                    (ty.name.clone(), method.name.clone()),
                    plan_method_exceptions(ty, method, facts),
                );
            }
            Member::Type(nested) => collect_method_exception_plans_for_type(nested, facts, methods),
            _ => {}
        }
    }
}

pub fn plan_throw_expr(expr: &Expr) -> ThrowExprPlan<'_> {
    match expr {
        Expr::New { ty, args, .. } => ThrowExprPlan::NewException {
            class_name: exception_class_name(ty),
            args,
        },
        _ => ThrowExprPlan::FallbackValue(expr),
    }
}

pub fn exception_class_name(class_ty: &JavaType) -> String {
    match class_ty {
        JavaType::Named { name, .. } => name.clone(),
        JavaType::Unknown(raw) => raw.clone(),
        _ => "JavaException".to_string(),
    }
}
