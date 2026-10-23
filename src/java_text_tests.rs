use crate::java_text::{JavaCodeUnit as Unit, JavaTextBounds, JavaUtf16 as Text};
use crate::java_utf16_string::JavaUtf16String as Object;

#[test]
fn lossless_string_objects_keep_value_identity_and_scalar_interop_separate() {
    let high = Object::from_units(vec![0xd83d]);
    let low = Object::from_units(vec![0xde00]);
    let combined = high.append(&low);
    assert_eq!(combined.scalar_string().unwrap(), "😀");
    assert!(high.scalar_string().is_err());
    assert_eq!(combined.substring(0, 1).unwrap().units(), high.units());
    assert!(
        combined
            .substring(0, 2)
            .unwrap()
            .java_identity_eq(&combined)
    );
    assert!(
        !combined
            .append(&Object::from_scalar(""))
            .java_identity_eq(&combined)
    );
    assert!(
        combined
            .concat(&Object::from_scalar(""))
            .java_identity_eq(&combined)
    );
    let interned = high.intern();
    assert!(
        Object::from_units(vec![0xd83d])
            .intern()
            .java_identity_eq(&interned)
    );
    assert!(!Object::from_units(vec![0xd83d]).java_identity_eq(&interned));
    assert!(high.java_equals(&interned));
}

#[test]
fn every_java_char_code_unit_survives_narrowing_and_widening() {
    for value in 0..=65535 {
        let unit = Unit::narrow_i32(value);
        assert_eq!(unit.as_u16(), value as u16);
        assert_eq!(unit.widen_i32(), value);
    }
    assert_eq!(Unit::narrow_i32(-1).as_u16(), 65535);
    assert_eq!(Unit::narrow_i32(65536).as_u16(), 0);
    assert_eq!(Unit::default().as_u16(), 0);
}

#[test]
fn utf16_indexing_preserves_bmp_and_surrogate_positions() {
    let text = Text::from_scalar("Aé😀Z");
    assert_eq!(text.len(), 5);
    assert_eq!(text.units(), &[65, 233, 0xd83d, 0xde00, 90]);
    for (i, expected) in text.units().iter().enumerate() {
        assert_eq!(text.char_at(i as i32).unwrap().as_u16(), *expected);
    }
    assert_eq!(
        text.char_at(-1),
        Err(JavaTextBounds::Index {
            index: -1,
            length: 5
        })
    );
    assert_eq!(
        text.char_at(5),
        Err(JavaTextBounds::Index {
            index: 5,
            length: 5
        })
    );
    assert_eq!(text.substring(2, 4).unwrap().scalar_string().unwrap(), "😀");
}

#[test]
fn substrings_may_hold_isolated_units_without_lossy_utf8_conversion() {
    let emoji = Text::from_scalar("😀");
    let high = emoji.substring(0, 1).unwrap();
    let low = emoji.substring(1, 2).unwrap();
    assert_eq!(high.units(), &[0xd83d]);
    assert_eq!(low.units(), &[0xde00]);
    assert!(high.scalar_string().is_err());
    assert!(low.scalar_string().is_err());
    assert_eq!(high.concat(&low), emoji);
    assert_eq!(high.concat(&low).scalar_string().unwrap(), "😀");
    assert_eq!(
        emoji.substring(1, 0),
        Err(JavaTextBounds::Range {
            begin: 1,
            end: 0,
            length: 2
        })
    );
    assert_eq!(
        emoji.substring(0, 3),
        Err(JavaTextBounds::Range {
            begin: 0,
            end: 3,
            length: 2
        })
    );
}

#[test]
fn java_search_hash_and_order_use_code_units_not_bytes_or_scalar_values() {
    let text = Text::from_scalar("Aé😀Z");
    assert_eq!(text.index_of(&Text::from_scalar("Z"), 2), 4);
    assert_eq!(text.last_index_of(&Text::from_scalar("Z"), 4), 4);
    assert_eq!(text.index_of_code_point(0x1f600, 0), 2);
    assert_eq!(text.index_of_code_point(0xd83d, 0), 2);
    assert_eq!(text.index_of_code_point(0xde00, 0), 3);
    assert_eq!(text.index_of_code_point(0x110000, 0), -1);
    assert_eq!(text.index_of(&Text::default(), i32::MAX), 5);
    assert_eq!(text.last_index_of(&Text::default(), -1), -1);
    assert_eq!(
        Text::from_scalar("aba").last_index_of(&Text::from_scalar("a"), i32::MAX),
        2
    );
    assert_eq!(Text::from_scalar("abc").java_hash(), 96354);
    assert!(Text::from_scalar("😀").java_compare(&Text::from_units(vec![0xe000])) < 0);
    assert_eq!(Text::from_units(vec![0xd83d]).java_hash(), 0xd83d);
}
