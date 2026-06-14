//! Port of `com.github.javaparser.ast.body.ModifierSet` (bitmask).
//!
//! Values mirror `java.lang.reflect.Modifier`.

pub const PUBLIC: i32 = 0x0001;
pub const PRIVATE: i32 = 0x0002;
pub const PROTECTED: i32 = 0x0004;
pub const STATIC: i32 = 0x0008;
pub const FINAL: i32 = 0x0010;
pub const SYNCHRONIZED: i32 = 0x0020;
pub const VOLATILE: i32 = 0x0040;
pub const TRANSIENT: i32 = 0x0080;
pub const NATIVE: i32 = 0x0100;
pub const ABSTRACT: i32 = 0x0400;
pub const STRICTFP: i32 = 0x0800;

pub fn is_public(m: i32) -> bool {
    m & PUBLIC != 0
}
pub fn is_private(m: i32) -> bool {
    m & PRIVATE != 0
}
pub fn is_protected(m: i32) -> bool {
    m & PROTECTED != 0
}
pub fn is_static(m: i32) -> bool {
    m & STATIC != 0
}
pub fn is_final(m: i32) -> bool {
    m & FINAL != 0
}
pub fn is_synchronized(m: i32) -> bool {
    m & SYNCHRONIZED != 0
}
pub fn is_volatile(m: i32) -> bool {
    m & VOLATILE != 0
}
pub fn is_transient(m: i32) -> bool {
    m & TRANSIENT != 0
}
pub fn is_native(m: i32) -> bool {
    m & NATIVE != 0
}
pub fn is_abstract(m: i32) -> bool {
    m & ABSTRACT != 0
}
pub fn is_strictfp(m: i32) -> bool {
    m & STRICTFP != 0
}

/// Map a modifier keyword (as it appears in tree-sitter) to its bit.
pub fn keyword_bit(kw: &str) -> i32 {
    match kw {
        "public" => PUBLIC,
        "private" => PRIVATE,
        "protected" => PROTECTED,
        "static" => STATIC,
        "final" => FINAL,
        "synchronized" => SYNCHRONIZED,
        "volatile" => VOLATILE,
        "transient" => TRANSIENT,
        "native" => NATIVE,
        "abstract" => ABSTRACT,
        "strictfp" => STRICTFP,
        _ => 0,
    }
}
