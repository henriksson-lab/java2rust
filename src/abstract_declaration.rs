//! Closed-program admission for stateless abstract declaration owners.
//!
//! These declarations supply inherited bodies and static helpers, not concrete
//! Java objects. Keep the proof separate from emission so every consumer makes
//! the same decision, and recompute it when the translated program grows.
use std::collections::{HashMap, HashSet};

use crate::java_ir::{
    Block, Expr, JavaType, LambdaBody, Member, Modifier, Program, Stmt, TypeDecl, TypeKind,
    WildcardBound,
};

pub(crate) fn declaration_only_abstract_owners(program: &Program) -> HashSet<String> {
    fn types<'a>(ty: &'a TypeDecl, out: &mut Vec<&'a TypeDecl>, captured: &mut HashSet<String>) {
        out.push(ty);
        for member in &ty.members {
            if let Member::Type(nested) = member {
                if nested.has_enclosing_instance_in(ty.kind) {
                    captured.insert(nested.name.clone());
                }
                types(nested, out, captured);
            }
        }
    }
    let mut declarations = Vec::new();
    let mut captured = HashSet::new();
    for ty in &program.types {
        types(ty, &mut declarations, &mut captured);
    }
    let candidates: HashSet<_> = declarations
        .iter()
        .filter(|ty| {
            ty.kind == TypeKind::Class
                && ty.modifiers.contains(&Modifier::Abstract)
                && !captured.contains(&ty.name)
                && ty.members.iter().all(|member| match member {
                    Member::Method(_) => true,
                    Member::Field(field) => field.modifiers.contains(&Modifier::Static),
                    // Constructors, captured nested classes and unsupported members
                    // need a stronger layout/initialization proof than this tier.
                    _ => false,
                })
        })
        .map(|ty| ty.name.clone())
        .collect();
    if candidates.is_empty() {
        return candidates;
    }

    let mut uses = ValueTypeUses::default();
    for ty in &program.types {
        uses.declaration(ty);
    }
    crate::semantic_analysis::walk_program_exprs(program, &mut |expr| uses.expression(expr));
    if uses.opaque {
        return HashSet::new();
    }
    let mut eligible = candidates;
    eligible.retain(|name| !uses.names.contains(name) && !uses.names.contains(leaf(name)));

    // Resolve parent identities once; no repeated per-candidate AST scans.
    let parents: HashMap<_, _> = declarations
        .iter()
        .filter_map(|ty| {
            let reference = ty.extends.as_deref()?;
            let parsed = crate::parser::parse_hierarchy_type(reference);
            let (JavaType::Named { name, .. } | JavaType::Unknown(name)) = parsed else {
                return None;
            };
            let parent = crate::inherited_members::find_type_decl_for_owner_context(
                program, &ty.name, &name,
            )
            .map(|parent| parent.name.clone())
            .unwrap_or(name);
            Some((ty.name.clone(), parent))
        })
        .collect();
    let mut materialized = HashSet::new();
    for ty in &declarations {
        if ty.kind != TypeKind::Class
            || ty.modifiers.contains(&Modifier::Abstract)
            || !ty.members.iter().any(|member| match member {
                Member::Method(method) => method.body.is_some(),
                Member::Constructor(ctor) => ctor.body.is_some(),
                _ => false,
            })
        {
            continue;
        }
        let mut current = ty.name.as_str();
        let mut seen = HashSet::new();
        while let Some(parent) = parents.get(current) {
            if !seen.insert(parent) {
                break;
            }
            materialized.insert(parent.clone());
            current = parent;
        }
    }
    eligible
        .iter()
        .filter(|name| {
            if !materialized.contains(*name) {
                return false;
            }
            let mut current = name.as_str();
            let mut seen = HashSet::new();
            while let Some(parent) = parents.get(current) {
                if matches!(parent.as_str(), "Object" | "java.lang.Object") {
                    return true;
                }
                if !seen.insert(parent) || !eligible.contains(parent) {
                    return false;
                }
                current = parent;
            }
            true
        })
        .cloned()
        .collect()
}

fn leaf(name: &str) -> &str {
    name.rsplit(['.', '$']).next().unwrap_or(name)
}

/// Stateful abstract declarations can retain construction support without
/// promising an impossible raw virtual receiver. This is deliberately a
/// separate admission tier from the stateless declaration-only proof above.
/// All ordinary instance bodies still materialize on concrete descendants.
pub(crate) fn construction_only_abstract_owners(program: &Program) -> HashSet<String> {
    fn collect<'a>(ty: &'a TypeDecl, out: &mut Vec<&'a TypeDecl>, captured: &mut HashSet<String>) {
        out.push(ty);
        for member in &ty.members {
            if let Member::Type(nested) = member {
                if nested.has_enclosing_instance_in(ty.kind) {
                    // Neither an inner root nor an outer root with captured
                    // children is admitted by this construction proof.
                    captured.insert(ty.name.clone());
                    captured.insert(nested.name.clone());
                }
                collect(nested, out, captured);
            }
        }
    }
    let mut declarations = Vec::new();
    let mut captured = HashSet::new();
    for ty in &program.types {
        collect(ty, &mut declarations, &mut captured);
    }
    let mut uses = ValueTypeUses::default();
    for ty in &program.types {
        uses.declaration(ty);
    }
    crate::semantic_analysis::walk_program_exprs(program, &mut |expr| uses.expression(expr));
    if uses.opaque {
        return HashSet::new();
    }
    let parent = |ty: &TypeDecl| {
        let reference = ty.extends.as_deref()?;
        let JavaType::Named { name, .. } = crate::parser::parse_hierarchy_type(reference) else {
            return None;
        };
        crate::inherited_members::find_type_decl_for_owner_context(program, &ty.name, &name)
    };
    let mut materialized = HashSet::new();
    for ty in &declarations {
        if ty.kind != TypeKind::Class || ty.modifiers.contains(&Modifier::Abstract) {
            continue;
        }
        let mut current = *ty;
        let mut seen = HashSet::new();
        while let Some(base) = parent(current) {
            if !seen.insert(base.name.as_str()) {
                break;
            }
            materialized.insert(base.name.clone());
            current = base;
        }
    }
    declarations
        .into_iter()
        .filter(|ty| {
            if ty.kind != TypeKind::Class
                || !ty.modifiers.contains(&Modifier::Abstract)
                || !ty.type_params.is_empty()
                || captured.contains(&ty.name)
                || uses.names.contains(&ty.name)
                || uses.names.contains(leaf(&ty.name))
                || !materialized.contains(&ty.name)
            {
                return false;
            }
            // This tier does not yet compose ancestor initialization effects.
            // Require a fully known, constructor-free, field-free parent
            // chain. Unknown external ancestors are not silently assumed safe.
            let mut current = *ty;
            let mut seen = HashSet::new();
            while let Some(reference) = current.extends.as_deref() {
                if matches!(reference, "Object" | "java.lang.Object") {
                    break;
                }
                let Some(base) = parent(current) else {
                    return false;
                };
                if !seen.insert(base.name.as_str())
                    || captured.contains(&base.name)
                    || base.members.iter().any(|member| match member {
                        Member::Constructor(_) | Member::Unsupported { .. } => true,
                        Member::Field(field) => !field.modifiers.contains(&Modifier::Static),
                        _ => false,
                    })
                {
                    return false;
                }
                current = base;
            }
            ty.members.iter().all(|member| match member {
                Member::Constructor(ctor) => ctor
                    .body
                    .as_ref()
                    .is_some_and(|body| construction_block_is_statically_bound(body, ty)),
                Member::Field(field) if !field.modifiers.contains(&Modifier::Static) => {
                    field.initializer.as_ref().is_none_or(|expr| {
                        construction_exprs_are_statically_bound(ty, |visit| {
                            crate::semantic_analysis::walk_expr(expr, &mut |expr| visit(expr))
                        })
                    })
                }
                Member::Method(method) if construction_support_method(method) => method
                    .body
                    .as_ref()
                    .is_some_and(|body| construction_block_is_statically_bound(body, ty)),
                Member::Unsupported { .. } => false,
                _ => true,
            })
        })
        .map(|ty| ty.name.clone())
        .collect()
}

pub(crate) fn construction_support_method(method: &crate::java_ir::MethodDecl) -> bool {
    !method.modifiers.contains(&Modifier::Static)
        && (method.modifiers.contains(&Modifier::Private)
            || method.modifiers.contains(&Modifier::Final))
}

fn construction_block_is_statically_bound(body: &Block, owner: &TypeDecl) -> bool {
    let mut supported = true;
    crate::semantic_analysis::walk_block_stmts(body, &mut |stmt| {
        if matches!(stmt, Stmt::LocalType(_) | Stmt::Unsupported(_)) {
            supported = false;
        }
    });
    supported
        && construction_exprs_are_statically_bound(owner, |visit| {
            crate::semantic_analysis::walk_block_exprs(body, &mut |expr| visit(expr))
        })
}

fn construction_exprs_are_statically_bound(
    owner: &TypeDecl,
    walk: impl FnOnce(&mut dyn FnMut(&Expr)),
) -> bool {
    let mut supported = true;
    let mut this_values = 0usize;
    let mut bound_this_receivers = 0usize;
    walk(&mut |expr| match expr {
        Expr::This => this_values += 1,
        Expr::FieldAccess(receiver, _) if matches!(receiver.as_ref(), Expr::This) => {
            bound_this_receivers += 1;
        }
        Expr::Call {
            receiver,
            name,
            args,
            ..
        } if receiver
            .as_deref()
            .is_none_or(|expr| matches!(expr, Expr::This)) =>
        {
            let mut methods = owner.members.iter().filter_map(|member| match member {
                Member::Method(method)
                    if method.name == *name && method.params.len() == args.len() =>
                {
                    Some(method)
                }
                _ => None,
            });
            // Overload ambiguity must not be resolved by name alone. Reject
            // it here; normal call resolution still owns the emitted ABI.
            let method = methods.next();
            if methods.next().is_some()
                || !method.is_some_and(|method| {
                    method.modifiers.contains(&Modifier::Static)
                        || construction_support_method(method)
                })
            {
                supported = false;
            }
            if receiver.is_some() {
                bound_this_receivers += 1;
            }
        }
        Expr::Super
        | Expr::Lambda { .. }
        | Expr::MethodReference { .. }
        | Expr::Raw(_)
        | Expr::Unsupported { .. } => supported = false,
        Expr::New { body: Some(_), .. } => supported = false,
        _ => {}
    });
    // Every explicit this must be a field or statically bound call receiver,
    // never an argument, return value, alias, capture or stored self value.
    supported && this_values == bound_this_receivers
}

#[derive(Default)]
struct ValueTypeUses {
    names: HashSet<String>,
    opaque: bool,
}
impl ValueTypeUses {
    fn ty(&mut self, ty: &JavaType) {
        match ty {
            JavaType::Named { name, args } => {
                self.names.insert(name.clone());
                self.names.insert(leaf(name).to_string());
                for arg in args {
                    self.ty(arg);
                }
            }
            JavaType::Unknown(name) => {
                self.names.insert(name.clone());
                self.names.insert(leaf(name).to_string());
            }
            JavaType::Array(element) => self.ty(element),
            JavaType::Intersection(bounds) => {
                for bound in bounds {
                    self.ty(bound);
                }
            }
            JavaType::Wildcard(Some(
                WildcardBound::Extends(bound) | WildcardBound::Super(bound),
            )) => self.ty(bound),
            _ => {}
        }
    }
    fn declaration(&mut self, ty: &TypeDecl) {
        for bound in ty.type_param_bounds.iter().flatten() {
            self.ty(bound);
        }
        // Inheritance itself is not a value use, but an edge's type arguments
        // can demand values of another abstract class.
        for reference in ty.extends.iter().chain(&ty.implements) {
            if let JavaType::Named { args, .. } = crate::parser::parse_hierarchy_type(reference) {
                for arg in &args {
                    self.ty(arg);
                }
            }
        }
        for member in &ty.members {
            self.member(member);
        }
        for block in &ty.static_init_blocks {
            self.block(&block.block);
        }
    }
    fn member(&mut self, member: &Member) {
        match member {
            Member::Field(field) => self.ty(&field.ty),
            Member::Method(method) => {
                self.ty(&method.return_type);
                for param in &method.params {
                    self.ty(&param.ty);
                }
                for bound in method
                    .type_param_bounds
                    .iter()
                    .flatten()
                    .chain(&method.throws)
                {
                    self.ty(bound);
                }
                if let Some(body) = &method.body {
                    self.block(body);
                }
            }
            Member::Constructor(ctor) => {
                for param in &ctor.params {
                    self.ty(&param.ty);
                }
                for bound in ctor.type_param_bounds.iter().flatten().chain(&ctor.throws) {
                    self.ty(bound);
                }
                if let Some(body) = &ctor.body {
                    self.block(body);
                }
            }
            Member::Type(ty) => self.declaration(ty),
            Member::Unsupported { .. } => self.opaque = true,
        }
    }
    fn block(&mut self, block: &Block) {
        crate::semantic_analysis::walk_block_stmts(block, &mut |stmt| match stmt {
            Stmt::VarDecl { ty, .. } => self.ty(ty),
            Stmt::EnhancedFor { var_type, .. } => self.ty(var_type),
            Stmt::Try { catches, .. } => {
                for catch in catches {
                    for ty in &catch.types {
                        self.ty(ty);
                    }
                }
            }
            Stmt::LocalType(ty) => {
                // Local/anonymous subclasses do not participate in the named
                // concrete materialization inventory used by this proof.
                if let Some(parent) = &ty.extends {
                    self.ty(&crate::parser::parse_hierarchy_type(parent));
                }
                self.declaration(ty);
            }
            Stmt::Unsupported(_) => self.opaque = true,
            _ => {}
        });
    }
    fn expression(&mut self, expr: &Expr) {
        match expr {
            Expr::New { ty, body, .. } => {
                self.ty(ty);
                if let Some(members) = body {
                    for member in members {
                        self.member(member);
                    }
                }
            }
            Expr::Cast { ty, .. } | Expr::InstanceOf { ty, .. } | Expr::ClassLiteral(ty) => {
                self.ty(ty)
            }
            Expr::ArrayCreation { element_type, .. } => self.ty(element_type),
            Expr::Call {
                type_args,
                resolved,
                ..
            } => {
                for arg in type_args {
                    self.ty(arg);
                }
                if let Some(call) = resolved {
                    self.ty(&call.return_type);
                }
            }
            Expr::Lambda { params, body } => {
                for param in params {
                    if let Some(ty) = &param.ty {
                        self.ty(ty);
                    }
                }
                if let LambdaBody::Block(block) = body {
                    self.block(block);
                }
            }
            Expr::Var(name) | Expr::MethodReference { target: name, .. } => {
                self.names.insert(name.clone());
                self.names.insert(leaf(name).to_string());
            }
            Expr::Raw(raw) => {
                for token in
                    raw.split(|c: char| !(c.is_alphanumeric() || matches!(c, '_' | '$' | '.')))
                {
                    self.names.insert(token.to_string());
                    self.names.insert(leaf(token).to_string());
                }
            }
            Expr::Unsupported { .. } => self.opaque = true,
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::parse_java_source;

    fn admits_construction(base_members: &str, extra: &str) -> bool {
        let parsed = parse_java_source(&format!(
            "abstract class Base {{ int state; {base_members} abstract int next(); }}
             class Leaf extends Base {{ Leaf() {{ super(7); }} int next() {{ return state; }} }}
             class Uses {{ {extra} }}"
        ));
        assert!(!parsed.diagnostics.has_errors(), "{:?}", parsed.diagnostics);
        construction_only_abstract_owners(&parsed.program).contains("Base")
    }

    #[test]
    fn construction_support_retains_state_private_helpers_and_static_nested_types() {
        assert!(admits_construction(
            "Base(int seed) { this.state = seed; initialize(seed); }
             private void initialize(int value) { this.state = value + 1; }
             static class Table { int value; }",
            "static int run() { return new Leaf().next(); }"
        ));
    }

    #[test]
    fn construction_support_rejects_virtual_calls_and_self_escape() {
        for body in [
            "next();",
            "this.next();",
            "initialize();",
            "Object alias = this;",
            "sink(this);",
            "this.state = this.hashCode();",
            "Runnable r = () -> next();",
            "java.util.function.IntSupplier r = this::next;",
        ] {
            let helper = if body == "initialize();" {
                "private void initialize() { next(); }"
            } else {
                ""
            };
            assert!(
                !admits_construction(
                    &format!(
                        "Base(int seed) {{ {body} }}
                          {helper}
                          static void sink(Object value) {{}}"
                    ),
                    ""
                ),
                "{body}"
            );
        }
    }

    #[test]
    fn construction_support_rejects_runtime_base_values() {
        for extra in [
            "Base value;",
            "static Base get() { return new Leaf(); }",
            "static void accept(Base value) {}",
            "static void run() { Base value = new Leaf(); }",
            "java.util.List<Base> values;",
            "static Object type() { return Base.class; }",
        ] {
            assert!(
                !admits_construction("Base(int seed) { state = seed; }", extra),
                "{extra}"
            );
        }
    }

    #[test]
    fn construction_support_requires_closed_ancestor_initialization() {
        for parent in [
            "class Parent { int value; }",
            "class Parent { Parent() { next(); } int next() { return 0; } }",
        ] {
            let parsed = parse_java_source(&format!(
                "{parent} abstract class Base extends Parent {{ int state; Base(int x) {{ state = x; }} abstract int next(); }}
                 class Leaf extends Base {{ Leaf() {{ super(7); }} int next() {{ return state; }} }}"
            ));
            assert!(!construction_only_abstract_owners(&parsed.program).contains("Base"));
        }
    }

    fn admits(extra: &str) -> bool {
        let parsed = parse_java_source(&format!(
            r#"
abstract class Base {{ abstract int next(); int twice() {{ return next() * 2; }} }}
class Leaf extends Base {{ int next() {{ return 7; }} }}
class Uses {{ {extra} }}
"#
        ));
        assert!(!parsed.diagnostics.has_errors(), "{:?}", parsed.diagnostics);
        declaration_only_abstract_owners(&parsed.program).contains("Base")
    }

    #[test]
    fn stateless_abstract_bodies_have_a_concrete_materialization_owner() {
        assert!(admits("static int run() { return new Leaf().twice(); }"));
        let parsed = parse_java_source("abstract class Base { abstract int next(); }");
        assert!(declaration_only_abstract_owners(&parsed.program).is_empty());
    }

    #[test]
    fn typed_base_values_disable_declaration_only_emission() {
        for extra in [
            "Base value;",
            "static Base get() { return null; }",
            "static void accept(Base value) {}",
            "static void run() { Base value = null; }",
            "static void run() { Base[] values = new Base[0]; }",
            "static void run(Object value) { Base other = (Base) value; }",
            "static boolean run(Object value) { return value instanceof Base; }",
            "static <T extends Base> void accept(T value) {}",
            "java.util.List<Base> values;",
            "static void run() { java.util.function.Consumer<Base> f = (Base value) -> {}; }",
            "static void run() { class Local extends Base { int next() { return 1; } } }",
            "static void run() { new Base() { int next() { return 1; } }; }",
        ] {
            assert!(!admits(extra), "value demand was lost: {extra}");
        }
    }

    #[test]
    fn state_and_constructor_layouts_are_not_pruned() {
        for base in [
            "abstract class Base { int state; abstract int next(); }",
            "abstract class Base { Base() {} abstract int next(); }",
            "abstract class Base { class Inner {} abstract int next(); }",
            "class Parent { int state; } abstract class Base extends Parent { abstract int next(); }",
        ] {
            let parsed = parse_java_source(&format!(
                "{base} class Leaf extends Base {{ int next() {{ return 1; }} }}"
            ));
            assert!(
                !declaration_only_abstract_owners(&parsed.program).contains("Base"),
                "{base}"
            );
        }
    }

    #[test]
    fn enclosing_instance_layouts_are_not_declaration_only() {
        let parsed = parse_java_source(
            "class Outer { int value; abstract class Base { abstract int next(); } class Leaf extends Base { int next() { return value; } } }",
        );
        assert!(!parsed.diagnostics.has_errors());
        assert!(declaration_only_abstract_owners(&parsed.program).is_empty());
    }
}
