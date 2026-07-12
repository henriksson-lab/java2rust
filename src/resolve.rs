use std::collections::HashMap;

use crate::diagnostics::DiagnosticBag;
use crate::java_ir::{ConstructorDecl, Member, MethodDecl, Program, TypeDecl};

#[derive(Debug, Clone)]
pub struct ResolvedProgram {
    pub program: Program,
    pub diagnostics: DiagnosticBag,
}

pub fn resolve_program(program: &Program, mut diagnostics: DiagnosticBag) -> ResolvedProgram {
    let mut program = program.clone();
    for ty in &mut program.types {
        resolve_type(ty, &mut diagnostics);
    }
    ResolvedProgram {
        program,
        diagnostics,
    }
}

fn resolve_type(ty: &mut TypeDecl, diagnostics: &mut DiagnosticBag) {
    let mut method_counts: HashMap<String, usize> = HashMap::new();
    let mut ctor_count = 0usize;
    for member in &ty.members {
        match member {
            Member::Method(method) => {
                *method_counts.entry(method.name.clone()).or_default() += 1;
            }
            Member::Constructor(_) => ctor_count += 1,
            _ => {}
        }
    }

    for member in &mut ty.members {
        match member {
            Member::Method(method) => resolve_method(method, &method_counts),
            Member::Constructor(ctor) => resolve_constructor(ctor, ctor_count),
            Member::Type(nested) => resolve_type(nested, diagnostics),
            Member::Unsupported { reason, .. } => {
                diagnostics.unsupported("unsupported_member", reason.clone());
            }
            _ => {}
        }
    }
}

fn resolve_method(method: &mut MethodDecl, counts: &HashMap<String, usize>) {
    let base = crate::parser::to_snake_case(&method.name);
    if counts.get(&method.name).copied().unwrap_or(0) <= 1 {
        method.rust_name = Some(base);
        return;
    }

    let suffix = method
        .params
        .iter()
        .map(|p| crate::parser::type_suffix(&p.ty, p.is_varargs))
        .collect::<Vec<_>>()
        .join("_");
    method.rust_name = Some(format!("{base}_{suffix}"));
}

fn resolve_constructor(ctor: &mut ConstructorDecl, ctor_count: usize) {
    if ctor.params.is_empty() {
        ctor.rust_name = Some("new".to_string());
    } else if ctor_count == 1 {
        ctor.rust_name = Some("new".to_string());
    } else {
        let suffix = ctor
            .params
            .iter()
            .map(|p| crate::parser::type_suffix(&p.ty, p.is_varargs))
            .collect::<Vec<_>>()
            .join("_");
        ctor.rust_name = Some(format!("new_{suffix}"));
    }
}
