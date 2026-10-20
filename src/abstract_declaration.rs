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
