// Shared by Character.toString(char) and Character.toString(int). Java chars
// retain single surrogate units; int inputs denote checked Unicode code points.
pub trait JavaCharacterStringValue {
    fn java_character_string_point(self) -> i32;
}
impl JavaCharacterStringValue for JavaCodeUnit {
    fn java_character_string_point(self) -> i32 {
        self.widen_i32()
    }
}
impl JavaCharacterStringValue for char {
    fn java_character_string_point(self) -> i32 {
        self as i32
    }
}
impl JavaCharacterStringValue for i32 {
    fn java_character_string_point(self) -> i32 {
        self
    }
}
pub fn java_character_to_string_result(
    value: impl JavaCharacterStringValue,
) -> Result<JavaUtf16, JavaException> {
    let point = value.java_character_string_point();
    if !(0..=0x10ffff).contains(&point) {
        return Err(JavaException::new(
            "IllegalArgumentException",
            format!("Not a valid Unicode code point: {point:#x}"),
        ));
    }
    let units = if point <= 0xffff {
        vec![point as u16]
    } else {
        let supplementary = point as u32 - 0x10000;
        vec![
            0xd800 | (supplementary >> 10) as u16,
            0xdc00 | (supplementary & 0x3ff) as u16,
        ]
    };
    Ok(JavaUtf16::from_units(units))
}
