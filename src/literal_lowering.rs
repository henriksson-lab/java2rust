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
        Literal::Char(value) => format!("JavaCodeUnit({}u16)", *value as u32),
        Literal::CharUtf16(value) => format!("JavaCodeUnit({value}u16)"),
        // The scalar-only backend is still in migration. Keep this boundary
        // loud rather than corrupting values preserved by the IR.
        Literal::StringUtf16(_) => {
            "compile_error!(\"unsupported Java UTF-16 literal requires lossless text storage\")"
                .to_string()
        }
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
        assert_eq!(literal_to_rust(&Literal::Char('x')), "JavaCodeUnit(120u16)");
    }

    #[test]
    fn char_literals_retain_surrogates_while_scalar_string_storage_stays_explicit() {
        assert_eq!(
            literal_to_rust(&Literal::CharUtf16(0xd800)),
            "JavaCodeUnit(55296u16)"
        );
        let value = literal_to_rust(&Literal::StringUtf16(vec![0xdfff]));
        assert!(value.contains("compile_error!"));
        assert!(value.contains("lossless text storage"));
    }
}
