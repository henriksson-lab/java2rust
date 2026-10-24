/// Shared Character.digit(char, 10) boundary for Java numeric constructors.
/// Java consumes UTF-16 chars here: supplementary digits are surrogate pairs,
/// not one accepted decimal digit.
pub fn java_numeric_decimal_digit(character: u16) -> Option<u8> {
    const ZEROS: &[u32] = &[
        0x0030, 0x0660, 0x06f0, 0x07c0, 0x0966, 0x09e6, 0x0a66, 0x0ae6, 0x0b66, 0x0be6, 0x0c66,
        0x0ce6, 0x0d66, 0x0de6, 0x0e50, 0x0ed0, 0x0f20, 0x1040, 0x1090, 0x17e0, 0x1810, 0x1946,
        0x19d0, 0x1a80, 0x1a90, 0x1b50, 0x1bb0, 0x1c40, 0x1c50, 0xa620, 0xa8d0, 0xa900, 0xa9d0,
        0xa9f0, 0xaa50, 0xabf0, 0xff10,
    ];
    let code = character as u32;
    ZEROS.iter().find_map(|zero| {
        code.checked_sub(*zero)
            .filter(|value| *value < 10)
            .map(|value| value as u8)
    })
}
