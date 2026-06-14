//! java2rust-rs — a behavioral reimplementation of the java2rust converter.
//!
//! Pipeline (mirrors `JavaConverter.convert`):
//!   parse -> IdTracker -> TypeTracker -> RustDumpVisitor -> String

pub mod adapter;
pub mod ast;
pub mod dump;
pub mod id_tracker;
pub mod modifiers;
pub mod naming;
pub mod parse;
pub mod type_tracker;

/// Convert a Java source string to the java2rust Rust output.
///
/// Mirrors `JavaConverter.convert2Rust(String)`.
pub fn convert(java: &str) -> String {
    let Some((arena, root)) = parse::create_compilation_unit(java) else {
        // PartParser would throw ParseException; JavaConverter.convert returns
        // e.toString(). We approximate by emitting nothing for now.
        return String::new();
    };

    let mut id_tracker = id_tracker::IdTracker::new();
    id_tracker::run(&arena, root, &mut id_tracker);
    type_tracker::run(&arena, root, &mut id_tracker);

    let mut dumper = dump::RustDumpVisitor::new(true, &arena, &mut id_tracker);
    dumper.visit(root, None);
    dumper.get_source()
}
