//! Runtime class identity and ancestry projected from the existing nominal facts.
use crate::{
    facts::FactSet,
    java_ir::{Expr, JavaType, Member, Modifier, Program, TypeDecl, TypeKind},
};
use std::collections::{BTreeMap, BTreeSet};

pub(crate) fn support(programs: &[&Program], facts: &FactSet) -> String {
    fn visit<'a>(
        ty: &'a TypeDecl,
        program: &'a Program,
        out: &mut Vec<(&'a TypeDecl, &'a Program)>,
    ) {
        out.push((ty, program));
        for member in &ty.members {
            if let Member::Type(nested) = member {
                visit(nested, program, out);
            }
        }
    }
    let mut declarations = Vec::new();
    for program in programs {
        for ty in &program.types {
            visit(ty, program, &mut declarations);
        }
    }
    let mut aliases = BTreeMap::<String, String>::new();
    for name in [
        "Object",
        "String",
        "Number",
        "CharSequence",
        "Cloneable",
        "Comparable",
        "Iterable",
        "Enum",
        "Record",
        "Boolean",
        "Byte",
        "Short",
        "Integer",
        "Long",
        "Float",
        "Double",
        "Character",
        "Void",
    ] {
        aliases.insert(name.into(), format!("java.lang.{name}"));
    }
    aliases.insert("Serializable".into(), "java.io.Serializable".into());
    for (name, binary) in crate::stdlib::JDK_EXCEPTION_BINARY_NAMES {
        aliases.insert((*name).into(), (*binary).into());
    }
    for program in programs {
        for import in &program.imports {
            if !import.is_static && !import.is_wildcard {
                aliases.insert(
                    import
                        .path
                        .rsplit('.')
                        .next()
                        .unwrap_or(&import.path)
                        .into(),
                    import.path.clone(),
                );
            }
        }
    }
    // Original canonical/binary names are authoritative; ambiguous simple
    // aliases cannot select an arbitrary peer from another compilation unit.
    let mut source_aliases = BTreeMap::<String, BTreeSet<String>>::new();
    for (ty, program) in &declarations {
        let binary = crate::java_naming::source_binary_class_name(ty, program.package.as_deref());
        let mut names = vec![ty.name.clone(), binary.clone()];
        if let Some(original) = &ty.source_names {
            names.push(original.canonical.clone());
        }
        for name in names {
            source_aliases
                .entry(name)
                .or_default()
                .insert(binary.clone());
        }
    }
    for (alias, names) in source_aliases {
        if names.len() == 1 {
            aliases.insert(alias, names.into_iter().next().unwrap());
        } else {
            aliases.remove(&alias);
        }
    }
    let canonical = |name: &str| aliases.get(name).cloned().unwrap_or_else(|| name.into());
    // The same source edges feed native boundary discovery and final metadata;
    // imports and observed class literals are not a complete dependency list.
    let declarations: Vec<_> = declarations
        .into_iter()
        .map(|(ty, program)| {
            let actual = JavaType::Named {
                name: ty.name.clone(),
                args: vec![],
            };
            let edges = facts.reachable_hierarchy_edges_for_type(&actual);
            (ty, program, actual, edges)
        })
        .collect();
    let mut native = BTreeSet::from([
        "java.lang.Object".to_string(),
        "java.lang.Cloneable".into(),
        "java.io.Serializable".into(),
    ]);
    native.extend(
        aliases
            .values()
            .filter(|name| name.starts_with("java.") || name.starts_with("javax."))
            .cloned(),
    );
    fn literal_names(ty: &JavaType, into: &mut BTreeSet<String>) {
        match ty {
            JavaType::Named { name, .. } | JavaType::Unknown(name) => {
                into.insert(name.clone());
            }
            JavaType::String => {
                into.insert("java.lang.String".into());
            }
            JavaType::Array(element) => literal_names(element, into),
            _ => {}
        }
    }
    for program in programs {
        crate::semantic_analysis::walk_program_exprs(program, &mut |expr| {
            if let Expr::ClassLiteral(ty) = expr {
                let mut names = BTreeSet::new();
                literal_names(ty, &mut names);
                native.extend(
                    names
                        .into_iter()
                        .map(|name| canonical(&name))
                        .filter(|name| name.starts_with("java.") || name.starts_with("javax.")),
                );
            }
        });
    }
    for (_, _, _, edges) in &declarations {
        native.extend(
            edges
                .iter()
                .map(|edge| canonical(&edge.super_name))
                .filter(|name| name.starts_with("java.") || name.starts_with("javax.")),
        );
    }
    let mut parents = BTreeMap::<String, BTreeSet<String>>::new();
    // A distinct runtime class cannot extend a proved final target, even
    // when the actual class's ancestry has not been registered. This is not
    // permission to mark an incomplete native ancestry table complete.
    let mut final_classes = BTreeSet::from(["java.lang.String".to_string()]);
    let mut complete = BTreeSet::from([
        "java.lang.Object".to_string(),
        "java.lang.Cloneable".into(),
        "java.io.Serializable".into(),
    ]);
    let mut pending: Vec<_> = native.iter().cloned().collect();
    while let Some(actual) = pending.pop() {
        if let Some(ancestors) = crate::stdlib::native_functional_runtime_parent_names(&actual) {
            complete.insert(actual.clone());
            for parent in ancestors {
                parents
                    .entry(actual.clone())
                    .or_default()
                    .insert(parent.clone());
                if native.insert(parent.clone()) {
                    pending.push(parent);
                }
            }
        }
    }
    for actual in &native {
        for target in &native {
            if actual != target && crate::stdlib::is_known_java_platform_subtype(actual, target) {
                parents
                    .entry(actual.clone())
                    .or_default()
                    .insert(target.clone());
            }
        }
    }
    let mut enum_kinds = BTreeMap::new();
    for name in [
        "java.lang.Object",
        "java.lang.String",
        "java.lang.Enum",
        "java.lang.Class",
        "java.lang.Number",
        "java.lang.Record",
        "java.lang.Boolean",
        "java.lang.Byte",
        "java.lang.Short",
        "java.lang.Integer",
        "java.lang.Long",
        "java.lang.Float",
        "java.lang.Double",
        "java.lang.Character",
        "java.lang.Void",
    ] {
        enum_kinds.insert(name.to_string(), false);
    }
    for (ty, program, actual, edges) in declarations {
        if ty.modifiers.contains(&Modifier::Final) || ty.kind == TypeKind::Record {
            final_classes.insert(crate::java_naming::source_binary_class_name(
                ty,
                program.package.as_deref(),
            ));
        }
        complete.insert(crate::java_naming::source_binary_class_name(
            ty,
            program.package.as_deref(),
        ));
        let binary = crate::java_naming::source_binary_class_name(ty, program.package.as_deref());
        enum_kinds.insert(binary.clone(), ty.kind == TypeKind::Enum);
        if ty.kind == TypeKind::Enum {
            let mut anonymous = 0;
            for field in crate::constructor_plan::enum_constant_fields(ty) {
                if matches!(&field.initializer, Some(Expr::New { body: Some(_), .. })) {
                    anonymous += 1;
                    let subclass = format!("{binary}${anonymous}");
                    enum_kinds.insert(subclass.clone(), false);
                    parents
                        .entry(subclass.clone())
                        .or_default()
                        .insert(binary.clone());
                    complete.insert(subclass.clone());
                    final_classes.insert(subclass);
                }
            }
        }
        let ancestry = parents.entry(binary).or_default();
        for edge in edges {
            ancestry.insert(canonical(&edge.super_name));
        }
        match ty.kind {
            TypeKind::Enum => {
                ancestry.insert("java.lang.Enum".into());
            }
            TypeKind::Record => {
                ancestry.insert("java.lang.Record".into());
            }
            _ => {}
        }
        // Platform boundaries use the existing nominal authority, not a new
        // table of source inheritance answers or an all-source pair scan.
        for target in &native {
            if facts
                .hierarchy_subtype_policy(
                    &actual,
                    &JavaType::Named {
                        name: target.clone(),
                        args: vec![],
                    },
                )
                .reaches()
            {
                ancestry.insert(target.clone());
            }
        }
    }
    let alias_arms = aliases
        .into_iter()
        .map(|(alias, name)| format!("        {alias:?} => {name:?},"))
        .collect::<Vec<_>>()
        .join("\n");
    let parent_arms = parents
        .into_iter()
        .map(|(name, values)| {
            format!(
                "        {name:?} => &[{}],",
                values
                    .into_iter()
                    .map(|v| format!("{v:?}"))
                    .collect::<Vec<_>>()
                    .join(",")
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    let complete_arms = complete
        .into_iter()
        .map(|name| format!("        {name:?} => true,"))
        .collect::<Vec<_>>()
        .join("\n");
    let runtime = include_str!("runtime_class_support.inc.rs");
    let enum_arms = enum_kinds
        .into_iter()
        .map(|(name, is_enum)| format!("        {name:?} => Some({is_enum}),"))
        .collect::<Vec<_>>()
        .join("\n");
    let final_arms = final_classes
        .into_iter()
        .map(|name| format!("        {name:?} => true,"))
        .collect::<Vec<_>>()
        .join("\n");
    runtime
        .replace("/* CLASS_ALIASES */", &alias_arms)
        .replace("/* CLASS_PARENTS */", &parent_arms)
        .replace("/* CLASS_COMPLETE */", &complete_arms)
        .replace("/* CLASS_FINAL */", &final_arms)
        .replace("/* CLASS_ENUM_KIND */", &enum_arms)
}
