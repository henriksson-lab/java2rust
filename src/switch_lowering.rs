use crate::rust_ir::{RustExpr, RustStmt};
use crate::switch_plan::{SwitchCaseLabelPlan, SwitchCasePlan, SwitchPlan};

#[derive(Debug, Clone, PartialEq)]
pub struct SwitchRenderCase {
    pub label_condition: String,
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
    out.push_str(&format!("        break '{};\n", plan.break_label));
    out.push_str("    }\n");
    out.push('}');
    out
}
