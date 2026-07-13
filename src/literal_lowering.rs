use crate::java_ir::Literal;

pub fn literal_to_rust(lit: &Literal) -> String {
    match lit {
        Literal::Null => "None".to_string(),
        Literal::Bool(value) => value.to_string(),
        Literal::Int(value) => value.to_string(),
        Literal::Long(value) => format!("{value}i64"),
        Literal::Float(value) => format!("{value}f32"),
        Literal::Double(value) => format!("{value}f64"),
        Literal::String(value) => format!("{value:?}.to_string()"),
        Literal::Char(value) => format!("{value:?}"),
    }
}
