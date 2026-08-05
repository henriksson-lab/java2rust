//! Non-static inner classes whose construction cannot be lowered.
//!
//! A non-static inner class captures its enclosing instance by reference. The
//! *access* side of that works: the generated struct carries
//! `__outer: Rc<RefCell<Outer>>` and reads through it, which gives Java's
//! semantics. The *construction* side cannot: a method of `Outer` holds
//! `&mut Outer`, and there is no way to obtain an `Rc<RefCell<Outer>>` from it
//! while `Outer` is represented by value.
//!
//! So the constructor is emitted as a `panic!`. The program compiles and aborts
//! on first use — invisible to a compile-error count, which is exactly the
//! failure mode [0018](../claude_audits/0018-the-compile-error-metric-cannot-see-silent-failures.md)
//! is about, and what
//! [0023](../claude_audits/0023-inner-class-construction-always-panics.md) asks
//! be reported by the *translator* rather than left as a runtime surprise.
//!
//! This module does not repair anything: the repair is the shared
//! representation in [0002b](../claude_audits/0002b-shared-representation-design.md),
//! and 0023 is its clearest concrete consumer. This makes the cost visible in
//! the meantime.

use crate::diagnostics::DiagnosticBag;
use crate::facts::FactSet;
use crate::java_ir::{Member, Program, TypeDecl};

/// A non-static inner class whose constructor cannot produce its outer handle.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnconstructableInnerClass {
    /// The inner class, as written in Java.
    pub inner_type: String,
    /// The enclosing type it captures.
    pub outer_type: String,
}

/// Every inner class in `program` whose construction would panic.
pub fn unconstructable_inner_classes(
    program: &Program,
    facts: &FactSet,
) -> Vec<UnconstructableInnerClass> {
    let mut found = Vec::new();
    for ty in &program.types {
        collect_in_type(ty, facts, &mut found);
    }
    found
}

/// Reports each as `inner_class_construction_panics`.
///
/// A warning, not an error: the output is what it always was, and refusing to
/// emit would hide the rest of a file behind a feature gap. Naming it is what
/// stops a corpus reporting zero problems for a program guaranteed to abort.
pub fn report_unconstructable_inner_classes(
    program: &Program,
    facts: &FactSet,
    into: &mut DiagnosticBag,
) {
    for inner in unconstructable_inner_classes(program, facts) {
        into.warning(
            "inner_class_construction_panics",
            format!(
                "{} captures an enclosing {} instance, but its constructor cannot obtain a \
                 shared handle to it, so `new {}(…)` panics at runtime. Needs the shared \
                 object representation (0002b); the generated code compiles either way.",
                inner.inner_type, inner.outer_type, inner.inner_type,
            ),
        );
    }
}

fn collect_in_type(ty: &TypeDecl, facts: &FactSet, found: &mut Vec<UnconstructableInnerClass>) {
    if let Some(outer_type) = facts.captures.outer_this_type_for_type(&ty.name) {
        found.push(UnconstructableInnerClass {
            inner_type: ty.name.clone(),
            outer_type: outer_type.to_string(),
        });
    }
    for member in &ty.members {
        if let Member::Type(nested) = member {
            collect_in_type(nested, facts, found);
        }
    }
}
