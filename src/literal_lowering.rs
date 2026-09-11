use crate::java_ir::Literal;

pub(crate) fn literal_to_rust(lit: &Literal) -> String {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renders_java_literals_as_rust_literal_spelling() {
        assert_eq!(literal_to_rust(&Literal::Null), "None");
        assert_eq!(literal_to_rust(&Literal::Bool(true)), "true");
        assert_eq!(literal_to_rust(&Literal::Int(7)), "7");
        assert_eq!(literal_to_rust(&Literal::Long(7)), "7i64");
        assert_eq!(literal_to_rust(&Literal::Float(1.5)), "1.5f32");
        assert_eq!(literal_to_rust(&Literal::Double(1.5)), "1.5f64");
        assert_eq!(
            literal_to_rust(&Literal::String("name".to_string())),
            "\"name\".to_string()"
        );
        assert_eq!(literal_to_rust(&Literal::Char('x')), "'x'");
    }
}
