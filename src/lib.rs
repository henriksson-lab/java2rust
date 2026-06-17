//! java2rust-rs — a behavioral reimplementation of the java2rust converter.
//!
//! Pipeline (mirrors `JavaConverter.convert`):
//!   parse -> IdTracker -> TypeTracker -> RustDumpVisitor -> String

pub mod adapter;
pub mod ast;
pub mod borrow;
pub mod crate_layout;
pub mod dump;
pub mod id_tracker;
pub mod modifiers;
pub mod naming;
pub mod nullability;
pub mod parse;
pub mod stdlib;
pub mod stubs;
pub mod symbol_map;
pub mod type_tracker;
pub mod types;

use std::collections::HashSet;

use stubs::StubCollector;
use symbol_map::LinkIndex;

/// Convert a Java source string to the java2rust Rust output.
///
/// Mirrors `JavaConverter.convert2Rust(String)`.
pub fn convert(java: &str) -> String {
    convert_with_links(java, &LinkIndex::default())
}

/// Like [`convert`], but resolves referenced types against one or more
/// previously-translated dependency symbol maps (`link`), so references to those
/// types emit their real Rust paths instead of bare names.
pub fn convert_with_links(java: &str, link: &LinkIndex) -> String {
    convert_full(java, link, &HashSet::new(), false).0
}

/// Full conversion entry point: in addition to [`convert_with_links`], when
/// `emit_stubs` is set it records every unresolved external symbol (one not
/// covered by `link`, the stdlib mapping, or `known_types`) and returns the
/// collected [`StubCollector`] alongside the Rust source. `known_types` holds
/// the FQNs of types defined elsewhere in the same translated tree, so their
/// cross-file references are not mistaken for missing externals.
pub fn convert_full(
    java: &str,
    link: &LinkIndex,
    known_types: &HashSet<String>,
    emit_stubs: bool,
) -> (String, StubCollector) {
    convert_full_opts(java, link, known_types, emit_stubs, false)
}

/// Like [`convert_full`], but with an explicit `crate_mode` flag: in crate mode,
/// linked dependency paths are made `crate::`-relative (the deps are emitted as
/// crate modules by [`crate_layout::generate_dep_modules`]).
pub fn convert_full_opts(
    java: &str,
    link: &LinkIndex,
    known_types: &HashSet<String>,
    emit_stubs: bool,
    crate_mode: bool,
) -> (String, StubCollector) {
    let Some((arena, root)) = parse::create_compilation_unit(java) else {
        // PartParser would throw ParseException; JavaConverter.convert returns
        // e.toString(). We approximate by emitting nothing for now.
        return (String::new(), StubCollector::default());
    };

    let mut id_tracker = id_tracker::IdTracker::new();
    id_tracker::run(&arena, root, &mut id_tracker);
    type_tracker::run(&arena, root, &mut id_tracker);

    let nullable = nullability::analyze(&arena, root, &id_tracker);
    let elem_nullable = nullability::array_elem_nullable(&arena, root, &id_tracker);

    let mut dumper =
        dump::RustDumpVisitor::new(true, &arena, &mut id_tracker, &nullable, link);
    dumper.set_elem_nullable(&elem_nullable);
    dumper.set_stub_collection(emit_stubs, known_types);
    dumper.set_crate_mode(crate_mode);
    dumper.visit(root, None);
    let src = dumper.get_source();
    (src, dumper.take_stubs())
}
