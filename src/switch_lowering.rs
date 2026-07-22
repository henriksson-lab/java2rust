use crate::rust_ir::{RustExpr, RustStmt};
use crate::switch_plan::{SwitchCaseLabelPlan, SwitchCasePlan, SwitchPlan};

#[derive(Debug, Clone, PartialEq)]
pub struct SwitchRenderCase {
    pub label_condition: String,
    pub statements: Vec<RustStmt>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SwitchMatchRenderCase {
    pub patterns: Vec<String>,
    pub statements: Vec<RustStmt>,
}

pub fn switch_case_condition(
    plan: &SwitchPlan<'_>,
    case_plan: &SwitchCasePlan,
    label_exprs: Vec<RustExpr>,
) -> String {
    if case_plan
        .labels
        .iter()
        .any(|label| matches!(label, SwitchCaseLabelPlan::Default))
    {
        return format!("!{}", plan.matched_binding);
    }
    let comparisons = label_exprs
        .iter()
        .map(|expr| format!("{} == {}", plan.value_binding, crate::emit::emit_expr(expr)))
        .collect::<Vec<_>>();
    if comparisons.is_empty() {
        "false".to_string()
    } else {
        format!(
            "!{} && ({})",
            plan.matched_binding,
            comparisons.join(" || ")
        )
    }
}

pub fn render_switch(
    plan: &SwitchPlan<'_>,
    selector: RustExpr,
    cases: Vec<SwitchRenderCase>,
    append_unreachable: bool,
) -> String {
    let mut out = String::new();
    out.push_str("{\n");
    out.push_str(&format!(
        "    let {} = {};\n",
        plan.value_binding,
        crate::emit::emit_expr(&selector)
    ));
    out.push_str(&format!("    let mut {} = false;\n", plan.matched_binding));
    out.push_str(&format!("    '{}: loop {{\n", plan.break_label));
    for case in cases {
        out.push_str(&format!("        if {} {{\n", case.label_condition));
        out.push_str(&format!("            {} = true;\n", plan.matched_binding));
        out.push_str("        }\n");
        out.push_str(&format!("        if {} {{\n", plan.matched_binding));
        out.push_str(&crate::emit::emit_statements(&case.statements, 3));
        out.push_str("        }\n");
    }
    out.push_str(&format!("        break '{} ();\n", plan.break_label));
    out.push_str("    }\n");
    out.push_str("};");
    if append_unreachable {
        out.push_str("\n    unreachable!(\"Java switch completed without returning\");");
    }
    out
}

pub fn render_simple_return_match_switch(
    selector: RustExpr,
    cases: Vec<SwitchMatchRenderCase>,
    default_statements: Option<Vec<RustStmt>>,
) -> String {
    let mut out = String::new();
    out.push_str("match ");
    out.push_str(&crate::emit::emit_expr(&selector));
    out.push_str(" {\n");
    for case in cases {
        if case.patterns.is_empty() {
            continue;
        }
        out.push_str("    ");
        out.push_str(&case.patterns.join(" | "));
        out.push_str(" => {\n");
        out.push_str(&crate::emit::emit_statements(&case.statements, 2));
        out.push_str("    }\n");
    }
    out.push_str("    _ => {\n");
    if let Some(statements) = default_statements {
        out.push_str(&crate::emit::emit_statements(&statements, 2));
    }
    out.push_str("    }\n");
    out.push('}');
    out
}

pub fn switch_match_pattern_expr(expr: RustExpr) -> String {
    crate::emit::emit_expr(&expr)
}
