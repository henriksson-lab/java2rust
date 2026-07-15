use new_java2rust::facts::{FactProvenance, UnresolvedTargetKind};
use new_java2rust::java_ir::{CallKind, ConstructorInvocationKind, Expr, JavaType, Member, Stmt};
use new_java2rust::parser::parse_java_source;
use new_java2rust::resolve::{resolve_program, resolve_program_with_source_program};
use new_java2rust::resolver_benchmark::{
    benchmark_java_resolver_dir, render_resolver_benchmark_report,
};

#[test]
fn resolve_annotates_same_class_overload_call_targets() {
    let source =
        std::fs::read_to_string("examples/java/03-methods/OverloadSameArity.java").unwrap();
    let parsed = parse_java_source(&source);
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    assert!(
        !resolved.diagnostics.has_errors(),
        "{:?}",
        resolved.diagnostics
    );

    let ty = &resolved.program.types[0];
    let call_all = ty
        .members
        .iter()
        .find_map(|member| match member {
            Member::Method(method) if method.name == "callAll" => method.body.as_ref(),
            _ => None,
        })
        .unwrap();

    let Stmt::Return(Some(expr)) = &call_all.statements[0] else {
        panic!("expected return expression");
    };

    let mut calls = Vec::new();
    collect_calls(expr, &mut calls);
    let names = calls
        .iter()
        .filter_map(|expr| match expr {
            Expr::Call {
                resolved: Some(call),
                ..
            } => Some((
                call.declaring_type.as_str(),
                call.rust_name.as_str(),
                call.kind,
            )),
            _ => None,
        })
        .collect::<Vec<_>>();

    assert!(names.contains(&("OverloadSameArity", "choose_int", CallKind::Instance)));
    assert!(names.contains(&("OverloadSameArity", "choose_long", CallKind::Instance)));
    assert!(names.contains(&("OverloadSameArity", "choose_string", CallKind::Instance)));
}

#[test]
fn resolve_annotates_same_class_constructor_delegation_targets() {
    let source =
        std::fs::read_to_string("examples/java/03-methods/ConstructorOverloads.java").unwrap();
    let parsed = parse_java_source(&source);
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    assert!(
        !resolved.diagnostics.has_errors(),
        "{:?}",
        resolved.diagnostics
    );

    let ty = &resolved.program.types[0];
    let delegations = ty
        .members
        .iter()
        .filter_map(|member| match member {
            Member::Constructor(constructor) => constructor.body.as_ref(),
            _ => None,
        })
        .flat_map(|body| &body.statements)
        .filter_map(|stmt| match stmt {
            Stmt::ConstructorInvocation {
                kind: ConstructorInvocationKind::This,
                resolved: Some(resolved),
                ..
            } => Some((
                resolved.declaring_type.as_str(),
                resolved.rust_name.as_str(),
            )),
            _ => None,
        })
        .collect::<Vec<_>>();

    assert_eq!(
        delegations,
        vec![
            ("ConstructorOverloads", "new_string_int"),
            ("ConstructorOverloads", "new_string_int")
        ]
    );
}

#[test]
fn resolver_uses_enclosing_static_method_return_type_in_nested_constructor_delegation() {
    let parsed = parse_java_source(
        r#"
class EnclosingFactoryDelegationResolution {
    static class Result {
        double value;
    }

    static Result makeResult() {
        return new Result();
    }

    static class Segment {
        Segment(double value) {}
        Segment(String value) {}

        Segment(int start, int end) {
            this(makeResult().value);
        }
    }
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    assert!(
        !resolved.diagnostics.has_errors(),
        "{:?}",
        resolved.diagnostics
    );

    let nested_segment = resolved.program.types[0]
        .members
        .iter()
        .find_map(|member| match member {
            Member::Type(ty) if ty.name == "Segment" => Some(ty),
            _ => None,
        })
        .unwrap();
    let delegations = nested_segment
        .members
        .iter()
        .filter_map(|member| match member {
            Member::Constructor(constructor) => constructor.body.as_ref(),
            _ => None,
        })
        .flat_map(|body| &body.statements)
        .filter_map(|stmt| match stmt {
            Stmt::ConstructorInvocation {
                kind: ConstructorInvocationKind::This,
                resolved: Some(resolved),
                ..
            } => Some(resolved.rust_name.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>();

    assert_eq!(delegations, vec!["new_double"]);

    let unresolved_fields = resolved
        .facts
        .unresolved_target_facts
        .iter()
        .filter(|fact| fact.kind == UnresolvedTargetKind::FieldAccess)
        .collect::<Vec<_>>();
    assert!(
        unresolved_fields.is_empty(),
        "nested constructor delegation should resolve source result fields: {unresolved_fields:?}"
    );
}

#[test]
fn resolve_annotates_new_expression_constructor_targets() {
    let parsed = parse_java_source(
        r#"
class Boxed {
    Boxed(String value) {}
}

class MakeBoxed {
    Boxed make(String value) {
        return new Boxed(value);
    }
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    assert!(
        !resolved.diagnostics.has_errors(),
        "{:?}",
        resolved.diagnostics
    );

    let maker = &resolved.program.types[1];
    let body = maker
        .members
        .iter()
        .find_map(|member| match member {
            Member::Method(method) => method.body.as_ref(),
            _ => None,
        })
        .unwrap();
    let Stmt::Return(Some(Expr::New {
        resolved: Some(resolved),
        ..
    })) = &body.statements[0]
    else {
        panic!("expected resolved new expression");
    };

    assert_eq!(resolved.declaring_type, "Boxed");
    assert_eq!(resolved.rust_name, "new");
}

#[test]
fn resolver_uses_implicit_no_arg_constructor_for_source_class_without_constructors() {
    let parsed = parse_java_source(
        r#"
class Helper {
}

class MakeHelper {
    Helper make() {
        return new Helper();
    }
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    assert!(
        !resolved.diagnostics.has_errors(),
        "{:?}",
        resolved.diagnostics
    );

    let maker = &resolved.program.types[1];
    let Expr::New {
        resolved: Some(constructor),
        ..
    } = first_return_expr(maker).unwrap()
    else {
        panic!("expected resolved implicit no-arg constructor");
    };

    assert_eq!(constructor.declaring_type, "Helper");
    assert_eq!(constructor.rust_name, "new");
    assert!(
        resolved
            .facts
            .unresolved_target_facts
            .iter()
            .all(|fact| fact.target_name != "Helper")
    );
}

#[test]
fn resolver_uses_implicit_no_arg_constructor_for_source_nested_class() {
    let parsed = parse_java_source(
        r#"
class Owner {
    static class Helper {
    }

    Helper make() {
        return new Helper();
    }
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    assert!(
        !resolved.diagnostics.has_errors(),
        "{:?}",
        resolved.diagnostics
    );

    let owner = &resolved.program.types[0];
    let method = owner
        .members
        .iter()
        .find_map(|member| match member {
            Member::Method(method) if method.name == "make" => Some(method),
            _ => None,
        })
        .unwrap();
    let Expr::New {
        resolved: Some(constructor),
        ..
    } = method
        .body
        .as_ref()
        .and_then(|body| match &body.statements[0] {
            Stmt::Return(Some(expr)) => Some(expr),
            _ => None,
        })
        .unwrap()
    else {
        panic!("expected resolved nested implicit no-arg constructor");
    };

    assert_eq!(constructor.declaring_type, "Helper");
    assert_eq!(constructor.rust_name, "new");
}

#[test]
fn resolver_prefers_lexically_scoped_nested_constructor_when_simple_names_collide() {
    let parsed = parse_java_source(
        r#"
class LeftOwner {
    static class Result {
        Result(int value) {}
    }

    Result make(int value) {
        return new Result(value);
    }
}

class RightOwner {
    static class Result {
        Result(int value) {}
    }

    Result make(int value) {
        return new Result(value);
    }
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    assert!(
        !resolved.diagnostics.has_errors(),
        "{:?}",
        resolved.diagnostics
    );

    for owner in &resolved.program.types {
        let Expr::New {
            resolved: Some(constructor),
            ..
        } = first_return_expr(owner).unwrap()
        else {
            panic!(
                "expected resolved nested Result constructor for {}",
                owner.name
            );
        };
        assert_eq!(constructor.declaring_type, "Result");
        assert_eq!(constructor.rust_name, "new");
    }
}

#[test]
fn resolver_prefers_sibling_nested_constructor_visible_from_nested_class() {
    let parsed = parse_java_source(
        r#"
class OwnerWithNestedSiblings {
    static class Insert {
        Insert(int offset, int length) {}
    }

    static class Element {
        Insert merge(int offset, int length) {
            return new Insert(offset, length);
        }
    }
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    assert!(
        !resolved.diagnostics.has_errors(),
        "{:?}",
        resolved.diagnostics
    );

    let owner = &resolved.program.types[0];
    let element = owner
        .members
        .iter()
        .find_map(|member| match member {
            Member::Type(nested) if nested.name == "Element" => Some(nested),
            _ => None,
        })
        .unwrap();
    let method = element
        .members
        .iter()
        .find_map(|member| match member {
            Member::Method(method) if method.name == "merge" => Some(method),
            _ => None,
        })
        .unwrap();
    let Expr::New {
        resolved: Some(constructor),
        ..
    } = method
        .body
        .as_ref()
        .and_then(|body| match &body.statements[0] {
            Stmt::Return(Some(expr)) => Some(expr),
            _ => None,
        })
        .unwrap()
    else {
        panic!("expected resolved sibling nested constructor");
    };

    assert_eq!(constructor.declaring_type, "Insert");
    assert_eq!(constructor.rust_name, "new");
}

#[test]
fn resolver_matches_generic_sibling_nested_constructor_with_current_receiver_argument() {
    let parsed = parse_java_source(
        r#"
class OuterWithGenericNestedSiblings {
    static class Reader<T> {
        Iterator<T> query(Segment segment) {
            return new OverlapIterator<>(segment, this);
        }
    }

    static class OverlapIterator<T> {
        OverlapIterator(Segment segment, Reader<T> reader) {}
    }
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    assert!(
        !resolved.diagnostics.has_errors(),
        "{:?}",
        resolved.diagnostics
    );

    let outer = &resolved.program.types[0];
    let reader = outer
        .members
        .iter()
        .find_map(|member| match member {
            Member::Type(nested) if nested.name == "Reader" => Some(nested),
            _ => None,
        })
        .unwrap();
    let method = reader
        .members
        .iter()
        .find_map(|member| match member {
            Member::Method(method) if method.name == "query" => Some(method),
            _ => None,
        })
        .unwrap();
    let Expr::New {
        resolved: Some(constructor),
        ..
    } = method
        .body
        .as_ref()
        .and_then(|body| match &body.statements[0] {
            Stmt::Return(Some(expr)) => Some(expr),
            _ => None,
        })
        .unwrap()
    else {
        panic!("expected resolved generic sibling nested constructor");
    };

    assert_eq!(constructor.declaring_type, "OverlapIterator");
    assert_eq!(constructor.rust_name, "new");
}

#[test]
fn resolver_treats_anonymous_interface_creation_as_resolved_constructor_shape() {
    let parsed = parse_java_source(
        r#"
interface Reader {
    int read();
}

class AnonymousInterfaceCreationResolution {
    Reader make() {
        return new Reader() {
            public int read() {
                return 1;
            }
        };
    }
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    assert!(
        !resolved.diagnostics.has_errors(),
        "{:?}",
        resolved.diagnostics
    );

    let Expr::New {
        resolved: Some(constructor),
        ..
    } = first_return_expr(&resolved.program.types[1]).unwrap()
    else {
        panic!("expected resolved anonymous interface creation");
    };

    assert_eq!(constructor.declaring_type, "Reader");
    assert_eq!(constructor.rust_name, "anonymous");
    assert!(
        resolved
            .facts
            .unresolved_target_facts
            .iter()
            .all(|fact| fact.target_name != "Reader")
    );
}

#[test]
fn resolver_records_unresolved_targets_as_facts_before_lowering() {
    let parsed = parse_java_source(
        r#"
class Known {
    Known(int value) {}
}

class UsesUnknownTargets {
    int field;
    void run(External external) {
        missingLocal();
        external.missingMethod(field);
        int x = external.missingField;
        MissingExternal known = new MissingExternal("wrong");
    }
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    assert!(
        !resolved.diagnostics.has_errors(),
        "{:?}",
        resolved.diagnostics
    );

    let facts = &resolved.facts.unresolved_target_facts;
    assert!(facts.iter().any(|fact| {
        fact.kind == UnresolvedTargetKind::MethodCall
            && fact.target_name == "missingLocal"
            && fact.enclosing_type == "UsesUnknownTargets"
            && fact.enclosing_member.as_deref() == Some("run")
            && fact.receiver_type
                == Some(JavaType::Named {
                    name: "UsesUnknownTargets".to_string(),
                    args: Vec::new(),
                })
            && fact.provenance == FactProvenance::ParsedSource
    }));
    assert!(facts.iter().any(|fact| {
        fact.kind == UnresolvedTargetKind::MethodCall
            && fact.target_name == "missingMethod"
            && fact.receiver_type
                == Some(JavaType::Named {
                    name: "External".to_string(),
                    args: Vec::new(),
                })
            && fact.arg_types
                == vec![JavaType::Primitive(
                    new_java2rust::java_ir::PrimitiveType::Int,
                )]
    }));
    assert!(facts.iter().any(|fact| {
        fact.kind == UnresolvedTargetKind::FieldAccess
            && fact.target_name == "missingField"
            && fact.receiver_type
                == Some(JavaType::Named {
                    name: "External".to_string(),
                    args: Vec::new(),
                })
    }));
    assert!(facts.iter().any(|fact| {
        fact.kind == UnresolvedTargetKind::ConstructorCall
            && fact.target_name == "MissingExternal"
            && fact.arg_types == vec![JavaType::String]
    }));
}

#[test]
fn resolver_does_not_record_resolved_source_field_accesses_as_unresolved() {
    let parsed = parse_java_source(
        r#"
class FieldOwner {
    int instanceCount;
    static final int STATIC_COUNT = 7;
}

class ChildOwner extends FieldOwner {
}

class UsesFields {
    int read(FieldOwner owner, ChildOwner child) {
        return owner.instanceCount + FieldOwner.STATIC_COUNT + child.instanceCount;
    }
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    assert!(
        !resolved.diagnostics.has_errors(),
        "{:?}",
        resolved.diagnostics
    );

    let field_facts = resolved
        .facts
        .unresolved_target_facts
        .iter()
        .filter(|fact| fact.kind == UnresolvedTargetKind::FieldAccess)
        .collect::<Vec<_>>();

    assert!(
        field_facts.is_empty(),
        "source fields should resolve before unresolved facts: {field_facts:?}"
    );
}

#[test]
fn resolver_prefers_lexically_scoped_nested_field_when_simple_names_collide() {
    let parsed = parse_java_source(
        r#"
class LeftOwner {
    static class Entry {
        int offset;
    }

    void accept(int value) {}
    void accept(String value) {}

    void read(Entry entry) {
        accept(entry.offset);
    }
}

class RightOwner {
    static class Entry {
        String offset;
    }
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    assert!(
        !resolved.diagnostics.has_errors(),
        "{:?}",
        resolved.diagnostics
    );

    let field_facts = resolved
        .facts
        .unresolved_target_facts
        .iter()
        .filter(|fact| fact.kind == UnresolvedTargetKind::FieldAccess)
        .collect::<Vec<_>>();
    assert!(field_facts.is_empty(), "{field_facts:?}");

    let left = resolved
        .program
        .types
        .iter()
        .find(|ty| ty.name == "LeftOwner")
        .unwrap();
    let Stmt::Expr(Expr::Call {
        resolved: Some(call),
        ..
    }) = first_stmt(left).unwrap()
    else {
        panic!("expected resolved overload from lexically scoped nested field");
    };
    assert_eq!(
        (call.declaring_type.as_str(), call.rust_name.as_str()),
        ("LeftOwner", "accept_int")
    );
}

#[test]
fn resolver_uses_enclosing_source_field_type_for_nested_class_overload_arguments() {
    let parsed = parse_java_source(
        r#"
class RecordTable {
    private static final String KIND_COLUMN = "kind";

    static class Reader {
        void accept(String column) {}
        void accept(int column) {}

        void run() {
            accept(KIND_COLUMN);
        }
    }
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    assert!(
        !resolved.diagnostics.has_errors(),
        "{:?}",
        resolved.diagnostics
    );

    let reader = resolved
        .program
        .types
        .iter()
        .find(|ty| ty.name == "RecordTable")
        .and_then(|ty| {
            ty.members.iter().find_map(|member| match member {
                Member::Type(nested) if nested.name == "Reader" => Some(nested),
                _ => None,
            })
        })
        .unwrap();
    let body = reader
        .members
        .iter()
        .find_map(|member| match member {
            Member::Method(method) if method.name == "run" => method.body.as_ref(),
            _ => None,
        })
        .unwrap();
    let Stmt::Expr(Expr::Call {
        resolved: Some(call),
        ..
    }) = &body.statements[0]
    else {
        panic!("expected resolved nested class overload call from enclosing field type");
    };

    assert_eq!(call.declaring_type, "Reader");
    assert_eq!(call.rust_name, "accept_string");
}

#[test]
fn resolver_treats_enum_constants_and_fields_as_source_members() {
    let parsed = parse_java_source(
        r#"
enum SiteStatus {
    MONO("m"),
    POLY("p");

    public final String label;

    SiteStatus(String label) {
        this.label = label;
    }
}

class UsesEnumMembers {
    String read(SiteStatus status) {
        SiteStatus fixed = SiteStatus.MONO;
        return fixed.label + status.label + SiteStatus.POLY.label;
    }
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    assert!(
        !resolved.diagnostics.has_errors(),
        "{:?}",
        resolved.diagnostics
    );

    let field_facts = resolved
        .facts
        .unresolved_target_facts
        .iter()
        .filter(|fact| fact.kind == UnresolvedTargetKind::FieldAccess)
        .collect::<Vec<_>>();

    assert!(
        field_facts.is_empty(),
        "enum constants and enum fields should resolve as source members: {field_facts:?}"
    );
}

#[test]
fn resolver_treats_nested_type_enum_constants_as_source_members() {
    let parsed = parse_java_source(
        r#"
class OuterRecord {
    enum Compatibility {
        COMPATIBLE,
        NOT_COMPATIBLE
    }

    Compatibility make() {
        return OuterRecord.Compatibility.COMPATIBLE;
    }
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    assert!(
        !resolved.diagnostics.has_errors(),
        "{:?}",
        resolved.diagnostics
    );

    let field_facts = resolved
        .facts
        .unresolved_target_facts
        .iter()
        .filter(|fact| fact.kind == UnresolvedTargetKind::FieldAccess)
        .collect::<Vec<_>>();

    assert!(
        field_facts.is_empty(),
        "nested type enum constants should resolve as source members: {field_facts:?}"
    );
}

#[test]
fn resolver_resolves_fields_on_source_nested_class_instances() {
    let parsed = parse_java_source(
        r#"
class OwnerWithNestedData {
    static final class WalkData {
        int visitNum;
        int minVisitNum;
    }

    void update(WalkData data, WalkData next) {
        data.minVisitNum = next.visitNum;
    }
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    assert!(
        !resolved.diagnostics.has_errors(),
        "{:?}",
        resolved.diagnostics
    );

    let field_facts = resolved
        .facts
        .unresolved_target_facts
        .iter()
        .filter(|fact| fact.kind == UnresolvedTargetKind::FieldAccess)
        .collect::<Vec<_>>();

    assert!(
        field_facts.is_empty(),
        "fields on source nested class instances should resolve: {field_facts:?}"
    );
}

#[test]
fn resolver_resolves_fields_on_later_declared_nested_class_instances() {
    let parsed = parse_java_source(
        r#"
class OwnerWithLateNestedData {
    void update(WalkData data, WalkData next) {
        data.minVisitNum = next.visitNum;
    }

    static final class WalkData {
        int visitNum;
        int minVisitNum;
    }
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    assert!(
        !resolved.diagnostics.has_errors(),
        "{:?}",
        resolved.diagnostics
    );

    let field_facts = resolved
        .facts
        .unresolved_target_facts
        .iter()
        .filter(|fact| fact.kind == UnresolvedTargetKind::FieldAccess)
        .collect::<Vec<_>>();

    assert!(
        field_facts.is_empty(),
        "fields on later-declared nested class instances should resolve: {field_facts:?}"
    );
}

#[test]
fn resolver_prefers_local_receiver_over_snake_case_type_owner_collision() {
    let parsed = parse_java_source(
        r#"
class SnakeCollisionOwner {
    static final class WalkData {
        int minVisitNum;
    }

    void update(WalkData walkData) {
        walkData.minVisitNum = walkData.minVisitNum;
    }
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    assert!(
        !resolved.diagnostics.has_errors(),
        "{:?}",
        resolved.diagnostics
    );

    let field_facts = resolved
        .facts
        .unresolved_target_facts
        .iter()
        .filter(|fact| fact.kind == UnresolvedTargetKind::FieldAccess)
        .collect::<Vec<_>>();

    assert!(
        field_facts.is_empty(),
        "local variable receivers must beat snake_case type-owner matches: {field_facts:?}"
    );
}

#[test]
fn resolver_resolves_fields_on_resolved_call_results() {
    let parsed = parse_java_source(
        r#"
class CallResultFieldAccess {
    static final class WalkData {
        int minVisitNum;
    }

    WalkData next() {
        return new WalkData();
    }

    int read() {
        return next().minVisitNum;
    }
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    assert!(
        !resolved.diagnostics.has_errors(),
        "{:?}",
        resolved.diagnostics
    );

    let field_facts = resolved
        .facts
        .unresolved_target_facts
        .iter()
        .filter(|fact| fact.kind == UnresolvedTargetKind::FieldAccess)
        .collect::<Vec<_>>();

    assert!(
        field_facts.is_empty(),
        "fields on resolved call results should resolve: {field_facts:?}"
    );
}

#[test]
fn resolver_uses_method_local_class_fields_for_overload_arguments() {
    let parsed = parse_java_source(
        r#"
class MethodLocalClassFieldArgumentResolution {
    void accept(int value) {}
    void accept(String value) {}
    void accept(byte value) {}

    void run() {
        final class Row {
            int count;
            String name;
            byte tag;
        }
        Row row = new Row();
        row.count = 1;
        row.name = "x";
        row.tag = (byte) 3;
        accept(row.count);
        accept(row.name);
        accept(row.tag);
    }
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    assert!(
        !resolved.diagnostics.has_errors(),
        "{:?}",
        resolved.diagnostics
    );

    let field_facts = resolved
        .facts
        .unresolved_target_facts
        .iter()
        .filter(|fact| fact.kind == UnresolvedTargetKind::FieldAccess)
        .collect::<Vec<_>>();
    assert!(
        field_facts.is_empty(),
        "method-local class fields should resolve as source fields: {field_facts:?}"
    );

    let body = resolved.program.types[0]
        .members
        .iter()
        .find_map(|member| match member {
            Member::Method(method) if method.name == "run" => method.body.as_ref(),
            _ => None,
        })
        .unwrap();
    let calls = body
        .statements
        .iter()
        .filter_map(|stmt| match stmt {
            Stmt::Expr(Expr::Call {
                resolved: Some(call),
                ..
            }) => Some(call.rust_name.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>();

    assert_eq!(calls, vec!["accept_int", "accept_string", "accept_byte"]);
}

#[test]
fn resolver_treats_qualified_this_as_typed_receiver_not_field_access() {
    let parsed = parse_java_source(
        r#"
class QualifiedOuter {
    Object make() {
        return new Object() {
            String read() {
                return QualifiedOuter.this.outerHook();
            }
        };
    }
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    assert!(
        !resolved.diagnostics.has_errors(),
        "{:?}",
        resolved.diagnostics
    );

    assert!(!resolved.facts.unresolved_target_facts.iter().any(|fact| {
        fact.kind == UnresolvedTargetKind::FieldAccess && fact.target_name == "this"
    }));
    assert!(
        resolved.facts.unresolved_target_facts.iter().any(|fact| {
            fact.kind == UnresolvedTargetKind::MethodCall
                && fact.target_name == "outerHook"
                && fact.receiver_type
                    == Some(JavaType::Named {
                        name: "QualifiedOuter".to_string(),
                        args: Vec::new(),
                    })
        }),
        "qualified this should preserve the named outer receiver type"
    );
}

#[test]
fn resolver_records_super_receiver_unresolved_calls_as_current_type_facts() {
    let parsed = parse_java_source(
        r#"
class Child {
    String label() {
        return super.toString();
    }
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    assert!(
        !resolved.diagnostics.has_errors(),
        "{:?}",
        resolved.diagnostics
    );

    assert!(resolved.facts.unresolved_target_facts.iter().any(|fact| {
        fact.kind == UnresolvedTargetKind::MethodCall
            && fact.target_name == "toString"
            && fact.receiver_type
                == Some(JavaType::Named {
                    name: "Child".to_string(),
                    args: Vec::new(),
                })
    }));
}

#[test]
fn resolver_uses_source_implements_for_constructor_overload_assignability() {
    let parsed = parse_java_source(
        r#"
interface Api {
}

class Impl implements Api {
}

class Target {
    Target(String label) {}
    Target(Api value) {}
}

class UseConstructorInterfaceAssignability {
    Target make(Impl value) {
        return new Target(value);
    }
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    assert!(
        !resolved.diagnostics.has_errors(),
        "{:?}",
        resolved.diagnostics
    );

    let Expr::New {
        resolved: Some(constructor),
        ..
    } = first_return_expr(&resolved.program.types[3]).unwrap()
    else {
        panic!("expected resolved constructor call through implemented interface");
    };

    assert_eq!(constructor.declaring_type, "Target");
    assert_eq!(constructor.rust_name, "new_api");
}

#[test]
fn resolver_uses_generic_source_implements_for_overload_assignability() {
    let parsed = parse_java_source(
        r#"
interface Iterable<T> {
}

class Element {
}

class ElementGroup implements Iterable<Element> {
}

class Stream<T> {
}

class StreamFactory {
    <T> Stream<T> stream(Iterable<T> values) {
        return null;
    }

    Stream<String> stream(String value) {
        return null;
    }

    Stream<Element> run(ElementGroup group) {
        return stream(group);
    }
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    assert!(
        !resolved.diagnostics.has_errors(),
        "{:?}",
        resolved.diagnostics
    );

    let run_body = resolved.program.types[4]
        .members
        .iter()
        .find_map(|member| match member {
            Member::Method(method) if method.name == "run" => method.body.as_ref(),
            _ => None,
        })
        .unwrap();
    let Stmt::Return(Some(Expr::Call {
        resolved: Some(call),
        ..
    })) = &run_body.statements[0]
    else {
        panic!("expected resolved generic interface overload");
    };

    assert_eq!(call.declaring_type, "StreamFactory");
    assert_eq!(call.rust_name, "stream_iterable");
}

#[test]
fn resolver_prefers_direct_source_interface_over_indirect_external_superinterface() {
    let parsed = parse_java_source(
        r#"
interface Iterator<T> {
}

interface Iterable<T> {
}

class Element {
}

class ElementIterator implements Iterator<Element> {
}

class Stream<T> {
}

class StreamFactory {
    <T> Stream<T> stream(Iterable<T> values) {
        return null;
    }

    <T> Stream<T> stream(Iterator<T> values) {
        return null;
    }

    Stream<Element> run(ElementIterator iterator) {
        return stream(iterator);
    }
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    assert!(
        !resolved.diagnostics.has_errors(),
        "{:?}",
        resolved.diagnostics
    );

    let run_body = resolved.program.types[5]
        .members
        .iter()
        .find_map(|member| match member {
            Member::Method(method) if method.name == "run" => method.body.as_ref(),
            _ => None,
        })
        .unwrap();
    let Stmt::Return(Some(Expr::Call {
        resolved: Some(call),
        ..
    })) = &run_body.statements[0]
    else {
        panic!("expected resolved direct interface overload");
    };

    assert_eq!(call.declaring_type, "StreamFactory");
    assert_eq!(call.rust_name, "stream_iterator");
}

#[test]
fn resolver_applies_receiver_generic_substitution_before_boxed_primitive_matching() {
    let parsed = parse_java_source(
        r#"
class Box<E> {
    boolean add(E value) {
        return true;
    }
}

class GenericReceiverBoxingResolution {
    void run(Box<Integer> values) {
        values.add(1);
    }
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    assert!(
        !resolved.diagnostics.has_errors(),
        "{:?}",
        resolved.diagnostics
    );

    let body = resolved.program.types[1]
        .members
        .iter()
        .find_map(|member| match member {
            Member::Method(method) if method.name == "run" => method.body.as_ref(),
            _ => None,
        })
        .unwrap();
    let Stmt::Expr(Expr::Call {
        resolved: Some(call),
        ..
    }) = &body.statements[0]
    else {
        panic!("expected resolved generic receiver call");
    };

    assert_eq!(call.declaring_type, "Box");
    assert_eq!(call.rust_name, "add");
}

#[test]
fn resolver_does_not_substitute_method_type_variable_from_generic_receiver() {
    let parsed = parse_java_source(
        r#"
class Box<E> {
    <T> T convert(T value) {
        return value;
    }
}

class GenericReceiverMethodVariableResolution {
    String run(Box<Integer> values, String name) {
        return values.convert(name);
    }
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    assert!(
        !resolved.diagnostics.has_errors(),
        "{:?}",
        resolved.diagnostics
    );

    let body = resolved.program.types[1]
        .members
        .iter()
        .find_map(|member| match member {
            Member::Method(method) if method.name == "run" => method.body.as_ref(),
            _ => None,
        })
        .unwrap();
    let Stmt::Return(Some(Expr::Call {
        resolved: Some(call),
        ..
    })) = &body.statements[0]
    else {
        panic!("expected resolved method-generic receiver call");
    };

    assert_eq!(call.declaring_type, "Box");
    assert_eq!(call.rust_name, "convert");
    assert_eq!(call.return_type, JavaType::String);
}

#[test]
fn resolver_uses_collections_singleton_list_element_type_for_constructor_overloads() {
    let parsed = parse_java_source(
        r#"
class ItemBatch {
    ItemBatch(List<String> names) {}
    ItemBatch(String name) {}
}

class SingletonListConstructorResolution {
    ItemBatch make(String name) {
        return new ItemBatch(Collections.singletonList(name));
    }
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    assert!(
        !resolved.diagnostics.has_errors(),
        "{:?}",
        resolved.diagnostics
    );

    let Expr::New {
        resolved: Some(constructor),
        ..
    } = first_return_expr(&resolved.program.types[1]).unwrap()
    else {
        panic!("expected resolved constructor from singletonList result");
    };

    assert_eq!(constructor.declaring_type, "ItemBatch");
    assert_eq!(constructor.rust_name, "new_list");
}

#[test]
fn resolver_uses_target_type_for_empty_list_constructor_overloads() {
    let parsed = parse_java_source(
        r#"
class Settings {}

class ItemBatch {
    ItemBatch(Settings settings, List<String> names) {}
    ItemBatch(Settings settings, Header header) {}
}

class EmptyListConstructorResolution {
    ItemBatch make(Settings settings) {
        return new ItemBatch(settings, Collections.emptyList());
    }
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    assert!(
        !resolved.diagnostics.has_errors(),
        "{:?}",
        resolved.diagnostics
    );

    let Expr::New {
        resolved: Some(constructor),
        ..
    } = first_return_expr(&resolved.program.types[2]).unwrap()
    else {
        panic!("expected resolved constructor from target-typed emptyList result");
    };

    assert_eq!(constructor.declaring_type, "ItemBatch");
    assert_eq!(constructor.rust_name, "new_settings_list");
}

#[test]
fn resolver_uses_pair_factory_result_type_for_constructor_overloads() {
    let parsed = parse_java_source(
        r#"
class Pair<L, R> {}

class FragmentLike {
    FragmentLike(Item item) {}
    FragmentLike(Pair<Item, Item> pair) {}
}

class PairFactoryConstructorResolution {
    FragmentLike make(Item first, Item second) {
        return new FragmentLike(ImmutablePair.of(first, second));
    }
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    assert!(
        !resolved.diagnostics.has_errors(),
        "{:?}",
        resolved.diagnostics
    );

    let Expr::New {
        resolved: Some(constructor),
        ..
    } = first_return_expr(&resolved.program.types[2]).unwrap()
    else {
        panic!("expected resolved constructor from pair factory result");
    };

    assert_eq!(constructor.declaring_type, "FragmentLike");
    assert_eq!(constructor.rust_name, "new_pair");
}

#[test]
fn resolver_uses_immutable_sorted_map_factory_result_type_for_constructor_overloads() {
    let parsed = parse_java_source(
        r#"
class SimpleInterval {}
class AnnotationMap {}

class AnnotatedRecord {
    AnnotatedRecord(SimpleInterval interval, AnnotationMap annotations) {}
    AnnotatedRecord(SimpleInterval interval, SortedMap<String, String> annotations) {}
}

class ImmutableSortedMapFactoryConstructorResolution {
    AnnotatedRecord make(SimpleInterval interval, String key, String value) {
        return new AnnotatedRecord(interval, ImmutableSortedMap.of(key, value));
    }
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    assert!(
        !resolved.diagnostics.has_errors(),
        "{:?}",
        resolved.diagnostics
    );

    let Expr::New {
        resolved: Some(constructor),
        ..
    } = first_return_expr(&resolved.program.types[3]).unwrap()
    else {
        panic!("expected resolved constructor from immutable sorted map factory result");
    };

    assert_eq!(constructor.declaring_type, "AnnotatedRecord");
    assert_eq!(constructor.rust_name, "new_simple_interval_sorted_map");
}

#[test]
fn resolver_infers_constructor_type_variables_from_nested_generic_arguments() {
    let parsed = parse_java_source(
        r#"
class Evidence {}
class Allele {}

class EvidenceLikelihoods<E, A> {
    EvidenceLikelihoods(SampleList samples, AlleleList<A> alleles, Map<String, List<E>> evidenceBySample) {}
    EvidenceLikelihoods(SampleList samples, AlleleList<A> alleles, String evidenceBySample) {}
}

class ConstructorNestedGenericInferenceResolution {
    EvidenceLikelihoods<Evidence, Allele> make(SampleList samples, AlleleList<Allele> alleles, List<Evidence> reads) {
        return new EvidenceLikelihoods<>(samples, alleles, ImmutableMap.of("SAMPLE", reads));
    }
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    assert!(
        !resolved.diagnostics.has_errors(),
        "{:?}",
        resolved.diagnostics
    );

    let use_case = resolved
        .program
        .types
        .iter()
        .find(|ty| ty.name == "ConstructorNestedGenericInferenceResolution")
        .unwrap();
    let Expr::New {
        resolved: Some(constructor),
        ..
    } = first_return_expr(use_case).unwrap()
    else {
        panic!("expected resolved constructor from nested generic argument inference");
    };

    assert_eq!(constructor.declaring_type, "EvidenceLikelihoods");
    assert_eq!(constructor.rust_name, "new_sample_list_allele_list_map");
}

#[test]
fn resolver_matches_private_style_generic_constructor_with_null_and_multidimensional_array() {
    let parsed = parse_java_source(
        r#"
class Evidence {}
class Allele {}

class EvidenceLikelihoods<E, A> {
    EvidenceLikelihoods(AlleleList<A> alleles, SampleList samples, List<List<E>> evidenceBySample, List<List<E>> filteredEvidenceBySample, double[][][] values) {}
    EvidenceLikelihoods(AlleleList<A> alleles, SampleList samples, String evidenceBySample, List<List<E>> filteredEvidenceBySample, double[][][] values) {}

    EvidenceLikelihoods<Evidence, A> regroup(List<List<Evidence>> grouped, double[][][] values) {
        return new EvidenceLikelihoods<>(alleles(), samples(), grouped, null, values);
    }

    AlleleList<A> alleles() {
        return null;
    }

    SampleList samples() {
        return null;
    }
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    assert!(
        !resolved.diagnostics.has_errors(),
        "{:?}",
        resolved.diagnostics
    );

    let Expr::New {
        resolved: Some(constructor),
        ..
    } = first_return_expr(&resolved.program.types[2]).unwrap()
    else {
        panic!("expected resolved constructor from nested generic list and null argument");
    };

    assert_eq!(constructor.declaring_type, "EvidenceLikelihoods");
    assert_eq!(
        constructor.rust_name,
        "new_allele_list_sample_list_list_list_double_array_array_array"
    );
}

#[test]
fn resolver_ignores_argument_comments_for_generic_constructor_overloads() {
    let parsed = parse_java_source(
        r#"
class Evidence {}
class Allele {}

class EvidenceLikelihoods<E, A> {
    EvidenceLikelihoods(AlleleList<A> alleles, SampleList samples, List<List<E>> evidenceBySample, List<List<E>> filteredEvidenceBySample, double[][][] values) {}
    EvidenceLikelihoods(AlleleList<A> alleles, SampleList samples, String evidenceBySample, List<List<E>> filteredEvidenceBySample, double[][][] values) {}

    EvidenceLikelihoods<Evidence, A> regroup(List<List<Evidence>> grouped, double[][][] values) {
        return new EvidenceLikelihoods<>(
                alleles(),
                samples(),
                grouped,
                null, // intentionally no filtered evidence
                values);
    }

    AlleleList<A> alleles() {
        return null;
    }

    SampleList samples() {
        return null;
    }
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    assert!(
        !resolved.diagnostics.has_errors(),
        "{:?}",
        resolved.diagnostics
    );

    let Expr::New {
        resolved: Some(constructor),
        ..
    } = first_return_expr(&resolved.program.types[2]).unwrap()
    else {
        panic!("expected resolved constructor with commented null argument");
    };

    assert_eq!(constructor.declaring_type, "EvidenceLikelihoods");
    assert_eq!(
        constructor.rust_name,
        "new_allele_list_sample_list_list_list_double_array_array_array"
    );
}

#[test]
fn resolver_uses_source_static_factory_return_type_for_constructor_arguments() {
    let parsed = parse_java_source(
        r#"
interface SampleList {
    static SampleList singletonSampleList(String sampleName) {
        return null;
    }
}

class Evidence {}
class Allele {}

class EvidenceLikelihoods<E, A> {
    EvidenceLikelihoods(SampleList samples, AlleleList<A> alleles, Map<String, List<E>> evidenceBySample) {}
    EvidenceLikelihoods(String samples, AlleleList<A> alleles, Map<String, List<E>> evidenceBySample) {}
}

class SourceStaticFactoryConstructorArgumentResolution {
    Object make(AlleleList<Allele> alleles, List<Evidence> reads) {
        return new EvidenceLikelihoods<>(SampleList.singletonSampleList("SAMPLE"), alleles, ImmutableMap.of("SAMPLE", reads));
    }
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    assert!(
        !resolved.diagnostics.has_errors(),
        "{:?}",
        resolved.diagnostics
    );

    let use_case = resolved
        .program
        .types
        .iter()
        .find(|ty| ty.name == "SourceStaticFactoryConstructorArgumentResolution")
        .unwrap();
    let Expr::New {
        resolved: Some(constructor),
        ..
    } = first_return_expr(use_case).unwrap()
    else {
        panic!("expected resolved constructor from source static factory argument");
    };

    assert_eq!(constructor.declaring_type, "EvidenceLikelihoods");
    assert_eq!(constructor.rust_name, "new_sample_list_allele_list_map");
}

#[test]
fn resolver_uses_arrays_as_list_element_type_for_constructor_overloads() {
    let parsed = parse_java_source(
        r#"
class ItemBatch {
    ItemBatch(List<String> names) {}
    ItemBatch(String name) {}
}

class ArraysAsListConstructorResolution {
    ItemBatch make(String first, String second) {
        return new ItemBatch(Arrays.asList(first, second));
    }
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    assert!(
        !resolved.diagnostics.has_errors(),
        "{:?}",
        resolved.diagnostics
    );

    let Expr::New {
        resolved: Some(constructor),
        ..
    } = first_return_expr(&resolved.program.types[1]).unwrap()
    else {
        panic!("expected resolved constructor from Arrays.asList result");
    };

    assert_eq!(constructor.declaring_type, "ItemBatch");
    assert_eq!(constructor.rust_name, "new_list");
}

#[test]
fn resolver_keeps_primitive_array_as_arrays_as_list_element_type() {
    let parsed = parse_java_source(
        r#"
class ProbabilityBatch {
    ProbabilityBatch(List<double[]> probabilities) {}
    ProbabilityBatch(List<double> probabilities, String label) {}
}

class ArraysAsListPrimitiveArrayResolution {
    ProbabilityBatch make(double[] values) {
        return new ProbabilityBatch(Arrays.asList(values));
    }
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    assert!(
        !resolved.diagnostics.has_errors(),
        "{:?}",
        resolved.diagnostics
    );

    let Expr::New {
        resolved: Some(constructor),
        ..
    } = first_return_expr(&resolved.program.types[1]).unwrap()
    else {
        panic!("expected resolved constructor from primitive array Arrays.asList result");
    };

    assert_eq!(constructor.declaring_type, "ProbabilityBatch");
    assert_eq!(constructor.rust_name, "new_list");
}

#[test]
fn resolver_uses_collection_union_result_type_for_constructor_overloads() {
    let parsed = parse_java_source(
        r#"
class ItemBatch {
    ItemBatch(List<String> names) {}
    ItemBatch(String name) {}
}

class NameSet {
    NameSet(Set<String> names) {}
    NameSet(String name) {}
}

class CollectionUnionConstructorResolution {
    ItemBatch makeList(List<String> left, List<String> right) {
        return new ItemBatch(ListUtils.union(left, right));
    }

    NameSet makeSet(Set<String> left, Set<String> right) {
        return new NameSet(Sets.union(left, right));
    }
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    assert!(
        !resolved.diagnostics.has_errors(),
        "{:?}",
        resolved.diagnostics
    );

    let ty = &resolved.program.types[2];
    let constructors = ty
        .members
        .iter()
        .filter_map(|member| match member {
            Member::Method(method) => method.body.as_ref(),
            _ => None,
        })
        .filter_map(|body| match &body.statements[0] {
            Stmt::Return(Some(Expr::New {
                resolved: Some(constructor),
                ..
            })) => Some(constructor.rust_name.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>();

    assert_eq!(constructors, vec!["new_list", "new_set"]);
}

#[test]
fn resolver_uses_stream_collect_result_type_for_constructor_overloads() {
    let parsed = parse_java_source(
        r#"
class ItemBatch {
    ItemBatch(List<String> names) {}
    ItemBatch(String name) {}
}

class NameSet {
    NameSet(Set<String> names) {}
    NameSet(String name) {}
}

class StreamCollectConstructorResolution {
    ItemBatch makeList(List<String> names) {
        return new ItemBatch(names.stream().collect(Collectors.toList()));
    }

    ItemBatch makeFilteredList(List<String> names) {
        return new ItemBatch(names.stream().filter(name -> !name.isEmpty()).collect(Collectors.toList()));
    }

    NameSet makeSet(Set<String> names) {
        return new NameSet(names.stream().collect(Collectors.toSet()));
    }
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    assert!(
        !resolved.diagnostics.has_errors(),
        "{:?}",
        resolved.diagnostics
    );

    let ty = &resolved.program.types[2];
    let constructors = ty
        .members
        .iter()
        .filter_map(|member| match member {
            Member::Method(method) => method.body.as_ref(),
            _ => None,
        })
        .filter_map(|body| match &body.statements[0] {
            Stmt::Return(Some(Expr::New {
                resolved: Some(constructor),
                ..
            })) => Some(constructor.rust_name.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>();

    assert_eq!(constructors, vec!["new_list", "new_list", "new_set"]);
}

#[test]
fn resolver_uses_joining_collector_string_result_inside_singleton_list() {
    let parsed = parse_java_source(
        r#"
class Value {
    String label() {
        return "";
    }
}

class JoinedValueBatch {
    JoinedValueBatch(List<String> values, int marker) {}
    JoinedValueBatch(List<Integer> values, String marker) {}
}

class JoiningCollectorSingletonListResolution {
    JoinedValueBatch run(List<Value> values) {
        return new JoinedValueBatch(
            Collections.singletonList(values.stream().map(v -> v.label()).collect(Collectors.joining("|"))),
            1);
    }
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    assert!(
        !resolved.diagnostics.has_errors(),
        "{:?}",
        resolved.diagnostics
    );

    let Expr::New {
        resolved: Some(constructor),
        ..
    } = first_return_expr(&resolved.program.types[2]).unwrap()
    else {
        panic!("expected resolved constructor from joining collector result");
    };

    assert_eq!(constructor.declaring_type, "JoinedValueBatch");
    assert_eq!(constructor.rust_name, "new_list_int");
}

#[test]
fn resolver_uses_map_entry_set_stream_before_joining_collector_result() {
    let parsed = parse_java_source(
        r#"
class JoinedEntryBatch {
    JoinedEntryBatch(List<String> values, int marker) {}
    JoinedEntryBatch(List<Integer> values, String marker) {}
}

class MapEntrySetStreamJoiningResolution {
    JoinedEntryBatch run(Map<String, Integer> counts) {
        return new JoinedEntryBatch(
            Collections.singletonList(counts.entrySet().stream()
                .map(entry -> entry.getKey())
                .collect(Collectors.joining("|"))),
            1);
    }
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    assert!(
        !resolved.diagnostics.has_errors(),
        "{:?}",
        resolved.diagnostics
    );

    let Expr::New {
        resolved: Some(constructor),
        ..
    } = first_return_expr(&resolved.program.types[1]).unwrap()
    else {
        panic!("expected resolved constructor from map entry-set stream joining result");
    };

    assert_eq!(constructor.declaring_type, "JoinedEntryBatch");
    assert_eq!(constructor.rust_name, "new_list_int");
}

#[test]
fn resolver_uses_stream_entry_value_type_inside_lambda_constructor_overload() {
    let parsed = parse_java_source(
        r#"
class Key {}
class Evidence {}

class EvidenceBatch {
    EvidenceBatch(Key key, Iterable<Evidence> values) {}
    EvidenceBatch(Key left, Key right) {}
}

class StreamEntryValueConstructorResolution {
    List<EvidenceBatch> run(Map<Key, List<Evidence>> values) {
        return values.entrySet().stream()
            .map(entry -> new EvidenceBatch(entry.getKey(), entry.getValue()))
            .collect(Collectors.toList());
    }
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    assert!(
        !resolved.diagnostics.has_errors(),
        "{:?}",
        resolved.diagnostics
    );

    let body = resolved.program.types[3]
        .members
        .iter()
        .find_map(|member| match member {
            Member::Method(method) if method.name == "run" => method.body.as_ref(),
            _ => None,
        })
        .unwrap();
    let Stmt::Return(Some(Expr::Call {
        receiver: Some(collect_receiver),
        ..
    })) = &body.statements[0]
    else {
        panic!("expected collect call return");
    };
    let Expr::Call { args, .. } = collect_receiver.as_ref() else {
        panic!("expected map call receiver");
    };
    let [
        Expr::Lambda {
            body: new_java2rust::java_ir::LambdaBody::Expr(lambda_body),
            ..
        },
    ] = args.as_slice()
    else {
        panic!("expected map lambda");
    };
    let Expr::New {
        resolved: Some(constructor),
        ..
    } = lambda_body.as_ref()
    else {
        panic!("expected resolved constructor in stream entry lambda");
    };

    assert_eq!(constructor.declaring_type, "EvidenceBatch");
    assert_eq!(constructor.rust_name, "new_key_iterable");
}

#[test]
fn resolver_uses_stream_map_lambda_body_type_before_collect_result_type() {
    let parsed = parse_java_source(
        r#"
class Genotype {}

class PosteriorBatch {
    PosteriorBatch(List<double[]> probabilities) {}
    PosteriorBatch(String probabilities) {}
}

class StreamMapCollectLambdaBodyTypeResolution {
    double[] parse(Genotype genotype) {
        return null;
    }

    PosteriorBatch run(List<Genotype> genotypes) {
        return new PosteriorBatch(genotypes.stream().map(g -> parse(g)).collect(Collectors.toList()));
    }
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    assert!(
        !resolved.diagnostics.has_errors(),
        "{:?}",
        resolved.diagnostics
    );

    let ty = &resolved.program.types[2];
    let run_body = ty
        .members
        .iter()
        .find_map(|member| match member {
            Member::Method(method) if method.name == "run" => method.body.as_ref(),
            _ => None,
        })
        .unwrap();
    let Stmt::Return(Some(Expr::New {
        resolved: Some(constructor),
        ..
    })) = &run_body.statements[0]
    else {
        panic!("expected resolved constructor from stream map collect result");
    };

    assert_eq!(constructor.declaring_type, "PosteriorBatch");
    assert_eq!(constructor.rust_name, "new_list");
}

#[test]
fn resolver_target_types_primitive_stream_mapper_lambda_before_array_length_field() {
    let parsed = parse_java_source(
        r#"
class ItemWithBytes {
    byte[] bytes() {
        return new byte[0];
    }
}

class PrimitiveStreamMapperLambdaResolution {
    void accept(int value) {}
    void accept(String value) {}

    void run(List<ItemWithBytes> items) {
        accept(items.stream().mapToInt(item -> item.bytes().length).sum());
    }
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    assert!(
        !resolved.diagnostics.has_errors(),
        "{:?}",
        resolved.diagnostics
    );

    let unresolved_fields = resolved
        .facts
        .unresolved_target_facts
        .iter()
        .filter(|fact| fact.kind == UnresolvedTargetKind::FieldAccess)
        .collect::<Vec<_>>();
    assert!(
        unresolved_fields.is_empty(),
        "typed stream mapper lambda should resolve array length on source call result: {unresolved_fields:?}"
    );

    let Stmt::Expr(Expr::Call {
        resolved: Some(call),
        ..
    }) = first_stmt(&resolved.program.types[1]).unwrap()
    else {
        panic!("expected resolved overload from primitive stream sum");
    };

    assert_eq!(call.declaring_type, "PrimitiveStreamMapperLambdaResolution");
    assert_eq!(call.rust_name, "accept_int");
}

#[test]
fn resolver_keeps_raw_primitive_stream_mapper_lambda_parameter_conservative() {
    let parsed = parse_java_source(
        r#"
class RawPrimitiveStreamMapperLambdaResolution {
    void run(Stream values) {
        values.mapToInt(value -> value.bytes().length).sum();
    }
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    assert!(
        !resolved.diagnostics.has_errors(),
        "{:?}",
        resolved.diagnostics
    );

    let unresolved_map_to_int = resolved
        .facts
        .unresolved_target_facts
        .iter()
        .filter(|fact| {
            fact.kind == UnresolvedTargetKind::MethodCall && fact.target_name == "mapToInt"
        })
        .collect::<Vec<_>>();
    assert_eq!(
        unresolved_map_to_int.len(),
        1,
        "raw stream mapper lambda should stay conservative without an element type: {:?}",
        resolved.facts.unresolved_target_facts
    );
}

#[test]
fn resolver_uses_static_source_factory_lambda_body_type_before_collect_result_type() {
    let parsed = parse_java_source(
        r#"
class Product {}

class ProductFactory {
    static Product create(String name) {
        return new Product();
    }
}

class StaticFactoryStreamCollectResolution {
    void accept(List<Product> products) {}
    void accept(String products) {}

    void run(List<String> names) {
        accept(names.stream().map(name -> ProductFactory.create(name)).collect(Collectors.toList()));
    }
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    assert!(
        !resolved.diagnostics.has_errors(),
        "{:?}",
        resolved.diagnostics
    );

    let ty = &resolved.program.types[2];
    let body = ty
        .members
        .iter()
        .find_map(|member| match member {
            Member::Method(method) if method.name == "run" => method.body.as_ref(),
            _ => None,
        })
        .unwrap();
    let Stmt::Expr(Expr::Call {
        resolved: Some(call),
        ..
    }) = &body.statements[0]
    else {
        panic!("expected resolved collect-result overload call");
    };

    assert_eq!(call.rust_name, "accept_list");
}

#[test]
fn resolver_uses_stream_map_method_reference_return_type_before_collect_result_type() {
    let parsed = parse_java_source(
        r#"
class HeaderBatch {
    HeaderBatch(List<String> headers, String label) {}
    HeaderBatch(List<Object> headers, int code) {}
}

class StreamMapMethodReferenceCollectTypeResolution {
    HeaderBatch run(List<HeaderLine> headers) {
        return new HeaderBatch(headers.stream().map(Object::toString).collect(Collectors.toList()), "ok");
    }
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    assert!(
        !resolved.diagnostics.has_errors(),
        "{:?}",
        resolved.diagnostics
    );

    let Expr::New {
        resolved: Some(constructor),
        ..
    } = first_return_expr(&resolved.program.types[1]).unwrap()
    else {
        panic!("expected resolved constructor from stream method-reference collect result");
    };

    assert_eq!(constructor.declaring_type, "HeaderBatch");
    assert_eq!(constructor.rust_name, "new_list_string");
}

#[test]
fn resolver_uses_current_receiver_method_reference_return_type_before_collect_result_type() {
    let parsed = parse_java_source(
        r#"
class Item {
}

class ItemConsumer {
    int consume(Collection<Item> items) {
        return 1;
    }

    int consume(Item item) {
        return 2;
    }
}

class CurrentReceiverMethodReferenceCollectResolution {
    Item getItem(int id) {
        return null;
    }

    int run(Collection<Integer> ids, ItemConsumer consumer) {
        return consumer.consume(ids.stream().map(this::getItem).collect(Collectors.toList()));
    }
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    assert!(
        !resolved.diagnostics.has_errors(),
        "{:?}",
        resolved.diagnostics
    );

    let run_body = resolved.program.types[2]
        .members
        .iter()
        .find_map(|member| match member {
            Member::Method(method) if method.name == "run" => method.body.as_ref(),
            _ => None,
        })
        .unwrap();
    let Stmt::Return(Some(Expr::Call {
        resolved: Some(call),
        ..
    })) = &run_body.statements[0]
    else {
        panic!(
            "expected resolved call from current-receiver method-reference collect result, got {:?}",
            run_body.statements[0]
        );
    };

    assert_eq!(call.declaring_type, "ItemConsumer");
    assert_eq!(call.rust_name, "consume_collection");
}

#[test]
fn resolver_uses_bound_receiver_method_reference_return_type_before_collect_result_type() {
    let parsed = parse_java_source(
        r#"
class ValueBatch {
    ValueBatch(List<String> values) {}
    ValueBatch(String value) {}
}

class BoundReceiverMethodReferenceCollectResolution {
    ValueBatch run(String[] keys, Map<String, String> valuesByKey) {
        return new ValueBatch(Arrays.stream(keys).map(valuesByKey::get).collect(Collectors.toList()));
    }
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    assert!(
        !resolved.diagnostics.has_errors(),
        "{:?}",
        resolved.diagnostics
    );

    let Expr::New {
        resolved: Some(constructor),
        ..
    } = first_return_expr(&resolved.program.types[1]).unwrap()
    else {
        panic!("expected resolved constructor from bound method-reference collect result");
    };

    assert_eq!(constructor.declaring_type, "ValueBatch");
    assert_eq!(constructor.rust_name, "new_list");
}

#[test]
fn resolver_uses_external_receiver_lambda_return_type_before_collect_result_type() {
    let parsed = parse_java_source(
        r#"
class ValueBatch {
    ValueBatch(List<String> values) {}
    ValueBatch(String value) {}
}

class ExternalReceiverLambdaCollectResolution {
    ValueBatch run(Map<String, String> valuesByKey) {
        List<String> keys = new ArrayList<>(valuesByKey.keySet());
        return new ValueBatch(keys.stream().map(key -> valuesByKey.get(key)).collect(Collectors.toList()));
    }
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    assert!(
        !resolved.diagnostics.has_errors(),
        "{:?}",
        resolved.diagnostics
    );

    let Expr::New {
        resolved: Some(constructor),
        ..
    } = first_return_expr(&resolved.program.types[1]).unwrap()
    else {
        panic!("expected resolved constructor from external receiver lambda collect result");
    };

    assert_eq!(constructor.declaring_type, "ValueBatch");
    assert_eq!(constructor.rust_name, "new_list");
}

#[test]
fn resolver_uses_stream_collect_type_for_same_class_static_factory_with_source_context() {
    let parsed = parse_java_source(
        r#"
class RecordFactory {
    static Record create(List<String> names, List<String> values, Marker marker, String label, Metadata metadata) {
        return null;
    }

    static Record create(LinkedHashMap<String, String> data, Marker marker, String label, Metadata metadata) {
        List<String> names = new ArrayList<>(data.keySet());
        List<String> values = names.stream().map(name -> data.get(name)).collect(Collectors.toList());
        return create(names, values, marker, label, null);
    }
}
"#,
    );
    let extra = parse_java_source(
        r#"
class ImportedSourceContext {
}
"#,
    );
    let mut source_program = parsed.program.clone();
    source_program.types.extend(extra.program.types.clone());

    let resolved =
        resolve_program_with_source_program(&parsed.program, parsed.diagnostics, &source_program);
    assert!(
        !resolved.diagnostics.has_errors(),
        "{:?}",
        resolved.diagnostics
    );

    let body = resolved.program.types[0]
        .members
        .iter()
        .find_map(|member| match member {
            Member::Method(method)
                if method.rust_name.as_deref()
                    == Some("create_linked_hash_map_marker_string_metadata") =>
            {
                method.body.as_ref()
            }
            _ => None,
        })
        .unwrap();
    let Stmt::Return(Some(Expr::Call {
        resolved: Some(call),
        ..
    })) = &body.statements[2]
    else {
        panic!("expected resolved same-class static factory call");
    };

    assert_eq!(call.declaring_type, "RecordFactory");
    assert_eq!(call.rust_name, "create_list_list_marker_string_metadata");
}

#[test]
fn resolver_matches_boxed_generic_element_type_for_overload_arguments() {
    let parsed = parse_java_source(
        r#"
class BoxedGenericElementOverloadResolution {
    double score(int refCount, List<Byte> qualities, int repeatFactor) {
        return 1.0;
    }

    double score(int refCount, int altCount, double errorProbability) {
        return 2.0;
    }

    double run(int refCount, List<byte> qualities) {
        return score(refCount, qualities, 1);
    }
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    assert!(
        !resolved.diagnostics.has_errors(),
        "{:?}",
        resolved.diagnostics
    );

    let run_body = resolved.program.types[0]
        .members
        .iter()
        .find_map(|member| match member {
            Member::Method(method) if method.name == "run" => method.body.as_ref(),
            _ => None,
        })
        .unwrap();
    let Stmt::Return(Some(Expr::Call {
        resolved: Some(call),
        ..
    })) = &run_body.statements[0]
    else {
        panic!("expected resolved overload from boxed generic element conversion");
    };

    assert_eq!(call.declaring_type, "BoxedGenericElementOverloadResolution");
    assert_eq!(call.rust_name, "score_int_list_int");
}

#[test]
fn resolver_substitutes_inherited_generic_return_type_through_superclass_chain() {
    let parsed = parse_java_source(
        r#"
class Meta {}
class Record {}
class Dictionary {}

class BaseCollection<M, R> {
    List<R> getRecords() {
        return null;
    }
}

class LocatableCollection<M, R> extends BaseCollection<M, R> {
}

class RecordCollection extends LocatableCollection<Meta, Record> {
}

class InheritedGenericReturnResolution {
    static List<Record> withFlanks(List<Record> records, int padding, Dictionary dictionary) {
        return records;
    }

    static List<Record> withFlanks(String records, int padding, Dictionary dictionary) {
        return null;
    }

    List<Record> run(RecordCollection collection, Dictionary dictionary) {
        return withFlanks(collection.getRecords(), 0, dictionary);
    }
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    assert!(
        !resolved.diagnostics.has_errors(),
        "{:?}",
        resolved.diagnostics
    );

    let run_body = resolved.program.types[6]
        .members
        .iter()
        .find_map(|member| match member {
            Member::Method(method) if method.name == "run" => method.body.as_ref(),
            _ => None,
        })
        .unwrap();
    let Stmt::Return(Some(Expr::Call {
        resolved: Some(call),
        ..
    })) = &run_body.statements[0]
    else {
        panic!("expected resolved static overload from inherited generic return type");
    };

    assert_eq!(call.declaring_type, "InheritedGenericReturnResolution");
    assert_eq!(call.rust_name, "with_flanks_list_int_dictionary");
}

#[test]
fn resolver_uses_type_parameter_bound_for_unbounded_wildcard_receiver_return() {
    let parsed = parse_java_source(
        r#"
interface Metadata {}
interface LocatableMetadata extends Metadata {}
interface Locatable {}

class BaseCollection<M extends Metadata, R> {
    M getMetadata() {
        return null;
    }
}

class LocatableCollection<M extends LocatableMetadata, R extends Locatable> extends BaseCollection<M, R> {
}

class Record implements Locatable {}

class TargetCollection {
    TargetCollection(LocatableMetadata metadata, List<Record> records) {}
    TargetCollection(String metadata, List<Record> records) {}
}

class BoundedWildcardReceiverReturnResolution {
    TargetCollection run(LocatableCollection<?, ?> collection, List<Record> records) {
        return new TargetCollection(collection.getMetadata(), records);
    }
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    assert!(
        !resolved.diagnostics.has_errors(),
        "{:?}",
        resolved.diagnostics
    );

    let Expr::New {
        resolved: Some(constructor),
        ..
    } = first_return_expr(&resolved.program.types[7]).unwrap()
    else {
        panic!("expected resolved constructor from bounded wildcard receiver return");
    };

    assert_eq!(constructor.declaring_type, "TargetCollection");
    assert_eq!(constructor.rust_name, "new_locatable_metadata_list");
}

#[test]
fn resolver_uses_current_type_parameter_bound_for_receiver_method_return() {
    let parsed = parse_java_source(
        r#"
class BaseNode {
    byte[] bytes(boolean includePrefix) {
        return null;
    }
}

class NodeSink {
    NodeSink(byte[] bytes) {}
    NodeSink(String text) {}
}

class BoundedReceiverMethodResolution<V extends BaseNode> {
    NodeSink make(V node) {
        return new NodeSink(node.bytes(true));
    }
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    assert!(
        !resolved.diagnostics.has_errors(),
        "{:?}",
        resolved.diagnostics
    );

    let Expr::New {
        resolved: Some(constructor),
        ..
    } = first_return_expr(&resolved.program.types[2]).unwrap()
    else {
        panic!("expected resolved constructor from bounded receiver method result");
    };

    assert_eq!(constructor.declaring_type, "NodeSink");
    assert_eq!(constructor.rust_name, "new_byte_array");
}

#[test]
fn resolver_deduplicates_substituted_generic_override_candidates() {
    let parsed = parse_java_source(
        r#"
class Item {}
class Result {}

abstract class Linkage<T> {
    abstract Result matches(T left, T right);

    int maxStart(T item) {
        return 1;
    }

    int maxStart(Collection<T> items) {
        return 2;
    }
}

class ConcreteLinkage<T extends Item> extends Linkage<T> {
    Result matches(Item left, Item right) {
        return null;
    }

    int maxStart(Item item) {
        return 3;
    }
}

class UsesLinkage {
    Result run(ConcreteLinkage<Item> linkage, Item left, Item right) {
        return linkage.matches(left, right);
    }

    int runOne(ConcreteLinkage<Item> linkage, Item item) {
        return linkage.maxStart(item);
    }
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    assert!(
        !resolved.diagnostics.has_errors(),
        "{:?}",
        resolved.diagnostics
    );

    let ty = &resolved.program.types[4];
    let calls = ty
        .members
        .iter()
        .filter_map(|member| match member {
            Member::Method(method) => method.body.as_ref(),
            _ => None,
        })
        .filter_map(|body| match &body.statements[0] {
            Stmt::Return(Some(Expr::Call {
                resolved: Some(call),
                ..
            })) => Some((call.declaring_type.as_str(), call.rust_name.as_str())),
            _ => None,
        })
        .collect::<Vec<_>>();

    assert_eq!(
        calls,
        vec![
            ("ConcreteLinkage", "matches"),
            ("ConcreteLinkage", "max_start")
        ]
    );
}

#[test]
fn resolver_uses_stream_iterator_result_type_after_map_for_overloads() {
    let parsed = parse_java_source(
        r#"
class StreamIteratorAfterMapResolution {
    void consume(Iterator<Product> values) {}
    void consume(Stream<Product> values) {}

    Product convert(Item item) {
        return null;
    }

    void run(Iterator<Item> items) {
        consume(Utils.stream(items).map(item -> convert(item)).iterator());
    }
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    assert!(
        !resolved.diagnostics.has_errors(),
        "{:?}",
        resolved.diagnostics
    );

    let body = resolved.program.types[0]
        .members
        .iter()
        .find_map(|member| match member {
            Member::Method(method) if method.name == "run" => method.body.as_ref(),
            _ => None,
        })
        .unwrap();
    let Stmt::Expr(Expr::Call {
        resolved: Some(call),
        ..
    }) = &body.statements[0]
    else {
        panic!("expected resolved overload from stream iterator terminal");
    };

    assert_eq!(call.rust_name, "consume_iterator");
}

#[test]
fn resolver_uses_source_iterator_return_type_for_source_stream_overload() {
    let parsed = parse_java_source(
        r#"
class Utils {
    static <T> Stream<T> stream(Iterable<T> values) {
        return null;
    }

    static <T> Stream<T> stream(Iterator<T> values) {
        return null;
    }
}

class Context {
    Iterator<Item> iterator() {
        return null;
    }
}

class SourceIteratorStreamOverloadResolution {
    void run(Context context) {
        Utils.stream(context.iterator()).forEach(item -> consume(item));
    }

    void consume(Item item) {
    }
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    assert!(
        !resolved.diagnostics.has_errors(),
        "{:?}",
        resolved.diagnostics
    );

    let body = resolved.program.types[2]
        .members
        .iter()
        .find_map(|member| match member {
            Member::Method(method) if method.name == "run" => method.body.as_ref(),
            _ => None,
        })
        .unwrap();
    let Stmt::Expr(expr) = &body.statements[0] else {
        panic!("expected call expression");
    };
    let mut calls = Vec::new();
    collect_calls(expr, &mut calls);
    let stream_call = calls
        .into_iter()
        .find_map(|expr| match expr {
            Expr::Call {
                name,
                resolved: Some(call),
                ..
            } if name == "stream" => Some(call),
            _ => None,
        })
        .expect("expected resolved source stream overload");

    assert_eq!(stream_call.rust_name, "stream_iterator");
}

#[test]
fn resolver_uses_imported_generic_source_iterator_return_type_for_stream_overload() {
    let parsed = parse_java_source(
        r#"
package usecase;

import model.RecordSource;
import model.Utils;

class ImportedGenericSourceIteratorStreamResolution {
    void consume(Record record) {}

    void run(String path) {
        try (final RecordSource<Record> source = new RecordSource<>(path, null, 0, null)) {
            Utils.stream(source.iterator()).forEach(record -> consume(record));
        }
    }
}
"#,
    );
    let source = parse_java_source(
        r#"
package model;

interface Feature {
}

class RecordSource<T extends Feature> {
    RecordSource(String path, Object featureInput, int lookahead, Object cloudPrefetchBuffer) {}

    Iterator<T> iterator() {
        return null;
    }
}

class Stream<T> {
    void forEach(Consumer<T> consumer) {}
}

class Utils {
    static <T> Stream<T> stream(Iterable<T> values) {
        return null;
    }

    static <T> Stream<T> stream(Iterator<T> values) {
        return null;
    }
}

class Record implements Feature {
}
"#,
    );
    let mut source_program = parsed.program.clone();
    source_program.types.extend(source.program.types.clone());

    let resolved =
        resolve_program_with_source_program(&parsed.program, parsed.diagnostics, &source_program);
    assert!(
        !resolved.diagnostics.has_errors(),
        "{:?}",
        resolved.diagnostics
    );

    let body = resolved.program.types[0]
        .members
        .iter()
        .find_map(|member| match member {
            Member::Method(method) if method.name == "run" => method.body.as_ref(),
            _ => None,
        })
        .unwrap();
    let Stmt::Try { body: try_body, .. } = &body.statements[0] else {
        panic!("expected try-with-resources statement");
    };
    let Stmt::Expr(expr) = &try_body.statements[0] else {
        panic!("expected stream expression statement inside try body");
    };
    let mut calls = Vec::new();
    collect_calls(expr, &mut calls);
    let stream_call = calls
        .iter()
        .find_map(|expr| match expr {
            Expr::Call {
                name,
                resolved: Some(call),
                ..
            } if name == "stream" => Some(call),
            _ => None,
        })
        .expect("expected resolved stream overload from imported generic source iterator");

    assert_eq!(stream_call.declaring_type, "Utils");
    assert_eq!(stream_call.rust_name, "stream_iterator");
}

#[test]
fn resolver_uses_external_identifier_getter_method_reference_before_collect_result_type() {
    let parsed = parse_java_source(
        r#"
class IdentifierBatch {
    IdentifierBatch(List<String> identifiers) {}
    IdentifierBatch(String identifier) {}
}

class StreamMapIdentifierGetterReferenceResolution {
    IdentifierBatch run(List<HeaderLine> headers) {
        return new IdentifierBatch(headers.stream().map(HeaderLine::getID).collect(Collectors.toList()));
    }
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    assert!(
        !resolved.diagnostics.has_errors(),
        "{:?}",
        resolved.diagnostics
    );

    let Expr::New {
        resolved: Some(constructor),
        ..
    } = first_return_expr(&resolved.program.types[1]).unwrap()
    else {
        panic!("expected resolved constructor from identifier getter collect result");
    };

    assert_eq!(constructor.declaring_type, "IdentifierBatch");
    assert_eq!(constructor.rust_name, "new_list");
}

#[test]
fn resolver_uses_external_header_metadata_retrieval_before_collect_result_type() {
    let parsed = parse_java_source(
        r#"
class HeaderBatch {
    HeaderBatch(List<String> identifiers, String label) {}
    HeaderBatch(List<Object> identifiers, int label) {}
}

class ExternalHeaderMetadataRetrievalResolution {
    HeaderBatch run(HeaderMetadata metadata) {
        return new HeaderBatch(metadata.retrieveAllHeaderInfo().stream().map(HeaderLine::getID).collect(Collectors.toList()), "headers");
    }
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    assert!(
        !resolved.diagnostics.has_errors(),
        "{:?}",
        resolved.diagnostics
    );

    let Expr::New {
        resolved: Some(constructor),
        ..
    } = first_return_expr(&resolved.program.types[1]).unwrap()
    else {
        panic!("expected resolved constructor from external header metadata collect result");
    };

    assert_eq!(constructor.declaring_type, "HeaderBatch");
    assert_eq!(constructor.rust_name, "new_list_string");
}

#[test]
fn resolver_matches_expanded_varargs_constructor_arguments() {
    let parsed = parse_java_source(
        r#"
class ColumnNames {
    ColumnNames(Iterable<String> names) {}
    ColumnNames(String... names) {}
    ColumnNames(Object... names) {}
}

class VarargsConstructorResolution {
    ColumnNames single() {
        return new ColumnNames("sample");
    }

    ColumnNames many() {
        return new ColumnNames("sample", "count");
    }
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    assert!(
        !resolved.diagnostics.has_errors(),
        "{:?}",
        resolved.diagnostics
    );

    let ty = &resolved.program.types[1];
    let constructors = ty
        .members
        .iter()
        .filter_map(|member| match member {
            Member::Method(method) => method.body.as_ref(),
            _ => None,
        })
        .filter_map(|body| match &body.statements[0] {
            Stmt::Return(Some(Expr::New {
                resolved: Some(constructor),
                ..
            })) => Some(constructor.rust_name.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>();

    assert_eq!(
        constructors,
        vec!["new_string_array_varargs", "new_string_array_varargs"]
    );
}

#[test]
fn resolver_matches_expanded_varargs_method_arguments() {
    let parsed = parse_java_source(
        r#"
class VarargsMethodResolution {
    void record(String... values) {}
    void record(Object... values) {}

    void run() {
        record("sample");
        record("sample", "count");
    }
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    assert!(
        !resolved.diagnostics.has_errors(),
        "{:?}",
        resolved.diagnostics
    );

    let ty = &resolved.program.types[0];
    let body = ty
        .members
        .iter()
        .find_map(|member| match member {
            Member::Method(method) if method.name == "run" => method.body.as_ref(),
            _ => None,
        })
        .unwrap();
    let calls = body
        .statements
        .iter()
        .filter_map(|stmt| match stmt {
            Stmt::Expr(Expr::Call {
                resolved: Some(call),
                ..
            }) => Some(call.rust_name.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>();

    assert_eq!(
        calls,
        vec!["record_string_array_varargs", "record_string_array_varargs"]
    );
}

#[test]
fn resolver_matches_external_write_option_subtypes_to_varargs_parameter() {
    let parsed = parse_java_source(
        r#"
class ExternalWriteOptionVarargsResolution {
    void write(long indexGranularity, WriteOption... options) {}
    void write(String path, WriteOption option) {}

    void run(FileCardinalityWriteOption cardinality,
             TempPartsDirectoryWriteOption tempParts,
             BaiWriteOption bai,
             SbiWriteOption sbi) {
        write(128L, cardinality, tempParts, bai, sbi);
    }
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    assert!(
        !resolved.diagnostics.has_errors(),
        "{:?}",
        resolved.diagnostics
    );

    let body = resolved.program.types[0]
        .members
        .iter()
        .find_map(|member| match member {
            Member::Method(method) if method.name == "run" => method.body.as_ref(),
            _ => None,
        })
        .unwrap();
    let Stmt::Expr(Expr::Call {
        resolved: Some(call),
        ..
    }) = &body.statements[0]
    else {
        panic!("expected resolved external write option varargs call");
    };

    assert_eq!(call.declaring_type, "ExternalWriteOptionVarargsResolution");
    assert_eq!(call.rust_name, "write_long_write_option_array_varargs");
}

#[test]
fn resolver_matches_explicit_array_argument_to_varargs_parameter() {
    let parsed = parse_java_source(
        r#"
class ExplicitArrayVarargsResolution {
    static String join(CharSequence separator, Object... values) {
        return null;
    }

    static String join(CharSequence separator, int value) {
        return null;
    }

    void run(String... columns) {
        join("\t", (Object[]) columns);
    }
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    assert!(
        !resolved.diagnostics.has_errors(),
        "{:?}",
        resolved.diagnostics
    );

    let body = resolved.program.types[0]
        .members
        .iter()
        .find_map(|member| match member {
            Member::Method(method) if method.name == "run" => method.body.as_ref(),
            _ => None,
        })
        .unwrap();
    let Stmt::Expr(Expr::Call {
        resolved: Some(call),
        ..
    }) = &body.statements[0]
    else {
        panic!(
            "expected resolved explicit array varargs call, got {:?}",
            body.statements[0]
        );
    };

    assert_eq!(call.declaring_type, "ExplicitArrayVarargsResolution");
    assert_eq!(
        call.rust_name, "join_char_sequence_object_array_varargs",
        "{:?}",
        body.statements[0]
    );
}

#[test]
fn resolver_uses_unknown_boundary_argument_when_other_arguments_select_overload() {
    let parsed = parse_java_source(
        r#"
class UnknownBoundaryOverloadResolution {
    void split(String value, char delimiter) {}
    void split(String value, String delimiter) {}

    void run(ExternalCursor cursor) {
        split(cursor.next(), ",");
    }
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    assert!(
        !resolved.diagnostics.has_errors(),
        "{:?}",
        resolved.diagnostics
    );

    let Stmt::Expr(Expr::Call {
        resolved: Some(call),
        ..
    }) = first_stmt(&resolved.program.types[0]).unwrap()
    else {
        panic!("expected resolved overload with unknown boundary argument");
    };

    assert_eq!(call.declaring_type, "UnknownBoundaryOverloadResolution");
    assert_eq!(call.rust_name, "split_string_string");
}

#[test]
fn resolver_uses_unknown_boundary_argument_for_primitive_target_when_disambiguated() {
    let parsed = parse_java_source(
        r#"
class UnknownBoundaryPrimitiveOverloadResolution {
    void validate(boolean condition, String message) {}
    void validate(boolean condition, Supplier<String> message) {}

    void run(ExternalComparator comparator) {
        validate(comparator.contigsMatch(), "mismatch");
    }
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    assert!(
        !resolved.diagnostics.has_errors(),
        "{:?}",
        resolved.diagnostics
    );

    let Stmt::Expr(Expr::Call {
        resolved: Some(call),
        ..
    }) = first_stmt(&resolved.program.types[0]).unwrap()
    else {
        panic!("expected resolved overload with unknown primitive-like boundary argument");
    };

    assert_eq!(
        call.declaring_type,
        "UnknownBoundaryPrimitiveOverloadResolution"
    );
    assert_eq!(call.rust_name, "validate_boolean_string");
}

#[test]
fn resolver_allows_raw_generic_actual_for_parameterized_generic_overload() {
    let parsed = parse_java_source(
        r#"
interface Locatable {
}

class ReadRecord implements Locatable {
}

class Rdd<T> {
}

class Boundary {
}

class GenericStaticDispatch {
    static <L extends Locatable, B extends Boundary> Rdd<L> shard(
            Rdd<L> records,
            Class<L> recordClass,
            List<B> boundaries,
            int size,
            boolean shuffle) {
        return null;
    }

    static Rdd<String> shard(String records, Class<String> recordClass, List<String> boundaries, String size, boolean shuffle) {
        return null;
    }

    Rdd<ReadRecord> run(Rdd<ReadRecord> records, List<Boundary> boundaries, Class recordClass) {
        return shard(records, recordClass, boundaries, 100, false);
    }
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    assert!(
        !resolved.diagnostics.has_errors(),
        "{:?}",
        resolved.diagnostics
    );

    let ty = &resolved.program.types[4];
    let body = ty
        .members
        .iter()
        .find_map(|member| match member {
            Member::Method(method) if method.name == "run" => method.body.as_ref(),
            _ => None,
        })
        .unwrap();
    let Stmt::Return(Some(Expr::Call {
        resolved: Some(call),
        ..
    })) = &body.statements[0]
    else {
        panic!("expected resolved generic overload with raw Class actual");
    };

    assert_eq!(call.declaring_type, "GenericStaticDispatch");
    assert_eq!(call.rust_name, "shard_rdd_class_list_int_boolean");
    assert_eq!(
        call.return_type,
        JavaType::Named {
            name: "Rdd".to_string(),
            args: vec![JavaType::Named {
                name: "ReadRecord".to_string(),
                args: Vec::new(),
            }],
        }
    );
}

#[test]
fn resolver_uses_transitive_source_hierarchy_for_constructor_overload_assignability() {
    let parsed = parse_java_source(
        r#"
interface Api {
}

class Base implements Api {
}

class Impl extends Base {
}

class Target {
    Target(int value) {}
    Target(Api value) {}
}

class UseTransitiveConstructorInterfaceAssignability {
    Target make(Impl value) {
        return new Target(value);
    }
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    assert!(
        !resolved.diagnostics.has_errors(),
        "{:?}",
        resolved.diagnostics
    );

    let Expr::New {
        resolved: Some(constructor),
        ..
    } = first_return_expr(&resolved.program.types[4]).unwrap()
    else {
        panic!("expected resolved constructor call through transitive source hierarchy");
    };

    assert_eq!(constructor.declaring_type, "Target");
    assert_eq!(constructor.rust_name, "new_api");
}

#[test]
fn resolver_uses_source_interface_extends_for_constructor_overload_assignability() {
    let parsed = parse_java_source(
        r#"
interface Locatable {
}

interface ReadRecord extends Locatable {
}

class TargetInterval {
    TargetInterval(String label) {}
    TargetInterval(Locatable locatable) {}
}

class UseSourceInterfaceExtendsAssignability {
    TargetInterval make(ReadRecord read) {
        return new TargetInterval(read);
    }
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    assert!(
        !resolved.diagnostics.has_errors(),
        "{:?}",
        resolved.diagnostics
    );

    let Expr::New {
        resolved: Some(constructor),
        ..
    } = first_return_expr(&resolved.program.types[3]).unwrap()
    else {
        panic!("expected resolved constructor call through source interface extends");
    };

    assert_eq!(constructor.declaring_type, "TargetInterval");
    assert_eq!(constructor.rust_name, "new_locatable");
}

#[test]
fn resolver_composes_source_extends_with_known_external_supertype_assignability() {
    let parsed = parse_java_source(
        r#"
class TranscriptFeature extends Feature {
}

class TargetInterval {
    TargetInterval(String label) {}
    TargetInterval(Locatable locatable) {}
}

class UseSourceAndExternalHierarchyAssignability {
    TargetInterval make(TranscriptFeature feature) {
        return new TargetInterval(feature);
    }
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    assert!(
        !resolved.diagnostics.has_errors(),
        "{:?}",
        resolved.diagnostics
    );

    let Expr::New {
        resolved: Some(constructor),
        ..
    } = first_return_expr(&resolved.program.types[2]).unwrap()
    else {
        panic!("expected resolved constructor through source plus external hierarchy");
    };

    assert_eq!(constructor.declaring_type, "TargetInterval");
    assert_eq!(constructor.rust_name, "new_locatable");
}

#[test]
fn resolver_allows_bounded_method_type_variable_actual_for_reference_constructor() {
    let parsed = parse_java_source(
        r#"
interface Locatable {
}

class TargetInterval {
    TargetInterval(String label) {}
    TargetInterval(Locatable locatable) {}
}

class UseBoundedTypeVariableConstructorArgument {
    <T extends Locatable> TargetInterval make(T value) {
        return new TargetInterval(value);
    }
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    assert!(
        !resolved.diagnostics.has_errors(),
        "{:?}",
        resolved.diagnostics
    );

    let Expr::New {
        resolved: Some(constructor),
        ..
    } = first_return_expr(&resolved.program.types[2]).unwrap()
    else {
        panic!("expected resolved constructor from bounded type-variable actual");
    };

    assert_eq!(constructor.declaring_type, "TargetInterval");
    assert_eq!(constructor.rust_name, "new_locatable");
}

#[test]
fn resolver_uses_class_literal_target_type_for_wildcard_class_parameters() {
    let parsed = parse_java_source(
        r#"
interface Feature {}
class VariantRecord implements Feature {}
class FeatureInput<T> {}
class Options {}

class ClassLiteralWildcardArgumentResolution {
    void addToFeatureSources(int lookahead, FeatureInput<? extends Feature> input, Class<? extends Feature> featureType, int cloudBuffer, int indexBuffer, Options options) {}
    void addToFeatureSources(int lookahead, FeatureInput<? extends Feature> input, String featureType, int cloudBuffer, int indexBuffer, Options options) {}

    void run(FeatureInput<VariantRecord> input, Options options) {
        addToFeatureSources(0, input, VariantRecord.class, 1, 2, options);
    }
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    assert!(
        !resolved.diagnostics.has_errors(),
        "{:?}",
        resolved.diagnostics
    );

    let Stmt::Expr(Expr::Call {
        resolved: Some(call),
        ..
    }) = first_stmt(&resolved.program.types[4]).unwrap()
    else {
        panic!("expected resolved class-literal wildcard overload");
    };

    assert_eq!(
        call.declaring_type,
        "ClassLiteralWildcardArgumentResolution"
    );
    assert_eq!(
        call.rust_name,
        "add_to_feature_sources_int_feature_input_class_int_int_options"
    );
}

#[test]
fn resolver_uses_known_external_feature_subtype_for_wildcard_class_parameters() {
    let parsed = parse_java_source(
        r#"
class FeatureInput<T> {}
class GenomicsOptions {}

class ExternalFeatureClassLiteralArgumentResolution {
    void addToFeatureSources(int lookahead, FeatureInput<? extends Feature> input, Class<? extends Feature> featureType, int cloudBuffer, int indexBuffer, GenomicsOptions options) {}
    void addToFeatureSources(int lookahead, FeatureInput<? extends Feature> input, String featureType, int cloudBuffer, int indexBuffer, GenomicsOptions options) {}

    void run(FeatureInput<VariantContext> input, GenomicsOptions options) {
        addToFeatureSources(0, input, VariantContext.class, 1, 2, options);
    }
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    assert!(
        !resolved.diagnostics.has_errors(),
        "{:?}",
        resolved.diagnostics
    );

    let Stmt::Expr(Expr::Call {
        resolved: Some(call),
        ..
    }) = first_stmt(&resolved.program.types[2]).unwrap()
    else {
        panic!("expected resolved external-feature class-literal wildcard overload");
    };

    assert_eq!(
        call.declaring_type,
        "ExternalFeatureClassLiteralArgumentResolution"
    );
    assert_eq!(
        call.rust_name,
        "add_to_feature_sources_int_feature_input_class_int_int_genomics_options"
    );
}

#[test]
fn resolver_uses_known_external_locatable_assignability_for_constructor_overload_arguments() {
    let parsed = parse_java_source(
        r#"
class TargetInterval {
    TargetInterval(String label) {}
    TargetInterval(Locatable locatable) {}
}

class UseExternalLocatableAssignability {
    TargetInterval make(VariantContext variant) {
        return new TargetInterval(variant);
    }

    TargetInterval makeNamed(NamedFeature feature) {
        return new TargetInterval(feature);
    }
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    assert!(
        !resolved.diagnostics.has_errors(),
        "{:?}",
        resolved.diagnostics
    );

    let constructors = resolved.program.types[1]
        .members
        .iter()
        .filter_map(|member| match member {
            Member::Method(method) => method.body.as_ref(),
            _ => None,
        })
        .filter_map(|body| match &body.statements[0] {
            Stmt::Return(Some(Expr::New {
                resolved: Some(constructor),
                ..
            })) => Some(constructor.rust_name.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>();

    assert_eq!(constructors, vec!["new_locatable", "new_locatable"]);
}

#[test]
fn resolver_uses_source_field_access_type_for_constructor_overload_arguments() {
    let parsed = parse_java_source(
        r#"
class Payload {
}

class Context {
    Payload payload;
}

class Target {
    Target(String label) {}
    Target(Payload payload) {}
}

class UseFieldAccessArgumentType {
    Target make(Context context) {
        return new Target(context.payload);
    }
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    assert!(
        !resolved.diagnostics.has_errors(),
        "{:?}",
        resolved.diagnostics
    );

    let Expr::New {
        resolved: Some(constructor),
        ..
    } = first_return_expr(&resolved.program.types[3]).unwrap()
    else {
        panic!("expected resolved constructor call from source field access type");
    };

    assert_eq!(constructor.declaring_type, "Target");
    assert_eq!(constructor.rust_name, "new_payload");
}

#[test]
fn resolver_uses_known_numeric_constant_field_types_for_overload_arguments() {
    let parsed = parse_java_source(
        r#"
class NumericConstantArgumentResolution {
    void useInt(int value) {}
    void useInt(String value) {}

    void useDouble(double value) {}
    void useDouble(String value) {}

    void run() {
        useInt(Integer.MAX_VALUE);
        useInt(Byte.MIN_VALUE);
        useDouble(Double.NaN);
        useDouble(Double.POSITIVE_INFINITY);
        useDouble(Float.NEGATIVE_INFINITY);
    }
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    assert!(
        !resolved.diagnostics.has_errors(),
        "{:?}",
        resolved.diagnostics
    );

    let ty = &resolved.program.types[0];
    let body = ty
        .members
        .iter()
        .find_map(|member| match member {
            Member::Method(method) if method.name == "run" => method.body.as_ref(),
            _ => None,
        })
        .unwrap();

    let expected = [
        "use_int_int",
        "use_int_int",
        "use_double_double",
        "use_double_double",
        "use_double_double",
    ];
    for (stmt, expected_rust_name) in body.statements.iter().zip(expected) {
        let Stmt::Expr(Expr::Call {
            resolved: Some(call),
            ..
        }) = stmt
        else {
            panic!("expected resolved numeric constant overload call");
        };

        assert_eq!(call.declaring_type, "NumericConstantArgumentResolution");
        assert_eq!(call.rust_name, expected_rust_name);
    }
}

#[test]
fn resolver_uses_inherited_source_fields_for_constructor_overload_arguments() {
    let parsed = parse_java_source(
        r#"
class Reference {
}

class FeatureStore {
}

class Span {
}

class BaseTraversal {
    Reference reference;
    FeatureStore features;
}

class ReferenceContext {
    ReferenceContext(Reference reference, Span span) {}
    ReferenceContext(String reference, String span) {}
}

class FeatureContext {
    FeatureContext(FeatureStore features, Span span) {}
    FeatureContext(String features, String span) {}
}

class ChildTraversal extends BaseTraversal {
    void run(Span span) {
        new ReferenceContext(reference, span);
        new FeatureContext(features, span);
    }
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    assert!(
        !resolved.diagnostics.has_errors(),
        "{:?}",
        resolved.diagnostics
    );

    let ty = resolved
        .program
        .types
        .iter()
        .find(|ty| ty.name == "ChildTraversal")
        .unwrap();
    let body = ty
        .members
        .iter()
        .find_map(|member| match member {
            Member::Method(method) if method.name == "run" => method.body.as_ref(),
            _ => None,
        })
        .unwrap();
    let constructors = body
        .statements
        .iter()
        .map(|stmt| match stmt {
            Stmt::Expr(Expr::New {
                resolved: Some(constructor),
                ..
            }) => constructor.rust_name.as_str(),
            _ => panic!("expected resolved constructor expression"),
        })
        .collect::<Vec<_>>();

    assert_eq!(
        constructors,
        vec!["new_reference_span", "new_feature_store_span"]
    );
}

#[test]
fn resolver_uses_modeled_external_superclass_fields_for_overload_arguments() {
    let parsed = parse_java_source(
        r#"
class MetricsRecord extends DuplicationMetrics {
    void acceptLibrary(String value) {}
    void acceptLibrary(int value) {}

    void acceptCount(long value) {}
    void acceptCount(String value) {}

    void acceptPercent(double value) {}
    void acceptPercent(String value) {}

    void run(MetricsRecord other) {
        acceptLibrary(this.LIBRARY);
        acceptCount(other.READ_PAIRS_EXAMINED);
        acceptCount(this.UNPAIRED_READ_DUPLICATES);
        acceptPercent(other.PERCENT_DUPLICATION);
    }
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    assert!(
        !resolved.diagnostics.has_errors(),
        "{:?}",
        resolved.diagnostics
    );

    let unresolved_fields = resolved
        .facts
        .unresolved_target_facts
        .iter()
        .filter(|fact| fact.kind == UnresolvedTargetKind::FieldAccess)
        .collect::<Vec<_>>();
    assert!(
        unresolved_fields.is_empty(),
        "modeled external superclass fields should resolve before lowering: {unresolved_fields:?}"
    );

    let calls = resolved.program.types[0]
        .members
        .iter()
        .find_map(|member| match member {
            Member::Method(method) if method.name == "run" => method.body.as_ref(),
            _ => None,
        })
        .unwrap()
        .statements
        .iter()
        .filter_map(|stmt| match stmt {
            Stmt::Expr(Expr::Call {
                resolved: Some(call),
                ..
            }) => Some(call.rust_name.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>();

    assert_eq!(
        calls,
        vec![
            "accept_library_string",
            "accept_count_long",
            "accept_count_long",
            "accept_percent_double",
        ]
    );
}

#[test]
fn resolver_uses_inherited_camel_case_field_receiver_for_source_getter_return() {
    let parsed = parse_java_source(
        r#"
class Path {}
class Source {}

class ReferenceArguments {
    Path getReferencePath() {
        return null;
    }
}

class BaseTool {
    protected final ReferenceArguments referenceArguments = null;
}

class Reader {
    Reader(Path path) {}
    Reader(Source source) {}
}

class ChildTool extends BaseTool {
    void run() {
        new Reader(referenceArguments.getReferencePath());
    }
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    assert!(
        !resolved.diagnostics.has_errors(),
        "{:?}",
        resolved.diagnostics
    );

    let ty = resolved
        .program
        .types
        .iter()
        .find(|ty| ty.name == "ChildTool")
        .unwrap();
    let body = ty
        .members
        .iter()
        .find_map(|member| match member {
            Member::Method(method) if method.name == "run" => method.body.as_ref(),
            _ => None,
        })
        .unwrap();
    let Stmt::Expr(Expr::New {
        resolved: Some(constructor),
        ..
    }) = &body.statements[0]
    else {
        panic!("expected resolved constructor from inherited field getter return");
    };

    assert_eq!(constructor.declaring_type, "Reader");
    assert_eq!(constructor.rust_name, "new_path");
}

#[test]
fn resolver_uses_external_reference_path_getter_return_type_for_constructor_overloads() {
    let parsed = parse_java_source(
        r#"
class Path {}
class Source {}

class Reader {
    Reader(Path path) {}
    Reader(Source source) {}
}

class ExternalReferencePathGetterResolution {
    Reader run(ExternalReferenceArguments arguments) {
        return new Reader(arguments.getReferencePath());
    }
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    assert!(
        !resolved.diagnostics.has_errors(),
        "{:?}",
        resolved.diagnostics
    );

    let ty = resolved
        .program
        .types
        .iter()
        .find(|ty| ty.name == "ExternalReferencePathGetterResolution")
        .unwrap();
    let body = ty
        .members
        .iter()
        .find_map(|member| match member {
            Member::Method(method) if method.name == "run" => method.body.as_ref(),
            _ => None,
        })
        .unwrap();
    let Stmt::Return(Some(Expr::New {
        resolved: Some(constructor),
        ..
    })) = &body.statements[0]
    else {
        panic!("expected resolved constructor from external reference path getter");
    };

    assert_eq!(constructor.declaring_type, "Reader");
    assert_eq!(constructor.rust_name, "new_path");
}

#[test]
fn resolver_uses_type_parameter_bound_fields_for_field_access_arguments() {
    let parsed = parse_java_source(
        r#"
class BaseConfig {
    Settings settings;
    boolean enabled;
}

class Settings {
}

class GenericEngine<C extends BaseConfig> {
    C configuration;

    void accept(Settings settings) {}
    void accept(String settings) {}

    void acceptEnabled(boolean enabled) {}
    void acceptEnabled(String enabled) {}

    void run() {
        accept(configuration.settings);
        acceptEnabled(configuration.enabled);
    }
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    assert!(
        !resolved.diagnostics.has_errors(),
        "{:?}",
        resolved.diagnostics
    );

    let unresolved_fields = resolved
        .facts
        .unresolved_target_facts
        .iter()
        .filter(|fact| fact.kind == UnresolvedTargetKind::FieldAccess)
        .collect::<Vec<_>>();
    assert!(
        unresolved_fields.is_empty(),
        "type parameter bound fields should resolve as source fields: {unresolved_fields:?}"
    );

    let ty = &resolved.program.types[2];
    let body = ty
        .members
        .iter()
        .find_map(|member| match member {
            Member::Method(method) if method.name == "run" => method.body.as_ref(),
            _ => None,
        })
        .unwrap();
    let calls = body
        .statements
        .iter()
        .filter_map(|stmt| match stmt {
            Stmt::Expr(Expr::Call {
                resolved: Some(call),
                ..
            }) => Some(call.rust_name.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>();

    assert_eq!(calls, vec!["accept_settings", "accept_enabled_boolean"]);
}

#[test]
fn resolver_uses_nested_type_parameter_bound_field_types_for_chained_access() {
    let parsed = parse_java_source(
        r#"
class BaseConfig {
    Options options;
}

class Options {
    int count;
}

class BoundedConfigurationChain<C extends BaseConfig> {
    C configuration;

    void accept(int value) {}
    void accept(String value) {}

    void run() {
        accept(configuration.options.count);
    }
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    assert!(
        !resolved.diagnostics.has_errors(),
        "{:?}",
        resolved.diagnostics
    );

    let unresolved_fields = resolved
        .facts
        .unresolved_target_facts
        .iter()
        .filter(|fact| fact.kind == UnresolvedTargetKind::FieldAccess)
        .collect::<Vec<_>>();
    assert!(
        unresolved_fields.is_empty(),
        "chained fields through a bounded type parameter should resolve from source bounds: {unresolved_fields:?}"
    );

    let ty = &resolved.program.types[2];
    let body = ty
        .members
        .iter()
        .find_map(|member| match member {
            Member::Method(method) if method.name == "run" => method.body.as_ref(),
            _ => None,
        })
        .unwrap();
    let Stmt::Expr(Expr::Call {
        resolved: Some(call),
        ..
    }) = &body.statements[0]
    else {
        panic!("expected resolved overload from nested bounded field access");
    };

    assert_eq!(call.rust_name, "accept_int");
}

#[test]
fn resolver_uses_method_type_parameter_bound_fields_for_field_access_arguments() {
    let parsed = parse_java_source(
        r#"
class BaseRegion {
    int index;
    String label;
}

class RegionOps {
    static void accept(int value) {}
    static void accept(String value) {}

    static <T extends BaseRegion> void merge(T left, T right) {
        accept(left.index);
        accept(right.label);
    }
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    assert!(
        !resolved.diagnostics.has_errors(),
        "{:?}",
        resolved.diagnostics
    );

    let unresolved_fields = resolved
        .facts
        .unresolved_target_facts
        .iter()
        .filter(|fact| fact.kind == UnresolvedTargetKind::FieldAccess)
        .collect::<Vec<_>>();
    assert!(
        unresolved_fields.is_empty(),
        "method type parameter bound fields should resolve as source fields: {unresolved_fields:?}"
    );

    let ty = &resolved.program.types[1];
    let body = ty
        .members
        .iter()
        .find_map(|member| match member {
            Member::Method(method) if method.name == "merge" => method.body.as_ref(),
            _ => None,
        })
        .unwrap();
    let calls = body
        .statements
        .iter()
        .filter_map(|stmt| match stmt {
            Stmt::Expr(Expr::Call {
                resolved: Some(call),
                ..
            }) => Some(call.rust_name.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>();

    assert_eq!(calls, vec!["accept_int", "accept_string"]);
}

#[test]
fn resolver_uses_shared_type_for_multi_declarator_local_field_access() {
    let parsed = parse_java_source(
        r#"
class Item {
    int count;
    boolean active;
}

class MultiDeclaratorLocalFieldResolution {
    Item first() {
        return null;
    }

    Item last() {
        return null;
    }

    void accept(int value) {}
    void accept(boolean value) {}
    void accept(String value) {}

    void run() {
        final Item head = first(), tail = last();
        accept(head.count);
        accept(tail.active);
    }
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    assert!(
        !resolved.diagnostics.has_errors(),
        "{:?}",
        resolved.diagnostics
    );

    let unresolved_fields = resolved
        .facts
        .unresolved_target_facts
        .iter()
        .filter(|fact| fact.kind == UnresolvedTargetKind::FieldAccess)
        .collect::<Vec<_>>();
    assert!(
        unresolved_fields.is_empty(),
        "all locals from a multi-declarator declaration should keep the shared declared type: {unresolved_fields:?}"
    );

    let ty = &resolved.program.types[1];
    let body = ty
        .members
        .iter()
        .find_map(|member| match member {
            Member::Method(method) if method.name == "run" => method.body.as_ref(),
            _ => None,
        })
        .unwrap();
    let calls = body
        .statements
        .iter()
        .filter_map(|stmt| match stmt {
            Stmt::Expr(Expr::Call {
                resolved: Some(call),
                ..
            }) => Some(call.rust_name.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>();

    assert_eq!(calls, vec!["accept_int", "accept_boolean"]);
}

#[test]
fn resolver_substitutes_inherited_generic_field_type_for_field_access() {
    let parsed = parse_java_source(
        r#"
class Options {
    boolean enabled;
}

class BaseConfig {
    Options options;
}

class Engine<C extends BaseConfig> {
    protected C configuration;
}

class ConcreteEngine extends Engine<BaseConfig> {
    void accept(boolean value) {}
    void accept(String value) {}

    void run() {
        accept(configuration.options.enabled);
    }
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    assert!(
        !resolved.diagnostics.has_errors(),
        "{:?}",
        resolved.diagnostics
    );

    let unresolved_fields = resolved
        .facts
        .unresolved_target_facts
        .iter()
        .filter(|fact| fact.kind == UnresolvedTargetKind::FieldAccess)
        .collect::<Vec<_>>();
    assert!(
        unresolved_fields.is_empty(),
        "inherited generic fields should be substituted through superclass arguments: {unresolved_fields:?}"
    );

    let ty = &resolved.program.types[3];
    let body = ty
        .members
        .iter()
        .find_map(|member| match member {
            Member::Method(method) if method.name == "run" => method.body.as_ref(),
            _ => None,
        })
        .unwrap();
    let Stmt::Expr(Expr::Call {
        resolved: Some(call),
        ..
    }) = &body.statements[0]
    else {
        panic!("expected resolved overload from inherited generic field");
    };

    assert_eq!(call.rust_name, "accept_boolean");
}

#[test]
fn resolver_uses_source_field_access_type_for_chained_call_arguments() {
    let parsed = parse_java_source(
        r#"
class Payload {
    String name() {
        return "payload";
    }
}

class Holder {
    Payload payload;
}

class ChainedFieldCallArgumentResolution {
    void accept(String value) {}
    void accept(int value) {}

    void run(Holder holder) {
        accept(holder.payload.name());
    }
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    assert!(
        !resolved.diagnostics.has_errors(),
        "{:?}",
        resolved.diagnostics
    );

    let Stmt::Expr(Expr::Call {
        resolved: Some(call),
        ..
    }) = first_stmt(&resolved.program.types[2]).unwrap()
    else {
        panic!("expected resolved overload from chained source field call");
    };

    assert_eq!(call.declaring_type, "ChainedFieldCallArgumentResolution");
    assert_eq!(call.rust_name, "accept_string");
}

#[test]
fn resolver_keeps_array_length_resolved_after_source_call_resolution() {
    let parsed = parse_java_source(
        r#"
class ArraySource {
    byte[] bytes() {
        return new byte[0];
    }
}

class ArrayLengthAfterSourceCall {
    int read(ArraySource source) {
        return source.bytes().length;
    }
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    assert!(
        !resolved.diagnostics.has_errors(),
        "{:?}",
        resolved.diagnostics
    );

    let field_facts = resolved
        .facts
        .unresolved_target_facts
        .iter()
        .filter(|fact| fact.kind == UnresolvedTargetKind::FieldAccess)
        .collect::<Vec<_>>();

    assert!(
        field_facts.is_empty(),
        "array length on resolved source call results should not be unresolved: {field_facts:?}"
    );
}

#[test]
fn resolver_treats_unknown_call_result_length_as_java_array_length() {
    let parsed = parse_java_source(
        r#"
class UnknownCallResultLengthResolution {
    void accept(int value) {}
    void accept(String value) {}

    void run(ExternalSource source) {
        accept(source.bytes().length);
    }
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    assert!(
        !resolved.diagnostics.has_errors(),
        "{:?}",
        resolved.diagnostics
    );

    let unresolved_lengths = resolved
        .facts
        .unresolved_target_facts
        .iter()
        .filter(|fact| {
            fact.kind == UnresolvedTargetKind::FieldAccess && fact.target_name == "length"
        })
        .collect::<Vec<_>>();
    assert!(
        unresolved_lengths.is_empty(),
        "`.length` on an unknown call result should be treated as Java array length: {unresolved_lengths:?}"
    );

    let Stmt::Expr(Expr::Call {
        resolved: Some(call),
        ..
    }) = first_stmt(&resolved.program.types[0]).unwrap()
    else {
        panic!("expected resolved overload from unknown call result length");
    };

    assert_eq!(call.declaring_type, "UnknownCallResultLengthResolution");
    assert_eq!(call.rust_name, "accept_int");
}

#[test]
fn resolver_benchmark_reports_unresolved_categories_without_lowering() {
    let root = std::env::temp_dir().join(format!(
        "new_java2rust_resolver_benchmark_{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap();
    std::fs::write(
        root.join("Sample.java"),
        r#"
class Sample {
    void run(External external) {
        external.missing();
        int count = 0;
        count.missingPrimitive();
        int x = external.value;
        MissingExternal created = new MissingExternal("x");
    }
}
"#,
    )
    .unwrap();

    let report = benchmark_java_resolver_dir(&root).unwrap();
    let rendered = render_resolver_benchmark_report(&report);
    std::fs::remove_dir_all(&root).unwrap();

    assert_eq!(report.files_seen, 1);
    assert_eq!(report.files_parsed_without_errors, 1);
    assert_eq!(report.files_resolved_without_errors, 1);
    assert_eq!(report.unresolved_method_calls, 2);
    assert_eq!(report.unresolved_constructors, 1);
    assert_eq!(report.external_api_explained_constructors, 1);
    assert_eq!(report.unexplained_constructors, 0);
    assert_eq!(report.unresolved_field_accesses, 1);
    assert_eq!(report.external_api_explained_field_accesses, 1);
    assert_eq!(report.unexplained_field_accesses, 0);
    assert_eq!(report.unexplained_source_field_accesses, 0);
    assert_eq!(report.unexplained_non_source_field_accesses, 0);
    assert!(rendered.contains("unresolved_method_calls\t2"));
    assert!(rendered.contains("external_api_explained_constructors\t1"));
    assert!(rendered.contains("unexplained_constructors\t0"));
    assert!(rendered.contains("external_api_explained_field_accesses\t1"));
    assert!(rendered.contains("unexplained_field_accesses\t0"));
    assert!(rendered.contains("unexplained_source_field_accesses\t0"));
    assert!(rendered.contains("unexplained_non_source_field_accesses\t0"));
    assert!(rendered.contains("top_unresolved_constructor_targets"));
    assert!(rendered.contains("ConstructorCall\tMissingExternal\t1"));
    assert!(rendered.contains("FieldAccess\tvalue\t1"));
    assert!(rendered.contains("top_unresolved_field_receivers"));
    assert!(rendered.contains("value\tExternal\t1"));
    assert!(rendered.contains("top_unexplained_method_receivers"));
    assert!(rendered.contains("missingPrimitive\t*\t1"));
    assert!(rendered.contains("top_external_api_guess_methods"));
    assert!(rendered.contains("External\tmissing\t1"));
    assert!(rendered.contains("top_external_constructor_guess_types"));
    assert!(rendered.contains("MissingExternal\t1\t1"));
    assert!(rendered.contains("top_external_field_guess_targets"));
    assert!(rendered.contains("External\tvalue\t1"));
    assert!(rendered.contains("top_external_field_guess_contexts"));
    assert!(rendered.contains("value\tExternal\tSample\trun\t1\tSample.java"));
}

#[test]
fn resolver_benchmark_reports_unexplained_field_contexts() {
    let root = std::env::temp_dir().join(format!(
        "new_java2rust_resolver_benchmark_field_context_{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap();
    std::fs::write(
        root.join("ContextSample.java"),
        r#"
class Source {
    int known;

    int compute(Source source) {
        return source.missing;
    }
}
"#,
    )
    .unwrap();

    let report = benchmark_java_resolver_dir(&root).unwrap();
    let rendered = render_resolver_benchmark_report(&report);
    std::fs::remove_dir_all(&root).unwrap();

    assert_eq!(report.files_seen, 1);
    assert_eq!(report.unexplained_field_accesses, 1, "{rendered}");
    assert_eq!(report.top_unexplained_field_contexts.len(), 1);
    let context = &report.top_unexplained_field_contexts[0];
    assert_eq!(context.field_name, "missing");
    assert_eq!(context.receiver_type, "Source");
    assert_eq!(context.enclosing_type, "Source");
    assert_eq!(context.enclosing_member, "compute");
    assert!(rendered.contains("top_unexplained_field_contexts"));
    assert!(rendered.contains("missing\tSource\tSource\tcompute\t1\tContextSample.java"));
}

#[test]
fn resolver_benchmark_uses_transitive_source_field_dependencies() {
    let root = std::env::temp_dir().join(format!(
        "new_java2rust_resolver_benchmark_transitive_source_fields_{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(root.join("consumer")).unwrap();
    std::fs::create_dir_all(root.join("api")).unwrap();
    std::fs::create_dir_all(root.join("model")).unwrap();
    std::fs::write(
        root.join("consumer").join("UseEntry.java"),
        r#"
package consumer;

import api.Entry;

class UseEntry {
    boolean present(Entry entry) {
        return entry.middle.leaf == null;
    }
}
"#,
    )
    .unwrap();
    std::fs::write(
        root.join("api").join("Entry.java"),
        r#"
package api;

import model.Middle;

public class Entry {
    public Middle middle;
}
"#,
    )
    .unwrap();
    std::fs::write(
        root.join("model").join("Middle.java"),
        r#"
package model;

public class Middle {
    public Leaf leaf;
}
"#,
    )
    .unwrap();
    std::fs::write(
        root.join("model").join("Leaf.java"),
        r#"
package model;

public class Leaf {
}
"#,
    )
    .unwrap();

    let report = benchmark_java_resolver_dir(&root).unwrap();
    let rendered = render_resolver_benchmark_report(&report);
    std::fs::remove_dir_all(&root).unwrap();

    assert_eq!(
        report.unresolved_field_accesses, 0,
        "transitive source field dependencies should resolve as source fields: {rendered}"
    );
    assert_eq!(report.external_field_guesses, 0, "{rendered}");
}

#[test]
fn resolver_benchmark_does_not_satisfy_exact_external_import_with_source_simple_name() {
    let root = std::env::temp_dir().join(format!(
        "new_java2rust_resolver_benchmark_external_import_shadow_{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(root.join("consumer")).unwrap();
    std::fs::create_dir_all(root.join("api")).unwrap();
    std::fs::create_dir_all(root.join("shadow")).unwrap();
    std::fs::write(
        root.join("consumer").join("UseEntry.java"),
        r#"
package consumer;

import api.Entry;

class UseEntry {
    void run(Entry entry) {
        entry.externalValue.missing();
    }
}
"#,
    )
    .unwrap();
    std::fs::write(
        root.join("api").join("Entry.java"),
        r#"
package api;

import external.library.Value;

public class Entry {
    public Value externalValue;
}
"#,
    )
    .unwrap();
    std::fs::write(
        root.join("shadow").join("Value.java"),
        r#"
package shadow;

public class Value {
    public int field;
}
"#,
    )
    .unwrap();

    let report = benchmark_java_resolver_dir(&root).unwrap();
    let rendered = render_resolver_benchmark_report(&report);
    std::fs::remove_dir_all(&root).unwrap();

    assert_eq!(report.resolver_diagnostics, 0, "{rendered}");
    assert_eq!(report.unresolved_method_calls, 1, "{rendered}");
    assert_eq!(report.external_api_explained_method_calls, 1, "{rendered}");
    assert_eq!(report.unresolved_field_accesses, 0, "{rendered}");
}

#[test]
fn resolver_benchmark_resolves_record_component_fields_and_canonical_constructor() {
    let root = std::env::temp_dir().join(format!(
        "new_java2rust_resolver_benchmark_record_components_{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap();
    std::fs::write(
        root.join("RecordComponents.java"),
        r#"
class RecordComponents {
    private record ItemStats(int ordinal, Payload payload, long total) { }

    ItemStats make(Payload payload) {
        return new ItemStats(1, payload, 2L);
    }

    Payload read(ItemStats stats) {
        return stats.payload;
    }
}

class Payload {
}
"#,
    )
    .unwrap();

    let report = benchmark_java_resolver_dir(&root).unwrap();
    let rendered = render_resolver_benchmark_report(&report);
    std::fs::remove_dir_all(&root).unwrap();

    assert_eq!(report.parser_diagnostics, 0, "{rendered}");
    assert_eq!(report.resolver_diagnostics, 0, "{rendered}");
    assert_eq!(report.unresolved_constructors, 0, "{rendered}");
    assert_eq!(report.unresolved_field_accesses, 0, "{rendered}");
    assert_eq!(report.external_field_guesses, 0, "{rendered}");
}

#[test]
fn resolver_uses_record_component_accessor_return_type_for_overload_arguments() {
    let parsed = parse_java_source(
        r#"
record ItemStats(int ordinal, Payload payload) {
}

class Payload {
}

class RecordAccessorResolution {
    void accept(Payload payload) {}
    void accept(String payload) {}

    void run(ItemStats stats) {
        accept(stats.payload());
    }
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    assert!(
        !resolved.diagnostics.has_errors(),
        "{:?}",
        resolved.diagnostics
    );

    let ty = resolved
        .program
        .types
        .iter()
        .find(|ty| ty.name == "RecordAccessorResolution")
        .unwrap();
    let Stmt::Expr(Expr::Call {
        resolved: Some(call),
        ..
    }) = first_stmt(ty).unwrap()
    else {
        panic!("expected resolved overload from record component accessor type");
    };

    assert_eq!(call.declaring_type, "RecordAccessorResolution");
    assert_eq!(call.rust_name, "accept_payload");
}

#[test]
fn resolver_benchmark_explains_receiverless_inherited_api_calls() {
    let root = std::env::temp_dir().join(format!(
        "new_java2rust_resolver_benchmark_inherited_{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap();
    std::fs::write(
        root.join("InheritedApi.java"),
        r#"
class InheritedApi {
    Object run(Object value) {
        if (nonNull(value)) {
            return getHeader();
        }
        return null;
    }
}
"#,
    )
    .unwrap();

    let report = benchmark_java_resolver_dir(&root).unwrap();
    let rendered = render_resolver_benchmark_report(&report);
    std::fs::remove_dir_all(&root).unwrap();

    assert_eq!(report.unresolved_method_calls, 2);
    assert_eq!(report.external_api_explained_method_calls, 2);
    assert_eq!(report.unexplained_method_calls, 0);
    assert!(rendered.contains("external_api_explained_method_calls\t2"));
}

#[test]
fn resolver_benchmark_explains_boxed_number_value_extractors() {
    let root = std::env::temp_dir().join(format!(
        "new_java2rust_resolver_benchmark_boxed_number_{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap();
    std::fs::write(
        root.join("BoxedNumberApi.java"),
        r#"
class BoxedNumberApi {
    float format(Number value) {
        return value.floatValue();
    }
}
"#,
    )
    .unwrap();

    let report = benchmark_java_resolver_dir(&root).unwrap();
    let rendered = render_resolver_benchmark_report(&report);
    std::fs::remove_dir_all(&root).unwrap();

    assert_eq!(report.unresolved_method_calls, 1);
    assert_eq!(report.external_api_explained_method_calls, 1);
    assert_eq!(report.unexplained_method_calls, 0, "{rendered}");
    assert!(rendered.contains("external_api_explained_method_calls\t1"));
}

#[test]
fn resolver_benchmark_explains_static_math_special_function_calls() {
    let root = std::env::temp_dir().join(format!(
        "new_java2rust_resolver_benchmark_static_math_{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap();
    std::fs::write(
        root.join("StaticMathApi.java"),
        r#"
import org.apache.commons.math3.special.Beta;

class StaticMathApi {
    double score(double alpha, double beta) {
        return Beta.logBeta(alpha, beta);
    }
}
"#,
    )
    .unwrap();

    let report = benchmark_java_resolver_dir(&root).unwrap();
    let rendered = render_resolver_benchmark_report(&report);
    std::fs::remove_dir_all(&root).unwrap();

    assert_eq!(report.unresolved_method_calls, 1);
    assert_eq!(report.external_api_explained_method_calls, 1);
    assert_eq!(report.unexplained_method_calls, 0, "{rendered}");
    assert!(rendered.contains("external_api_explained_method_calls\t1"));
}

#[test]
fn resolver_benchmark_explains_receiverless_record_collection_accessors() {
    let root = std::env::temp_dir().join(format!(
        "new_java2rust_resolver_benchmark_record_collection_{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap();
    std::fs::write(
        root.join("RecordCollectionAccessor.java"),
        r#"
class RecordCollectionAccessor {
    Object records() {
        return getRecords();
    }
}
"#,
    )
    .unwrap();

    let report = benchmark_java_resolver_dir(&root).unwrap();
    let rendered = render_resolver_benchmark_report(&report);
    std::fs::remove_dir_all(&root).unwrap();

    assert_eq!(report.unresolved_method_calls, 1);
    assert_eq!(report.external_api_explained_method_calls, 1);
    assert_eq!(report.unexplained_method_calls, 0, "{rendered}");
    assert!(rendered.contains("external_api_explained_method_calls\t1"));
}

#[test]
fn resolver_benchmark_reports_external_pack_coverage_for_variantcontext_calls() {
    let root = std::env::temp_dir().join(format!(
        "new_java2rust_resolver_benchmark_variant_pack_{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap();
    std::fs::write(
        root.join("VariantPackApi.java"),
        r#"
import htsjdk.variant.variantcontext.Allele;
import htsjdk.variant.variantcontext.VariantContext;
import htsjdk.variant.variantcontext.VariantContextBuilder;
import java.util.Arrays;

class VariantPackApi {
    VariantContext build(String contig, int start, Allele refAllele, Allele altAllele) {
        return new VariantContextBuilder()
            .chr(contig)
            .start(start)
            .stop(start)
            .alleles(Arrays.asList(refAllele, altAllele))
            .make();
    }

    Object attr(VariantContext vc) {
        return vc.getAttribute("DP");
    }
}
"#,
    )
    .unwrap();

    let report = benchmark_java_resolver_dir(&root).unwrap();
    let rendered = render_resolver_benchmark_report(&report);
    std::fs::remove_dir_all(&root).unwrap();

    assert_eq!(report.resolver_diagnostics, 0, "{rendered}");
    assert!(report.external_pack_api_facts >= 6, "{rendered}");
    assert!(report.external_pack_constructor_facts >= 1, "{rendered}");
    assert!(
        report.external_pack_explained_method_calls >= 6,
        "{rendered}"
    );
    assert!(
        report.external_fact_model_explained_method_calls >= 6,
        "{rendered}"
    );
    assert!(
        report.external_pack_explained_constructors >= 1,
        "{rendered}"
    );
    assert!(
        report.external_fact_model_explained_constructors >= 1,
        "{rendered}"
    );
    assert!(rendered.contains("external_pack_api_facts\t"));
    assert!(rendered.contains("external_pack_constructor_facts\t"));
    assert!(rendered.contains("external_pack_explained_method_calls\t"));
    assert!(rendered.contains("external_pack_explained_constructors\t"));
    assert!(rendered.contains("top_external_pack_api_methods"));
    assert!(
        rendered.contains("VariantContextBuilder\tchr\t1"),
        "{rendered}"
    );
    assert!(
        rendered.contains("VariantContext\tgetAttribute\t1"),
        "{rendered}"
    );
    assert!(rendered.contains("top_external_pack_constructor_types"));
    assert!(
        rendered.contains("VariantContextBuilder\t0\t1"),
        "{rendered}"
    );
}

#[test]
fn resolver_benchmark_reports_external_pack_coverage_for_vcf_header_calls() {
    let root = std::env::temp_dir().join(format!(
        "new_java2rust_resolver_benchmark_vcf_pack_{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap();
    std::fs::write(
        root.join("VcfPackApi.java"),
        r#"
import htsjdk.variant.vcf.VCFHeader;
import htsjdk.variant.vcf.VCFHeaderLine;

class VcfPackApi {
    VCFHeader header() {
        VCFHeader header = new VCFHeader();
        header.addMetaDataLine(new VCFHeaderLine("source", "tool"));
        return header;
    }

    Object samples(VCFHeader header) {
        return header.getGenotypeSamples();
    }

    Object dictionary(VCFHeader header) {
        return header.getSequenceDictionary();
    }
}
"#,
    )
    .unwrap();

    let report = benchmark_java_resolver_dir(&root).unwrap();
    let rendered = render_resolver_benchmark_report(&report);
    std::fs::remove_dir_all(&root).unwrap();

    assert_eq!(report.resolver_diagnostics, 0, "{rendered}");
    assert!(report.external_pack_api_facts >= 3, "{rendered}");
    assert!(report.external_pack_constructor_facts >= 2, "{rendered}");
    assert!(
        report.external_pack_explained_method_calls >= 3,
        "{rendered}"
    );
    assert!(
        report.external_pack_explained_constructors >= 2,
        "{rendered}"
    );
    assert!(rendered.contains("top_external_pack_api_methods"));
    assert!(
        rendered.contains("VCFHeader\taddMetaDataLine\t1"),
        "{rendered}"
    );
    assert!(
        rendered.contains("VCFHeader\tgetGenotypeSamples\t1"),
        "{rendered}"
    );
    assert!(
        rendered.contains("VCFHeader\tgetSequenceDictionary\t1"),
        "{rendered}"
    );
    assert!(rendered.contains("top_external_pack_constructor_types"));
    assert!(rendered.contains("VCFHeader\t0\t1"), "{rendered}");
    assert!(rendered.contains("VCFHeaderLine\t2\t1"), "{rendered}");
}

#[test]
fn resolver_benchmark_reports_external_pack_coverage_for_sam_header_calls() {
    let root = std::env::temp_dir().join(format!(
        "new_java2rust_resolver_benchmark_sam_header_pack_{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap();
    std::fs::write(
        root.join("SamHeaderPackApi.java"),
        r#"
import htsjdk.samtools.SAMFileHeader;
import htsjdk.samtools.SAMReadGroupRecord;
import htsjdk.samtools.SAMSequenceDictionary;
import htsjdk.samtools.SAMSequenceRecord;
import java.util.List;

class SamHeaderPackApi {
    SAMFileHeader header(List<SAMSequenceRecord> records) {
        SAMSequenceDictionary dictionary = new SAMSequenceDictionary(records);
        SAMFileHeader header = new SAMFileHeader();
        header.setSequenceDictionary(dictionary);
        header.addReadGroup(new SAMReadGroupRecord("sample"));
        return header;
    }

    Object dictionary(SAMFileHeader header) {
        return header.getSequenceDictionary();
    }

    Object contig(SAMSequenceDictionary dictionary) {
        return dictionary.getSequence(0);
    }

    Object sample(SAMReadGroupRecord readGroup) {
        return readGroup.getSample();
    }
}
"#,
    )
    .unwrap();

    let report = benchmark_java_resolver_dir(&root).unwrap();
    let rendered = render_resolver_benchmark_report(&report);
    std::fs::remove_dir_all(&root).unwrap();

    assert_eq!(report.resolver_diagnostics, 0, "{rendered}");
    assert!(report.external_pack_api_facts >= 4, "{rendered}");
    assert!(report.external_pack_constructor_facts >= 3, "{rendered}");
    assert!(
        report.external_pack_explained_method_calls >= 4,
        "{rendered}"
    );
    assert!(
        report.external_pack_explained_constructors >= 3,
        "{rendered}"
    );
    assert!(rendered.contains("top_external_pack_api_methods"));
    assert!(
        rendered.contains("SAMFileHeader\tsetSequenceDictionary\t1"),
        "{rendered}"
    );
    assert!(
        rendered.contains("SAMFileHeader\tgetSequenceDictionary\t1"),
        "{rendered}"
    );
    assert!(
        rendered.contains("SAMSequenceDictionary\tgetSequence\t1"),
        "{rendered}"
    );
    assert!(rendered.contains("top_external_pack_constructor_types"));
    assert!(
        rendered.contains("SAMSequenceDictionary\t1\t1"),
        "{rendered}"
    );
    assert!(rendered.contains("SAMFileHeader\t0\t1"), "{rendered}");
    assert!(rendered.contains("SAMReadGroupRecord\t1\t1"), "{rendered}");
}

#[test]
fn resolver_benchmark_reports_external_pack_coverage_for_tribble_feature_calls() {
    let root = std::env::temp_dir().join(format!(
        "new_java2rust_resolver_benchmark_tribble_feature_pack_{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap();
    std::fs::write(
        root.join("TribbleFeaturePackApi.java"),
        r#"
import htsjdk.tribble.Feature;
import htsjdk.tribble.SimpleFeature;

class TribbleFeaturePackApi {
    Feature make() {
        return new SimpleFeature("chr1", 1, 10);
    }

    String contig(Feature feature) {
        return feature.getContig();
    }

    int width(SimpleFeature feature) {
        return feature.getEnd() - feature.getStart();
    }
}
"#,
    )
    .unwrap();

    let report = benchmark_java_resolver_dir(&root).unwrap();
    let rendered = render_resolver_benchmark_report(&report);
    std::fs::remove_dir_all(&root).unwrap();

    assert_eq!(report.resolver_diagnostics, 0, "{rendered}");
    assert!(report.external_pack_api_facts >= 3, "{rendered}");
    assert!(report.external_pack_constructor_facts >= 1, "{rendered}");
    assert!(
        report.external_pack_explained_method_calls >= 3,
        "{rendered}"
    );
    assert!(
        report.external_fact_model_explained_method_calls >= 3,
        "{rendered}"
    );
    assert!(
        report.external_pack_explained_constructors >= 1,
        "{rendered}"
    );
    assert!(
        report.external_fact_model_explained_constructors >= 1,
        "{rendered}"
    );
    assert!(
        rendered.contains("rust_crate_mapping_explained_method_calls\t"),
        "{rendered}"
    );
    assert!(rendered.contains("top_external_pack_api_methods"));
    assert!(rendered.contains("Feature\tgetContig\t1"), "{rendered}");
    assert!(
        rendered.contains("SimpleFeature\tgetStart\t1"),
        "{rendered}"
    );
    assert!(rendered.contains("SimpleFeature\tgetEnd\t1"), "{rendered}");
    assert!(rendered.contains("top_external_pack_constructor_types"));
    assert!(rendered.contains("SimpleFeature\t3\t1"), "{rendered}");
}

#[test]
fn resolver_benchmark_reports_external_pack_coverage_for_samtools_util_calls() {
    let root = std::env::temp_dir().join(format!(
        "new_java2rust_resolver_benchmark_samtools_util_pack_{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap();
    std::fs::write(
        root.join("SamtoolsUtilPackApi.java"),
        r#"
import htsjdk.samtools.util.Interval;
import htsjdk.samtools.util.Locatable;
import htsjdk.samtools.util.PeekableIterator;
import java.util.Iterator;

class SamtoolsUtilPackApi {
    Interval interval(String contig, int start, int end) {
        return new Interval(contig, start, end);
    }

    int width(Locatable value) {
        return value.getEnd() - value.getStart();
    }

    boolean overlaps(Interval left, Interval right) {
        return left.intersects(right);
    }

    Object peek(Iterator<Interval> intervals) {
        return new PeekableIterator<Interval>(intervals).peek();
    }
}
"#,
    )
    .unwrap();

    let report = benchmark_java_resolver_dir(&root).unwrap();
    let rendered = render_resolver_benchmark_report(&report);
    std::fs::remove_dir_all(&root).unwrap();

    assert_eq!(report.resolver_diagnostics, 0, "{rendered}");
    assert!(report.external_pack_api_facts >= 4, "{rendered}");
    assert!(report.external_pack_constructor_facts >= 2, "{rendered}");
    assert!(
        report.external_pack_explained_method_calls >= 4,
        "{rendered}"
    );
    assert!(
        report.external_pack_explained_constructors >= 2,
        "{rendered}"
    );
    assert!(rendered.contains("top_external_pack_api_methods"));
    assert!(rendered.contains("Locatable\tgetStart\t1"), "{rendered}");
    assert!(rendered.contains("Locatable\tgetEnd\t1"), "{rendered}");
    assert!(rendered.contains("Interval\tintersects\t1"), "{rendered}");
    assert!(rendered.contains("PeekableIterator\tpeek\t1"), "{rendered}");
    assert!(rendered.contains("top_external_pack_constructor_types"));
    assert!(rendered.contains("Interval\t3\t1"), "{rendered}");
    assert!(rendered.contains("PeekableIterator\t1\t1"), "{rendered}");
}

#[test]
fn resolver_benchmark_reports_external_pack_coverage_for_hdf5_crate_mapping_calls() {
    let root = std::env::temp_dir().join(format!(
        "new_java2rust_resolver_benchmark_hdf5_pack_{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap();
    std::fs::write(
        root.join("Hdf5PackApi.java"),
        r#"
class Hdf5PackApi {
    void write(File file, double[] values) {
        HDF5File hdf5 = new HDF5File(file, HDF5File.OpenMode.CREATE);
        if (hdf5.exists("/metrics")) {
            hdf5.writeDoubleArray("/metrics", values);
        }
        hdf5.close();
    }
}
"#,
    )
    .unwrap();

    let report = benchmark_java_resolver_dir(&root).unwrap();
    let rendered = render_resolver_benchmark_report(&report);
    std::fs::remove_dir_all(&root).unwrap();

    assert_eq!(report.resolver_diagnostics, 0, "{rendered}");
    assert!(report.external_pack_api_facts >= 3, "{rendered}");
    assert!(report.external_pack_constructor_facts >= 1, "{rendered}");
    assert!(
        report.external_pack_explained_method_calls >= 3,
        "{rendered}"
    );
    assert!(
        report.rust_crate_mapping_explained_method_calls >= 3,
        "{rendered}"
    );
    assert!(
        report.external_pack_explained_constructors >= 1,
        "{rendered}"
    );
    assert!(
        report.rust_crate_mapping_explained_constructors >= 1,
        "{rendered}"
    );
    assert!(rendered.contains("HDF5File\texists\t1"), "{rendered}");
    assert!(
        rendered.contains("HDF5File\twriteDoubleArray\t1"),
        "{rendered}"
    );
    assert!(rendered.contains("HDF5File\tclose\t1"), "{rendered}");
    assert!(rendered.contains("HDF5File\t2\t1"), "{rendered}");
}

#[test]
fn resolver_benchmark_reports_external_pack_coverage_for_sqlite_crate_mapping_calls() {
    let root = std::env::temp_dir().join(format!(
        "new_java2rust_resolver_benchmark_sqlite_pack_{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap();
    std::fs::write(
        root.join("SqlitePackApi.java"),
        r#"
class SqlitePackApi {
    Object connect(String path) {
        SQLiteConfig config = new SQLiteConfig();
        config.setReadOnly(true);
        config.setBusyTimeout(1000);
        return config.createConnection(path);
    }
}
"#,
    )
    .unwrap();

    let report = benchmark_java_resolver_dir(&root).unwrap();
    let rendered = render_resolver_benchmark_report(&report);
    std::fs::remove_dir_all(&root).unwrap();

    assert_eq!(report.resolver_diagnostics, 0, "{rendered}");
    assert!(report.external_pack_api_facts >= 3, "{rendered}");
    assert!(report.external_pack_constructor_facts >= 1, "{rendered}");
    assert!(
        report.external_pack_explained_method_calls >= 3,
        "{rendered}"
    );
    assert!(
        report.external_pack_explained_constructors >= 1,
        "{rendered}"
    );
    assert!(
        rendered.contains("SQLiteConfig\tsetReadOnly\t1"),
        "{rendered}"
    );
    assert!(
        rendered.contains("SQLiteConfig\tsetBusyTimeout\t1"),
        "{rendered}"
    );
    assert!(
        rendered.contains("SQLiteConfig\tcreateConnection\t1"),
        "{rendered}"
    );
    assert!(rendered.contains("SQLiteConfig\t0\t1"), "{rendered}");
}

#[test]
fn resolver_benchmark_resolves_implicit_no_arg_source_constructors() {
    let root = std::env::temp_dir().join(format!(
        "new_java2rust_resolver_benchmark_implicit_ctor_{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap();
    std::fs::write(
        root.join("ImplicitConstructor.java"),
        r#"
class ImplicitConstructorOwner {
    static class Helper {
    }

    Helper make() {
        return new Helper();
    }
}
"#,
    )
    .unwrap();

    let report = benchmark_java_resolver_dir(&root).unwrap();
    let rendered = render_resolver_benchmark_report(&report);
    std::fs::remove_dir_all(&root).unwrap();

    assert_eq!(report.files_seen, 1);
    assert_eq!(report.unresolved_constructors, 0, "{rendered}");
    assert_eq!(report.unexplained_constructors, 0, "{rendered}");
}

#[test]
fn resolver_benchmark_reports_unexplained_source_constructor_targets() {
    let root = std::env::temp_dir().join(format!(
        "new_java2rust_resolver_benchmark_source_ctor_{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap();
    std::fs::write(
        root.join("SourceCtor.java"),
        r#"
class SourceCtor {
    SourceCtor(int value) {}

    void run() {
        SourceCtor wrong = new SourceCtor("x", "y");
    }
}
"#,
    )
    .unwrap();

    let report = benchmark_java_resolver_dir(&root).unwrap();
    let rendered = render_resolver_benchmark_report(&report);
    std::fs::remove_dir_all(&root).unwrap();

    assert_eq!(report.unresolved_constructors, 1);
    assert_eq!(report.external_api_explained_constructors, 0);
    assert_eq!(report.unexplained_constructors, 1);
    assert_eq!(report.top_unexplained_constructor_contexts.len(), 1);
    let context = &report.top_unexplained_constructor_contexts[0];
    assert_eq!(context.target_name, "SourceCtor");
    assert_eq!(context.receiver_type, "SourceCtor");
    assert_eq!(context.arg_types, vec!["String", "String"]);
    assert_eq!(context.enclosing_type, "SourceCtor");
    assert_eq!(context.enclosing_member, "run");
    assert!(rendered.contains("top_unexplained_constructor_targets"));
    assert!(rendered.contains("ConstructorCall\tSourceCtor\t1"));
    assert!(rendered.contains("top_unexplained_constructor_contexts"));
    assert!(
        rendered
            .contains("SourceCtor\tSourceCtor\tString,String\tSourceCtor\trun\t1\tSourceCtor.java")
    );
}

#[test]
fn resolver_benchmark_separates_cross_file_source_constructor_guesses() {
    let root = std::env::temp_dir().join(format!(
        "new_java2rust_resolver_benchmark_cross_file_source_ctor_{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap();
    std::fs::write(
        root.join("SourceRecord.java"),
        r#"
class SourceRecord {
    SourceRecord(int value) {}
}
"#,
    )
    .unwrap();
    std::fs::write(
        root.join("SourceRecordUse.java"),
        r#"
class SourceRecordUse {
    void run() {
        SourceRecord record = new SourceRecord("wrong", "extra");
    }
}
"#,
    )
    .unwrap();

    let report = benchmark_java_resolver_dir(&root).unwrap();
    let rendered = render_resolver_benchmark_report(&report);
    std::fs::remove_dir_all(&root).unwrap();

    assert_eq!(report.files_seen, 2);
    assert_eq!(report.unresolved_constructors, 1, "{rendered}");
    assert_eq!(report.external_api_explained_constructors, 0, "{rendered}");
    assert_eq!(report.unexplained_constructors, 1, "{rendered}");
    assert_eq!(report.unexplained_source_constructors, 1, "{rendered}");
    assert_eq!(report.unexplained_non_source_constructors, 0, "{rendered}");
    assert_eq!(report.external_constructor_guesses, 0, "{rendered}");
    assert_eq!(report.source_constructor_guesses, 1, "{rendered}");
    assert!(rendered.contains("unexplained_source_constructors\t1"));
    assert!(rendered.contains("source_constructor_guesses\t1"));
    assert!(rendered.contains("top_source_constructor_guess_types"));
    assert!(rendered.contains("SourceRecord\t2\t1"));
}

#[test]
fn resolver_benchmark_does_not_mark_explicit_external_import_as_source_constructor() {
    let root = std::env::temp_dir().join(format!(
        "new_java2rust_resolver_benchmark_external_import_ctor_{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap();
    std::fs::write(
        root.join("CollisionBox.java"),
        r#"
package local;

class CollisionBox {
    CollisionBox() {}
}
"#,
    )
    .unwrap();
    std::fs::write(
        root.join("CollisionBoxUse.java"),
        r#"
package local;

import external.metrics.CollisionBox;

class CollisionBoxUse {
    void run() {
        CollisionBox box = new CollisionBox("bin", "value");
    }
}
"#,
    )
    .unwrap();

    let report = benchmark_java_resolver_dir(&root).unwrap();
    let rendered = render_resolver_benchmark_report(&report);
    std::fs::remove_dir_all(&root).unwrap();

    assert_eq!(report.files_seen, 2);
    assert_eq!(report.unresolved_constructors, 1, "{rendered}");
    assert_eq!(report.external_api_explained_constructors, 1, "{rendered}");
    assert_eq!(report.unexplained_constructors, 0, "{rendered}");
    assert_eq!(report.unexplained_source_constructors, 0, "{rendered}");
    assert_eq!(report.external_constructor_guesses, 1, "{rendered}");
    assert_eq!(report.project_constructor_guesses, 0, "{rendered}");
    assert_eq!(report.source_constructor_guesses, 0, "{rendered}");
    assert!(rendered.contains("top_external_constructor_guess_types"));
    assert!(rendered.contains("CollisionBox\t2\t1"));
    assert!(!rendered.contains("top_source_constructor_guess_types"));
}

#[test]
fn resolver_benchmark_marks_missing_same_project_import_as_project_constructor_guess() {
    let root = std::env::temp_dir().join(format!(
        "new_java2rust_resolver_benchmark_project_import_ctor_{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap();
    std::fs::write(
        root.join("UseGeneratedRecord.java"),
        r#"
package demo.usecase;

import demo.generated.GeneratedRecord;

class UseGeneratedRecord {
    void run() {
        GeneratedRecord record = new GeneratedRecord("name", 1);
    }
}
"#,
    )
    .unwrap();

    let report = benchmark_java_resolver_dir(&root).unwrap();
    let rendered = render_resolver_benchmark_report(&report);
    std::fs::remove_dir_all(&root).unwrap();

    assert_eq!(report.files_seen, 1);
    assert_eq!(report.unresolved_constructors, 1, "{rendered}");
    assert_eq!(report.external_api_explained_constructors, 1, "{rendered}");
    assert_eq!(report.external_constructor_guesses, 0, "{rendered}");
    assert_eq!(report.project_constructor_guesses, 1, "{rendered}");
    assert_eq!(report.source_constructor_guesses, 0, "{rendered}");
    assert!(rendered.contains("project_constructor_guesses\t1"));
    assert!(rendered.contains("top_project_constructor_guess_types"));
    assert!(rendered.contains("GeneratedRecord\t2\t1"));
    assert!(!rendered.contains("top_external_constructor_guess_types"));
}

#[test]
fn resolver_benchmark_reports_anonymous_class_constructors_separately() {
    let root = std::env::temp_dir().join(format!(
        "new_java2rust_resolver_benchmark_anonymous_ctor_{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap();
    std::fs::write(
        root.join("AnonymousExternal.java"),
        r#"
class AnonymousExternal {
    Object run() {
        return new ExternalIterator() {
        };
    }
}
"#,
    )
    .unwrap();

    let report = benchmark_java_resolver_dir(&root).unwrap();
    let rendered = render_resolver_benchmark_report(&report);
    std::fs::remove_dir_all(&root).unwrap();

    assert_eq!(report.files_seen, 1);
    assert_eq!(report.unresolved_constructors, 1, "{rendered}");
    assert_eq!(report.anonymous_class_constructors, 1, "{rendered}");
    assert_eq!(report.external_api_explained_constructors, 0, "{rendered}");
    assert_eq!(report.unexplained_constructors, 0, "{rendered}");
    assert_eq!(report.external_constructor_guesses, 0, "{rendered}");
    assert_eq!(report.source_constructor_guesses, 0, "{rendered}");
    assert!(rendered.contains("anonymous_class_constructors\t1"));
    assert!(rendered.contains("top_anonymous_constructor_targets"));
    assert!(rendered.contains("ExternalIterator\t0\t1"));
    assert!(!rendered.contains("top_external_constructor_guess_types"));
}

#[test]
fn resolver_benchmark_resolves_local_class_constructor_inside_anonymous_body() {
    let root = std::env::temp_dir().join(format!(
        "new_java2rust_resolver_benchmark_anon_body_local_ctor_{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap();
    std::fs::write(
        root.join("AnonymousBodyLocal.java"),
        r#"
class Carrier {
    Carrier() {}
}

class AnonymousBodyLocal {
    Carrier run() {
        return new Carrier() {
            class LocalItem {
                LocalItem(String label) {}
            }

            Object make() {
                return new LocalItem("x");
            }
        };
    }
}
"#,
    )
    .unwrap();

    let report = benchmark_java_resolver_dir(&root).unwrap();
    let rendered = render_resolver_benchmark_report(&report);
    std::fs::remove_dir_all(&root).unwrap();

    assert_eq!(report.files_seen, 1);
    assert_eq!(report.unresolved_constructors, 0, "{rendered}");
    assert_eq!(report.external_constructor_guesses, 0, "{rendered}");
    assert_eq!(report.source_constructor_guesses, 0, "{rendered}");
    assert!(!rendered.contains("LocalItem"));
}

#[test]
fn resolver_benchmark_resolves_cross_file_inherited_source_fields() {
    let root = std::env::temp_dir().join(format!(
        "new_java2rust_resolver_benchmark_cross_file_fields_{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap();
    std::fs::write(
        root.join("BaseRecord.java"),
        r#"
class BaseRecord {
    protected int count;
}
"#,
    )
    .unwrap();
    std::fs::write(
        root.join("UsesDerivedRecord.java"),
        r#"
class DerivedRecord extends BaseRecord {
}

class UsesDerivedRecord {
    int read(DerivedRecord record) {
        return record.count;
    }
}
"#,
    )
    .unwrap();

    let report = benchmark_java_resolver_dir(&root).unwrap();
    let rendered = render_resolver_benchmark_report(&report);
    std::fs::remove_dir_all(&root).unwrap();

    assert_eq!(report.files_seen, 2);
    assert_eq!(report.unresolved_field_accesses, 0, "{rendered}");
    assert_eq!(report.unexplained_source_field_accesses, 0, "{rendered}");
}

#[test]
fn resolver_benchmark_resolves_cross_file_inherited_source_methods() {
    let root = std::env::temp_dir().join(format!(
        "new_java2rust_resolver_benchmark_cross_file_methods_{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap();
    std::fs::write(
        root.join("BaseService.java"),
        r#"
class BaseService {
    int size() {
        return 7;
    }
}
"#,
    )
    .unwrap();
    std::fs::write(
        root.join("UsesDerivedService.java"),
        r#"
class DerivedService extends BaseService {
}

class UsesDerivedService {
    int read(DerivedService service) {
        return service.size();
    }
}
"#,
    )
    .unwrap();

    let report = benchmark_java_resolver_dir(&root).unwrap();
    let rendered = render_resolver_benchmark_report(&report);
    std::fs::remove_dir_all(&root).unwrap();

    assert_eq!(report.files_seen, 2);
    assert_eq!(report.unresolved_method_calls, 0, "{rendered}");
    assert_eq!(report.unexplained_method_calls, 0, "{rendered}");
}

#[test]
fn resolver_benchmark_keeps_cross_file_source_symbols_package_scoped() {
    let root = std::env::temp_dir().join(format!(
        "new_java2rust_resolver_benchmark_package_scoped_{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(root.join("left")).unwrap();
    std::fs::create_dir_all(root.join("right")).unwrap();
    std::fs::write(
        root.join("left").join("BaseService.java"),
        r#"
package left;

class BaseService {
    int size() {
        return 7;
    }
}
"#,
    )
    .unwrap();
    std::fs::write(
        root.join("right").join("BaseService.java"),
        r#"
package right;

class BaseService {
    int size(String label) {
        return 9;
    }
}
"#,
    )
    .unwrap();
    std::fs::write(
        root.join("left").join("UsesDerivedService.java"),
        r#"
package left;

class DerivedService extends BaseService {
}

class UsesDerivedService {
    int read(DerivedService service) {
        return service.size();
    }
}
"#,
    )
    .unwrap();

    let report = benchmark_java_resolver_dir(&root).unwrap();
    let rendered = render_resolver_benchmark_report(&report);
    std::fs::remove_dir_all(&root).unwrap();

    assert_eq!(report.files_seen, 3);
    assert_eq!(report.resolver_diagnostics, 0, "{rendered}");
    assert_eq!(report.unresolved_method_calls, 0, "{rendered}");
}

#[test]
fn resolver_benchmark_uses_imported_source_types_for_cross_package_members() {
    let root = std::env::temp_dir().join(format!(
        "new_java2rust_resolver_benchmark_imported_source_{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(root.join("model")).unwrap();
    std::fs::create_dir_all(root.join("usecase")).unwrap();
    std::fs::write(
        root.join("model").join("Span.java"),
        r#"
package demo.model;

class Span {
    String name() {
        return "span";
    }
}
"#,
    )
    .unwrap();
    std::fs::write(
        root.join("model").join("Alignment.java"),
        r#"
package demo.model;

class Alignment {
    Span span;
}
"#,
    )
    .unwrap();
    std::fs::write(
        root.join("usecase").join("UsesImportedAlignment.java"),
        r#"
package demo.usecase;

import demo.model.Alignment;

class UsesImportedAlignment {
    void accept(String value) {}
    void accept(int value) {}

    void run(Alignment alignment) {
        accept(alignment.span.name());
    }
}
"#,
    )
    .unwrap();

    let report = benchmark_java_resolver_dir(&root).unwrap();
    let rendered = render_resolver_benchmark_report(&report);
    std::fs::remove_dir_all(&root).unwrap();

    assert_eq!(report.files_seen, 3);
    assert_eq!(report.resolver_diagnostics, 0, "{rendered}");
    assert_eq!(report.unresolved_field_accesses, 0, "{rendered}");
    assert_eq!(report.unresolved_method_calls, 0, "{rendered}");
}

#[test]
fn resolver_benchmark_resolves_wildcard_imported_source_static_fields() {
    let root = std::env::temp_dir().join(format!(
        "new_java2rust_resolver_benchmark_wildcard_static_field_{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(root.join("model")).unwrap();
    std::fs::create_dir_all(root.join("usecase")).unwrap();
    std::fs::write(
        root.join("model").join("Location.java"),
        r#"
package demo.model;

class Location {
    static final Location UNMAPPED = new Location();
}
"#,
    )
    .unwrap();
    std::fs::write(
        root.join("usecase").join("UsesWildcardStaticField.java"),
        r#"
package demo.usecase;

import demo.model.*;

class UsesWildcardStaticField {
    boolean run(List<Location> intervals) {
        return intervals.contains(Location.UNMAPPED);
    }
}
"#,
    )
    .unwrap();

    let report = benchmark_java_resolver_dir(&root).unwrap();
    let rendered = render_resolver_benchmark_report(&report);
    std::fs::remove_dir_all(&root).unwrap();

    assert_eq!(report.files_seen, 2);
    assert_eq!(report.resolver_diagnostics, 0, "{rendered}");
    assert_eq!(report.unexplained_field_accesses, 0, "{rendered}");
}

#[test]
fn resolver_resolves_static_field_on_qualified_nested_source_type() {
    let parsed = parse_java_source(
        r#"
class Outer {
    static final class Bounds {
        public final static int SENTINEL = 7;
    }
}

class UsesQualifiedNestedStaticField {
    void accept(int value) {}
    void accept(String value) {}

    void run() {
        accept(Outer.Bounds.SENTINEL);
    }
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    assert!(
        !resolved.diagnostics.has_errors(),
        "{:?}",
        resolved.diagnostics
    );

    let field_facts = resolved
        .facts
        .unresolved_target_facts
        .iter()
        .filter(|fact| fact.kind == UnresolvedTargetKind::FieldAccess)
        .collect::<Vec<_>>();
    assert!(field_facts.is_empty(), "{field_facts:?}");

    let ty = resolved
        .program
        .types
        .iter()
        .find(|ty| ty.name == "UsesQualifiedNestedStaticField")
        .unwrap();
    let Stmt::Expr(Expr::Call {
        resolved: Some(call),
        ..
    }) = first_stmt(ty).unwrap()
    else {
        panic!("expected resolved overload from qualified nested static field");
    };
    assert_eq!(call.rust_name, "accept_int");
}

#[test]
fn resolver_prefers_type_qualified_nested_static_member_over_snake_case_local_collision() {
    let parsed = parse_java_source(
        r#"
class ReadMetadata {
    Bounds[] bounds;

    static final class Bounds {
        public final static int UNMAPPED = 7;
    }
}

class UsesQualifiedNestedStaticFieldWithCollision {
    void accept(int value) {}
    void accept(String value) {}

    void run(ReadMetadata readMetadata) {
        accept(ReadMetadata.Bounds.UNMAPPED);
    }
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    assert!(
        !resolved.diagnostics.has_errors(),
        "{:?}",
        resolved.diagnostics
    );

    let field_facts = resolved
        .facts
        .unresolved_target_facts
        .iter()
        .filter(|fact| fact.kind == UnresolvedTargetKind::FieldAccess)
        .collect::<Vec<_>>();
    assert!(field_facts.is_empty(), "{field_facts:?}");

    let ty = resolved
        .program
        .types
        .iter()
        .find(|ty| ty.name == "UsesQualifiedNestedStaticFieldWithCollision")
        .unwrap();
    let Stmt::Expr(Expr::Call {
        resolved: Some(call),
        ..
    }) = first_stmt(ty).unwrap()
    else {
        panic!("expected resolved overload from type-qualified nested static field");
    };
    assert_eq!(call.rust_name, "accept_int");
}

#[test]
fn resolver_prefers_nested_type_static_member_over_same_named_instance_field() {
    let parsed = parse_java_source(
        r#"
class Metadata {
    Bounds[] bounds;

    static final class Bounds {
        public final static int UNMAPPED = 7;
    }

    int run() {
        return Bounds.UNMAPPED;
    }
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    assert!(
        !resolved.diagnostics.has_errors(),
        "{:?}",
        resolved.diagnostics
    );

    let field_facts = resolved
        .facts
        .unresolved_target_facts
        .iter()
        .filter(|fact| fact.kind == UnresolvedTargetKind::FieldAccess)
        .collect::<Vec<_>>();
    assert!(field_facts.is_empty(), "{field_facts:?}");
}

#[test]
fn resolver_treats_multidimensional_array_element_length_as_int() {
    let parsed = parse_java_source(
        r#"
class MatrixLengthResolution {
    long[][] totals;

    void accept(int value) {}
    void accept(String value) {}

    void run(MatrixLengthResolution other, int row) {
        accept(other.totals[row].length);
    }
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    assert!(
        !resolved.diagnostics.has_errors(),
        "{:?}",
        resolved.diagnostics
    );

    let field_facts = resolved
        .facts
        .unresolved_target_facts
        .iter()
        .filter(|fact| fact.kind == UnresolvedTargetKind::FieldAccess)
        .collect::<Vec<_>>();
    assert!(field_facts.is_empty(), "{field_facts:?}");

    let Stmt::Expr(Expr::Call {
        resolved: Some(call),
        ..
    }) = first_stmt(&resolved.program.types[0]).unwrap()
    else {
        panic!("expected resolved overload from multidimensional array element length");
    };
    assert_eq!(call.rust_name, "accept_int");
}

#[test]
fn resolver_treats_initialized_multidimensional_array_field_element_length_as_int() {
    let parsed = parse_java_source(
        r#"
class MatrixMerge {
    private static final int ROWS = 5;
    private int max = 0;
    private final long[][] totals = new long[ROWS][max];

    void accept(int value) {}
    void accept(String value) {}

    void merge(MatrixMerge other, int row) {
        accept(other.totals[row].length);
    }
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    assert!(
        !resolved.diagnostics.has_errors(),
        "{:?}",
        resolved.diagnostics
    );

    let field_facts = resolved
        .facts
        .unresolved_target_facts
        .iter()
        .filter(|fact| fact.kind == UnresolvedTargetKind::FieldAccess)
        .collect::<Vec<_>>();
    assert!(field_facts.is_empty(), "{field_facts:?}");

    let Stmt::Expr(Expr::Call {
        resolved: Some(call),
        ..
    }) = first_stmt(&resolved.program.types[0]).unwrap()
    else {
        panic!("expected resolved overload from initialized multidimensional array field length");
    };
    assert_eq!(call.rust_name, "accept_int");
}

#[test]
fn resolver_treats_multidimensional_array_row_length_in_for_condition_as_int() {
    let parsed = parse_java_source(
        r#"
class MatrixLoop {
    private static final int ROWS = 5;
    private int max = 0;
    private final long[][] totals = new long[ROWS][max];

    void accept(int value) {}
    void accept(String value) {}

    void merge(MatrixLoop other) {
        for (int i = 0; i < ROWS; i++) {
            for (int j = 0; j < other.totals[i].length; j++) {
                accept(other.totals[i].length);
            }
        }
    }
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    assert!(
        !resolved.diagnostics.has_errors(),
        "{:?}",
        resolved.diagnostics
    );

    let field_facts = resolved
        .facts
        .unresolved_target_facts
        .iter()
        .filter(|fact| fact.kind == UnresolvedTargetKind::FieldAccess)
        .collect::<Vec<_>>();
    assert!(field_facts.is_empty(), "{field_facts:?}");
}

#[test]
fn resolver_benchmark_handles_nested_multidimensional_array_row_length() {
    let root = std::env::temp_dir().join(format!(
        "new_java2rust_resolver_benchmark_nested_matrix_length_{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(root.join("metrics")).unwrap();
    std::fs::write(
        root.join("metrics").join("MetricTool.java"),
        r#"
package demo.metrics;

class MetricTool {
    private static final class Histogram {
        private static final int ROWS = 5;
        private int max = 0;
        private final long[][] totals = new long[ROWS][max];

        Histogram merge(Histogram other) {
            for (int i = 0; i < ROWS; i++) {
                for (int j = 0; j < other.totals[i].length; j++) {
                    totals[i][j] += other.totals[i][j];
                }
            }
            return this;
        }
    }
}
"#,
    )
    .unwrap();

    let report = benchmark_java_resolver_dir(&root).unwrap();
    let rendered = render_resolver_benchmark_report(&report);
    std::fs::remove_dir_all(&root).unwrap();

    assert_eq!(report.files_seen, 1);
    assert_eq!(report.resolver_diagnostics, 0, "{rendered}");
    assert_eq!(report.unexplained_field_accesses, 0, "{rendered}");
}

#[test]
fn resolver_benchmark_prefers_current_file_nested_type_scope_on_same_package_collisions() {
    let root = std::env::temp_dir().join(format!(
        "new_java2rust_resolver_benchmark_nested_scope_collision_{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(root.join("metrics")).unwrap();
    std::fs::write(
        root.join("metrics").join("CurrentMetric.java"),
        r#"
package demo.metrics;

class CurrentMetric {
    private static final class ReusedName {
        private static final int ROWS = 5;
        private final long[][] totalsByCycle = new long[ROWS][0];

        ReusedName merge(final ReusedName other) {
            for (int i = 0; i < ROWS; i++) {
                for (int j = 0; j < other.totalsByCycle[i].length; j++) {
                    this.totalsByCycle[i][j] += other.totalsByCycle[i][j];
                }
            }
            return this;
        }
    }
}
"#,
    )
    .unwrap();
    std::fs::write(
        root.join("metrics").join("OtherMetric.java"),
        r#"
package demo.metrics;

class OtherMetric {
    private static final class ReusedName {
        private final double[] totalsByCycle = new double[0];
    }
}
"#,
    )
    .unwrap();

    let report = benchmark_java_resolver_dir(&root).unwrap();
    let rendered = render_resolver_benchmark_report(&report);
    std::fs::remove_dir_all(&root).unwrap();

    assert_eq!(report.files_seen, 2);
    assert_eq!(report.resolver_diagnostics, 0, "{rendered}");
    assert_eq!(report.unexplained_field_accesses, 0, "{rendered}");
}

#[test]
fn resolver_treats_camel_case_multidimensional_source_field_row_length_as_int() {
    let parsed = parse_java_source(
        r#"
class MatrixHolder {
    private static final class Accumulator implements Serializable {
        private static final int ROWS = 5;
        private final long[][] firstReadTotalsByCycle = new long[ROWS][0];
        private final long[][] secondReadTotalsByCycle = new long[ROWS][0];

        public Accumulator merge(final Accumulator other) {
            int total = 0;
            for (int i = 0; i < ROWS; i++) {
                for (int j = 0; j < other.firstReadTotalsByCycle[i].length; j++) {
                    this.firstReadTotalsByCycle[i][j] += other.firstReadTotalsByCycle[i][j];
                }
                for (int j = 0; j < other.secondReadTotalsByCycle[i].length; j++) {
                    this.secondReadTotalsByCycle[i][j] += other.secondReadTotalsByCycle[i][j];
                }
            }
            return this;
        }
    }
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    assert!(
        !resolved.diagnostics.has_errors(),
        "{:?}",
        resolved.diagnostics
    );

    let field_facts = resolved
        .facts
        .unresolved_target_facts
        .iter()
        .filter(|fact| fact.kind == UnresolvedTargetKind::FieldAccess)
        .collect::<Vec<_>>();
    assert!(field_facts.is_empty(), "{field_facts:?}");
}

#[test]
fn resolver_uses_qualified_generic_inner_cast_type_for_field_lookup() {
    let parsed = parse_java_source(
        r#"
class Tree<T> {
    class Node {}

    abstract class IteratorBase {
        protected Node next;

        boolean same(Object obj) {
            return ((Tree<?>.IteratorBase)obj).next == next;
        }
    }
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    assert!(
        !resolved.diagnostics.has_errors(),
        "{:?}",
        resolved.diagnostics
    );

    let field_facts = resolved
        .facts
        .unresolved_target_facts
        .iter()
        .filter(|fact| fact.kind == UnresolvedTargetKind::FieldAccess)
        .collect::<Vec<_>>();
    assert!(field_facts.is_empty(), "{field_facts:?}");
}

#[test]
fn resolver_benchmark_resolves_annotated_nested_source_static_fields_across_files() {
    let root = std::env::temp_dir().join(format!(
        "new_java2rust_resolver_benchmark_annotated_nested_static_field_{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(root.join("model")).unwrap();
    std::fs::write(
        root.join("model").join("Outer.java"),
        r#"
package demo.model;

class Outer {
    @SerializableShape(Outer.Bounds.class)
    public static final class Bounds {
        public final static int SENTINEL = 7;
    }
}
"#,
    )
    .unwrap();
    std::fs::write(
        root.join("model")
            .join("UsesQualifiedNestedStaticField.java"),
        r#"
package demo.model;

class UsesQualifiedNestedStaticField {
    boolean run(List<Integer> values) {
        return values.contains(Outer.Bounds.SENTINEL);
    }
}
"#,
    )
    .unwrap();

    let report = benchmark_java_resolver_dir(&root).unwrap();
    let rendered = render_resolver_benchmark_report(&report);
    std::fs::remove_dir_all(&root).unwrap();

    assert_eq!(report.files_seen, 2);
    assert_eq!(report.resolver_diagnostics, 0, "{rendered}");
    assert_eq!(report.unexplained_field_accesses, 0, "{rendered}");
}

#[test]
fn resolver_benchmark_resolves_imported_nested_source_instance_fields_across_files() {
    let root = std::env::temp_dir().join(format!(
        "new_java2rust_resolver_benchmark_imported_nested_instance_field_{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(root.join("model")).unwrap();
    std::fs::create_dir_all(root.join("usecase")).unwrap();
    std::fs::write(
        root.join("model").join("Outer.java"),
        r#"
package demo.model;

class Outer {
    public static class Params {
        public String name;
        public int padding;
    }
}
"#,
    )
    .unwrap();
    std::fs::write(
        root.join("usecase").join("UsesImportedNested.java"),
        r#"
package demo.usecase;

import demo.model.Outer.Params;

class UsesImportedNested {
    void accept(String value) {}
    void accept(int value) {}

    void run(Params params) {
        accept(params.name);
        accept(params.padding);
    }
}
"#,
    )
    .unwrap();

    let report = benchmark_java_resolver_dir(&root).unwrap();
    let rendered = render_resolver_benchmark_report(&report);
    std::fs::remove_dir_all(&root).unwrap();

    assert_eq!(report.files_seen, 2);
    assert_eq!(report.resolver_diagnostics, 0, "{rendered}");
    assert_eq!(report.unexplained_field_accesses, 0, "{rendered}");
    assert_eq!(report.unexplained_method_calls, 0, "{rendered}");
}

#[test]
fn resolver_benchmark_does_not_import_incidental_nested_package_types() {
    let root = std::env::temp_dir().join(format!(
        "new_java2rust_resolver_benchmark_incidental_nested_package_type_{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(root.join("lib")).unwrap();
    std::fs::create_dir_all(root.join("usecase")).unwrap();
    std::fs::write(
        root.join("lib").join("Marker.java"),
        r#"
package demo.lib;

class Marker {}
"#,
    )
    .unwrap();
    std::fs::write(
        root.join("lib").join("Container.java"),
        r#"
package demo.lib;

class Container {
    static class Writer {
        Writer(int value) {}
    }
}
"#,
    )
    .unwrap();
    std::fs::write(
        root.join("usecase").join("UseExternalWriter.java"),
        r#"
package demo.usecase;

import demo.lib.Marker;

class UseExternalWriter {
    Object run(Source source) {
        return new Writer(source.name());
    }
}
"#,
    )
    .unwrap();

    let report = benchmark_java_resolver_dir(&root).unwrap();
    let rendered = render_resolver_benchmark_report(&report);
    std::fs::remove_dir_all(&root).unwrap();

    assert_eq!(report.files_seen, 3);
    assert_eq!(report.resolver_diagnostics, 0, "{rendered}");
    assert_eq!(report.unexplained_constructors, 0, "{rendered}");
}

#[test]
fn resolver_benchmark_resolves_nested_source_constructor_with_sibling_functional_interface() {
    let root = std::env::temp_dir().join(format!(
        "new_java2rust_resolver_benchmark_nested_ctor_sibling_functional_{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(root.join("model")).unwrap();
    std::fs::create_dir_all(root.join("usecase")).unwrap();
    std::fs::write(
        root.join("model").join("Outer.java"),
        r#"
package demo.model;

class Outer {
    @FunctionalInterface
    interface WriteFunc<T> {
        void write(T item, Writer<T> writer);
    }

    static class Writer<T> {
        Writer(Path path, Header header, WriteFunc<T> writeFunc, int level) {}
        Writer(String name, Output output, Header header, WriteFunc<T> writeFunc) {}
    }
}
"#,
    )
    .unwrap();
    std::fs::write(
        root.join("usecase").join("UseNestedWriter.java"),
        r#"
package demo.usecase;

import demo.model.Outer.Writer;

class UseNestedWriter {
    void encode(Item item, Writer<Item> writer) {}

    Writer<Item> run(Path path, Header header, int level) {
        return new Writer<>(path, header, this::encode, level);
    }
}
"#,
    )
    .unwrap();

    let report = benchmark_java_resolver_dir(&root).unwrap();
    let rendered = render_resolver_benchmark_report(&report);
    std::fs::remove_dir_all(&root).unwrap();

    assert_eq!(report.files_seen, 2);
    assert_eq!(report.resolver_diagnostics, 0, "{rendered}");
    assert_eq!(report.unresolved_constructors, 0, "{rendered}");
}

#[test]
fn resolver_benchmark_prefers_explicit_import_over_incidental_package_type_collision() {
    let root = std::env::temp_dir().join(format!(
        "new_java2rust_resolver_benchmark_import_precedence_{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(root.join("left")).unwrap();
    std::fs::create_dir_all(root.join("right")).unwrap();
    std::fs::create_dir_all(root.join("usecase")).unwrap();
    std::fs::write(
        root.join("left").join("Marker.java"),
        r#"
package demo.left;

class Marker {}
"#,
    )
    .unwrap();
    std::fs::write(
        root.join("left").join("Factory.java"),
        r#"
package demo.left;

class Factory {
}
"#,
    )
    .unwrap();
    std::fs::write(
        root.join("right").join("Factory.java"),
        r#"
package demo.right;

class Factory {
    Factory(String value) {}
}
"#,
    )
    .unwrap();
    std::fs::write(
        root.join("usecase").join("UseFactory.java"),
        r#"
package demo.usecase;

import demo.left.Marker;
import demo.right.Factory;

class UseFactory {
    Factory run() {
        return new Factory("ok");
    }
}
"#,
    )
    .unwrap();

    let report = benchmark_java_resolver_dir(&root).unwrap();
    let rendered = render_resolver_benchmark_report(&report);
    std::fs::remove_dir_all(&root).unwrap();

    assert_eq!(report.files_seen, 4);
    assert_eq!(report.resolver_diagnostics, 0, "{rendered}");
    assert_eq!(report.unresolved_constructors, 0, "{rendered}");
    assert_eq!(report.unexplained_constructors, 0, "{rendered}");
}

#[test]
fn resolver_benchmark_resolves_imported_constructor_from_map_entry_value() {
    let root = std::env::temp_dir().join(format!(
        "new_java2rust_resolver_benchmark_imported_map_entry_{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(root.join("model")).unwrap();
    std::fs::create_dir_all(root.join("usecase")).unwrap();
    std::fs::write(
        root.join("model").join("Report.java"),
        r#"
package demo.model;

class Report {
    Report(File file) {}
    Report(InputStream input) {}
}
"#,
    )
    .unwrap();
    std::fs::write(
        root.join("usecase").join("UseReport.java"),
        r#"
package demo.usecase;

import demo.model.Report;

class UseReport {
    void put(String key, Report report) {}

    void run(Map<String, File> reportFiles) {
        for (final Map.Entry<String,File> entry : reportFiles.entrySet()) {
            put(entry.getKey(), new Report(entry.getValue()));
        }
    }
}
"#,
    )
    .unwrap();

    let report = benchmark_java_resolver_dir(&root).unwrap();
    let rendered = render_resolver_benchmark_report(&report);
    std::fs::remove_dir_all(&root).unwrap();

    assert_eq!(report.files_seen, 2);
    assert_eq!(report.resolver_diagnostics, 0, "{rendered}");
}

#[test]
fn resolver_benchmark_resolves_imported_multi_arg_constructor_from_map_entry_value() {
    let root = std::env::temp_dir().join(format!(
        "new_java2rust_resolver_benchmark_imported_map_entry_pair_{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(root.join("model")).unwrap();
    std::fs::create_dir_all(root.join("usecase")).unwrap();
    std::fs::write(
        root.join("model").join("EvidenceBatch.java"),
        r#"
package demo.model;

class Key {}
class Evidence {}

class EvidenceBatch {
    EvidenceBatch(Key key, Iterable<Evidence> values) {}
    EvidenceBatch(Key left, Key right) {}
}
"#,
    )
    .unwrap();
    std::fs::write(
        root.join("usecase").join("UseEvidenceBatch.java"),
        r#"
package demo.usecase;

import demo.model.EvidenceBatch;
import demo.model.Evidence;
import demo.model.Key;

class UseEvidenceBatch {
    void add(EvidenceBatch batch) {}

    void run(Map<Key, List<Evidence>> values) {
        for (final Map.Entry<Key, List<Evidence>> entry : values.entrySet()) {
            add(new EvidenceBatch(entry.getKey(), entry.getValue()));
        }
    }
}
"#,
    )
    .unwrap();

    let report = benchmark_java_resolver_dir(&root).unwrap();
    let rendered = render_resolver_benchmark_report(&report);
    std::fs::remove_dir_all(&root).unwrap();

    assert_eq!(report.files_seen, 2);
    assert_eq!(report.resolver_diagnostics, 0, "{rendered}");
}

#[test]
fn resolver_benchmark_does_not_let_source_entry_shadow_map_entry_value_type() {
    let root = std::env::temp_dir().join(format!(
        "new_java2rust_resolver_benchmark_map_entry_shadow_{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(root.join("model")).unwrap();
    std::fs::create_dir_all(root.join("usecase")).unwrap();
    std::fs::create_dir_all(root.join("util")).unwrap();
    std::fs::write(
        root.join("model").join("EvidenceBatch.java"),
        r#"
package demo.model;

class Key {}
class Evidence {}

class EvidenceBatch {
    EvidenceBatch(Key key, Iterable<Evidence> values) {}
    EvidenceBatch(Key left, Key right) {}
}
"#,
    )
    .unwrap();
    std::fs::write(
        root.join("util").join("Tree.java"),
        r#"
package demo.util;

class Tree<V> {
    interface Entry<V1> {
        V1 getValue();
    }
}
"#,
    )
    .unwrap();
    std::fs::write(
        root.join("usecase").join("UseEvidenceBatch.java"),
        r#"
package demo.usecase;

import demo.model.EvidenceBatch;
import demo.model.Evidence;
import demo.model.Key;
import demo.util.Tree;

class UseEvidenceBatch {
    void add(EvidenceBatch batch) {}

    void run(Map<Key, List<Evidence>> values) {
        for (final Map.Entry<Key, List<Evidence>> entry : values.entrySet()) {
            add(new EvidenceBatch(entry.getKey(), entry.getValue()));
        }
    }
}
"#,
    )
    .unwrap();

    let report = benchmark_java_resolver_dir(&root).unwrap();
    let rendered = render_resolver_benchmark_report(&report);
    std::fs::remove_dir_all(&root).unwrap();

    assert_eq!(report.files_seen, 3);
    assert_eq!(report.resolver_diagnostics, 0, "{rendered}");
}

#[test]
fn resolver_benchmark_uses_imported_generic_source_iterator_return_type() {
    let root = std::env::temp_dir().join(format!(
        "new_java2rust_resolver_benchmark_imported_generic_iterator_{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(root.join("model")).unwrap();
    std::fs::create_dir_all(root.join("usecase")).unwrap();
    std::fs::write(
        root.join("model").join("RecordSource.java"),
        r#"
package model;

interface Feature {
}

class RecordSource<T extends Feature> {
    RecordSource(String path, Object featureInput, int lookahead, Object cloudPrefetchBuffer) {}

    Iterator<T> iterator() {
        return null;
    }
}

class Stream<T> {
    void forEach(Consumer<T> consumer) {}
}

class Utils {
    static <T> Stream<T> stream(Iterable<T> values) {
        return null;
    }

    static <T> Stream<T> stream(Iterator<T> values) {
        return null;
    }
}

class Record implements Feature {
}
"#,
    )
    .unwrap();
    std::fs::write(
        root.join("usecase").join("UseRecordSource.java"),
        r#"
package usecase;

import model.Record;
import model.RecordSource;
import model.Utils;

class UseRecordSource {
    void consume(Record record) {}

    void run(String path) {
        try (RecordSource<Record> source = new RecordSource<>(path, null, 0, null)) {
            Utils.stream(source.iterator()).forEach(record -> consume(record));
        }
    }
}
"#,
    )
    .unwrap();

    let report = benchmark_java_resolver_dir(&root).unwrap();
    let rendered = render_resolver_benchmark_report(&report);
    std::fs::remove_dir_all(&root).unwrap();

    assert_eq!(report.files_seen, 2);
    assert_eq!(report.resolver_diagnostics, 0, "{rendered}");
}

#[test]
fn resolver_benchmark_prefers_overriding_source_method_over_super_method() {
    let root = std::env::temp_dir().join(format!(
        "new_java2rust_resolver_benchmark_override_{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap();
    std::fs::write(
        root.join("NamedBase.java"),
        r#"
class NamedBase {
    String getName() {
        return "base";
    }
}
"#,
    )
    .unwrap();
    std::fs::write(
        root.join("NamedChild.java"),
        r#"
class NamedChild extends NamedBase {
    String getName() {
        return "child";
    }
}

class UsesNamedChild {
    String read(NamedChild child) {
        return child.getName();
    }
}
"#,
    )
    .unwrap();

    let report = benchmark_java_resolver_dir(&root).unwrap();
    let rendered = render_resolver_benchmark_report(&report);
    std::fs::remove_dir_all(&root).unwrap();

    assert_eq!(report.files_seen, 2);
    assert_eq!(report.resolver_diagnostics, 0, "{rendered}");
    assert_eq!(report.unresolved_method_calls, 0, "{rendered}");
}

#[test]
fn resolver_matches_lambda_arguments_to_source_functional_interface_parameters() {
    let parsed = parse_java_source(
        r#"
interface MessageSupplier {
    String get();
}

class LambdaOverloadResolution {
    void validate(boolean condition, String message) {}
    void validate(boolean condition, MessageSupplier message) {}

    void run(boolean ok) {
        validate(ok, () -> "bad");
    }
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    assert!(
        !resolved.diagnostics.has_errors(),
        "{:?}",
        resolved.diagnostics
    );

    let ty = &resolved.program.types[1];
    let body = ty
        .members
        .iter()
        .find_map(|member| match member {
            Member::Method(method) if method.name == "run" => method.body.as_ref(),
            _ => None,
        })
        .unwrap();
    let Stmt::Expr(Expr::Call {
        resolved: Some(call),
        ..
    }) = &body.statements[0]
    else {
        panic!("expected resolved lambda overload call");
    };

    assert_eq!(call.declaring_type, "LambdaOverloadResolution");
    assert_eq!(call.rust_name, "validate_boolean_message_supplier");
}

#[test]
fn resolver_matches_lambda_arguments_to_known_external_functional_interfaces() {
    let parsed = parse_java_source(
        r#"
class ExternalLambdaOverloadResolution {
    void apply(double[] values, String label) {}
    void apply(double[] values, DoubleUnaryOperator op) {}

    void run(double[] values) {
        apply(values, x -> x + 1.0);
    }
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    assert!(
        !resolved.diagnostics.has_errors(),
        "{:?}",
        resolved.diagnostics
    );

    let ty = &resolved.program.types[0];
    let body = ty
        .members
        .iter()
        .find_map(|member| match member {
            Member::Method(method) if method.name == "run" => method.body.as_ref(),
            _ => None,
        })
        .unwrap();
    let Stmt::Expr(Expr::Call {
        resolved: Some(call),
        ..
    }) = &body.statements[0]
    else {
        panic!("expected resolved external functional-interface overload call");
    };

    assert_eq!(call.declaring_type, "ExternalLambdaOverloadResolution");
    assert_eq!(call.rust_name, "apply_double_array_double_unary_operator");
}

#[test]
fn resolver_matches_lambda_arguments_to_known_iterable_functional_interface() {
    let parsed = parse_java_source(
        r#"
class IterableLambdaOverloadResolution {
    <T> Stream<T> stream(Iterable<T> values) {
        return null;
    }

    <T> Stream<T> stream(Iterator<T> values) {
        return null;
    }

    Stream<Item> run(Iterator<Item> iterator) {
        return stream(() -> iterator);
    }
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    assert!(
        !resolved.diagnostics.has_errors(),
        "{:?}",
        resolved.diagnostics
    );

    let ty = &resolved.program.types[0];
    let body = ty
        .members
        .iter()
        .find_map(|member| match member {
            Member::Method(method) if method.name == "run" => method.body.as_ref(),
            _ => None,
        })
        .unwrap();
    let Stmt::Return(Some(Expr::Call {
        resolved: Some(call),
        ..
    })) = &body.statements[0]
    else {
        panic!("expected resolved Iterable lambda overload call");
    };

    assert_eq!(call.declaring_type, "IterableLambdaOverloadResolution");
    assert_eq!(call.rust_name, "stream_iterable");
}

#[test]
fn resolver_matches_lambdas_to_known_external_primitive_function_interfaces() {
    let parsed = parse_java_source(
        r#"
class PrimitiveFunctionLambdaOverloadResolution {
    double[] transform(int[] values, String label) { return null; }
    double[] transform(int[] values, IntToDoubleFunction op) { return null; }

    double[] run(int[] values) {
        return transform(values, index -> index + 0.5);
    }
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    assert!(
        !resolved.diagnostics.has_errors(),
        "{:?}",
        resolved.diagnostics
    );

    let ty = &resolved.program.types[0];
    let body = ty
        .members
        .iter()
        .find_map(|member| match member {
            Member::Method(method) if method.name == "run" => method.body.as_ref(),
            _ => None,
        })
        .unwrap();
    let Stmt::Return(Some(Expr::Call {
        resolved: Some(call),
        ..
    })) = &body.statements[0]
    else {
        panic!("expected resolved primitive function lambda overload call");
    };

    assert_eq!(
        call.declaring_type,
        "PrimitiveFunctionLambdaOverloadResolution"
    );
    assert_eq!(call.rust_name, "transform_int_array_int_to_double_function");
}

#[test]
fn resolver_matches_method_references_to_source_functional_interface_parameters() {
    let parsed = parse_java_source(
        r#"
interface MessageSupplier {
    String get();
}

class MethodReferenceOverloadResolution {
    static String defaultMessage() {
        return "bad";
    }

    void validate(boolean condition, String message) {}
    void validate(boolean condition, MessageSupplier message) {}

    void run(boolean ok) {
        validate(ok, MethodReferenceOverloadResolution::defaultMessage);
    }
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    assert!(
        !resolved.diagnostics.has_errors(),
        "{:?}",
        resolved.diagnostics
    );

    let ty = &resolved.program.types[1];
    let body = ty
        .members
        .iter()
        .find_map(|member| match member {
            Member::Method(method) if method.name == "run" => method.body.as_ref(),
            _ => None,
        })
        .unwrap();
    let Stmt::Expr(Expr::Call {
        resolved: Some(call),
        ..
    }) = &body.statements[0]
    else {
        panic!("expected resolved method-reference overload call");
    };

    assert_eq!(call.declaring_type, "MethodReferenceOverloadResolution");
    assert_eq!(call.rust_name, "validate_boolean_message_supplier");
}

#[test]
fn resolver_matches_method_references_to_known_external_primitive_predicates() {
    let parsed = parse_java_source(
        r#"
class MethodReferencePrimitivePredicateResolution {
    static boolean isPositive(double value) {
        return value > 0.0;
    }

    void allMatch(double[] values, String label) {}
    void allMatch(double[] values, DoublePredicate predicate) {}

    void run(double[] values) {
        allMatch(values, MethodReferencePrimitivePredicateResolution::isPositive);
    }
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    assert!(
        !resolved.diagnostics.has_errors(),
        "{:?}",
        resolved.diagnostics
    );

    let ty = &resolved.program.types[0];
    let body = ty
        .members
        .iter()
        .find_map(|member| match member {
            Member::Method(method) if method.name == "run" => method.body.as_ref(),
            _ => None,
        })
        .unwrap();
    let Stmt::Expr(Expr::Call {
        resolved: Some(call),
        ..
    }) = &body.statements[0]
    else {
        panic!("expected resolved method-reference primitive predicate call");
    };

    assert_eq!(
        call.declaring_type,
        "MethodReferencePrimitivePredicateResolution"
    );
    assert_eq!(call.rust_name, "all_match_double_array_double_predicate");
}

#[test]
fn resolver_uses_source_functional_interface_parameter_types_inside_lambda_body() {
    let parsed = parse_java_source(
        r#"
interface RegionCallback {
    Result apply(Reference reference, FeatureSet features);
}

class Reference {}
class FeatureSet {}
class Span {}

class Result {
    Result(Reference reference, Span span) {}
    Result(FeatureSet features, Span span) {}
}

class TargetTypedLambdaBody {
    void register(RegionCallback callback) {}

    void run(Span span) {
        register((reference, features) -> new Result(reference, span));
    }
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    assert!(
        !resolved.diagnostics.has_errors(),
        "{:?}",
        resolved.diagnostics
    );

    let ty = resolved
        .program
        .types
        .iter()
        .find(|ty| ty.name == "TargetTypedLambdaBody")
        .unwrap();
    let body = ty
        .members
        .iter()
        .find_map(|member| match member {
            Member::Method(method) if method.name == "run" => method.body.as_ref(),
            _ => None,
        })
        .unwrap();
    let Stmt::Expr(Expr::Call { args, .. }) = &body.statements[0] else {
        panic!("expected register call");
    };
    let Expr::Lambda {
        body: new_java2rust::java_ir::LambdaBody::Expr(lambda_body),
        ..
    } = &args[0]
    else {
        panic!("expected lambda argument");
    };
    let Expr::New {
        resolved: Some(constructor),
        ..
    } = lambda_body.as_ref()
    else {
        panic!("expected resolved constructor in target-typed lambda body");
    };

    assert_eq!(constructor.declaring_type, "Result");
    assert_eq!(constructor.rust_name, "new_reference_span");
}

#[test]
fn resolver_uses_known_to_string_result_type_for_overload_arguments() {
    let parsed = parse_java_source(
        r#"
class ToStringArgumentResolution {
    void append(String value) {}
    void append(int value) {}

    void run(Object value) {
        append(value.toString());
    }
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    assert!(
        !resolved.diagnostics.has_errors(),
        "{:?}",
        resolved.diagnostics
    );

    let ty = &resolved.program.types[0];
    let body = ty
        .members
        .iter()
        .find_map(|member| match member {
            Member::Method(method) if method.name == "run" => method.body.as_ref(),
            _ => None,
        })
        .unwrap();
    let Stmt::Expr(Expr::Call {
        resolved: Some(call),
        ..
    }) = &body.statements[0]
    else {
        panic!("expected resolved toString argument overload call");
    };

    assert_eq!(call.declaring_type, "ToStringArgumentResolution");
    assert_eq!(call.rust_name, "append_string");
}

#[test]
fn resolver_uses_known_static_primitive_to_string_result_type_for_overload_arguments() {
    let parsed = parse_java_source(
        r#"
class StaticPrimitiveToStringArgumentResolution {
    void setColumn(int column, String value) {}
    void setColumn(String column, int value) {}

    void run(int column, long value) {
        setColumn(column, Long.toString(value));
    }
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    assert!(
        !resolved.diagnostics.has_errors(),
        "{:?}",
        resolved.diagnostics
    );

    let ty = &resolved.program.types[0];
    let body = ty
        .members
        .iter()
        .find_map(|member| match member {
            Member::Method(method) if method.name == "run" => method.body.as_ref(),
            _ => None,
        })
        .unwrap();
    let Stmt::Expr(Expr::Call {
        resolved: Some(call),
        ..
    }) = &body.statements[0]
    else {
        panic!("expected resolved static primitive toString overload call");
    };

    assert_eq!(
        call.declaring_type,
        "StaticPrimitiveToStringArgumentResolution"
    );
    assert_eq!(call.rust_name, "set_column_int_string");
}

#[test]
fn resolver_uses_bitwise_numeric_expression_type_for_overloads() {
    let parsed = parse_java_source(
        r#"
class BitwiseNumericArgumentResolution {
    void score(int left, int right) {}
    void score(String left, String right) {}

    void run(byte left, byte right) {
        score(left & 0xFF, right ^ 3);
    }
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    assert!(
        !resolved.diagnostics.has_errors(),
        "{:?}",
        resolved.diagnostics
    );

    let ty = &resolved.program.types[0];
    let body = ty
        .members
        .iter()
        .find_map(|member| match member {
            Member::Method(method) if method.name == "run" => method.body.as_ref(),
            _ => None,
        })
        .unwrap();
    let Stmt::Expr(Expr::Call {
        resolved: Some(call),
        ..
    }) = &body.statements[0]
    else {
        panic!("expected resolved score call");
    };

    assert_eq!(call.rust_name, "score_int_int");
}

#[test]
fn resolver_uses_known_static_format_result_type_for_overload_arguments() {
    let parsed = parse_java_source(
        r#"
class StaticFormatArgumentResolution {
    void setColumn(int column, String value) {}
    void setColumn(String column, int value) {}

    void run(int column, Object value) {
        setColumn(column, String.format("%s", value));
    }
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    assert!(
        !resolved.diagnostics.has_errors(),
        "{:?}",
        resolved.diagnostics
    );

    let ty = &resolved.program.types[0];
    let body = ty
        .members
        .iter()
        .find_map(|member| match member {
            Member::Method(method) if method.name == "run" => method.body.as_ref(),
            _ => None,
        })
        .unwrap();
    let Stmt::Expr(Expr::Call {
        resolved: Some(call),
        ..
    }) = &body.statements[0]
    else {
        panic!("expected resolved String.format argument overload call");
    };

    assert_eq!(call.declaring_type, "StaticFormatArgumentResolution");
    assert_eq!(call.rust_name, "set_column_int_string");
}

#[test]
fn resolver_uses_known_formatter_result_type_for_overload_arguments() {
    let parsed = parse_java_source(
        r#"
class FormatterArgumentResolution {
    DecimalFormat formatter;

    void setColumn(int column, String value) {}
    void setColumn(String column, int value) {}

    void run(int column, double value) {
        setColumn(column, formatter.format(value));
    }
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    assert!(
        !resolved.diagnostics.has_errors(),
        "{:?}",
        resolved.diagnostics
    );

    let ty = &resolved.program.types[0];
    let body = ty
        .members
        .iter()
        .find_map(|member| match member {
            Member::Method(method) if method.name == "run" => method.body.as_ref(),
            _ => None,
        })
        .unwrap();
    let Stmt::Expr(Expr::Call {
        resolved: Some(call),
        ..
    }) = &body.statements[0]
    else {
        panic!("expected resolved formatter overload call");
    };

    assert_eq!(call.declaring_type, "FormatterArgumentResolution");
    assert_eq!(call.rust_name, "set_column_int_string");
}

#[test]
fn resolver_uses_external_domain_getter_result_types_for_overload_arguments() {
    let parsed = parse_java_source(
        r#"
class DomainGetterArgumentResolution {
    void createInterval(String contig, int start, int end) {}
    void createInterval(int contig, String start, String end) {}

    void run(ExternalLocation location) {
        createInterval(location.getContig(), location.getStart(), location.getEnd());
    }
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    assert!(
        !resolved.diagnostics.has_errors(),
        "{:?}",
        resolved.diagnostics
    );

    let ty = &resolved.program.types[0];
    let body = ty
        .members
        .iter()
        .find_map(|member| match member {
            Member::Method(method) if method.name == "run" => method.body.as_ref(),
            _ => None,
        })
        .unwrap();
    let Stmt::Expr(Expr::Call {
        resolved: Some(call),
        ..
    }) = &body.statements[0]
    else {
        panic!("expected resolved domain getter overload call");
    };

    assert_eq!(call.declaring_type, "DomainGetterArgumentResolution");
    assert_eq!(call.rust_name, "create_interval_string_int_int");
}

#[test]
fn resolver_uses_external_string_list_getter_for_collection_constructor_overload() {
    let parsed = parse_java_source(
        r#"
class SampleList {
    SampleList(Collection<String> samples) {}
    SampleList(String... samples) {}
}

class ExternalStringListGetterConstructorResolution {
    SampleList run(ExternalVariantHeader header) {
        return new SampleList(header.getGenotypeSamples());
    }
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    assert!(
        !resolved.diagnostics.has_errors(),
        "{:?}",
        resolved.diagnostics
    );

    let Expr::New {
        resolved: Some(constructor),
        ..
    } = first_return_expr(&resolved.program.types[1]).unwrap()
    else {
        panic!("expected resolved constructor from external string-list getter");
    };

    assert_eq!(constructor.declaring_type, "SampleList");
    assert_eq!(constructor.rust_name, "new_collection");
}

#[test]
fn resolver_uses_external_numeric_domain_getters_for_overload_arguments() {
    let parsed = parse_java_source(
        r#"
class NumericDomainGetterArgumentResolution {
    void append(int value) {}
    void append(long value) {}
    void append(String value) {}

    void expandWithinContig(int padding, int contigLength) {}
    void expandWithinContig(int padding, String contigLength) {}

    void call(int maxPloidy, boolean emit) {}
    void call(String maxPloidy, boolean emit) {}

    void run(RepeatLocus locus, SequenceRecord record, VariantRecord variant) {
        append(locus.getPeriod());
        append(locus.getMask());
        append(Long.toBinaryString(locus.getMask()));
        expandWithinContig(10, record.getSequenceLength());
        call(variant.getMaxPloidy(2), true);
    }
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    assert!(
        !resolved.diagnostics.has_errors(),
        "{:?}",
        resolved.diagnostics
    );

    let ty = &resolved.program.types[0];
    let body = ty
        .members
        .iter()
        .find_map(|member| match member {
            Member::Method(method) if method.name == "run" => method.body.as_ref(),
            _ => None,
        })
        .unwrap();

    let expected = [
        "append_int",
        "append_long",
        "append_string",
        "expand_within_contig_int_int",
        "call_int_boolean",
    ];
    for (stmt, expected_rust_name) in body.statements.iter().zip(expected) {
        let Stmt::Expr(Expr::Call {
            resolved: Some(call),
            ..
        }) = stmt
        else {
            panic!("expected resolved numeric domain getter overload call");
        };

        assert_eq!(call.declaring_type, "NumericDomainGetterArgumentResolution");
        assert_eq!(call.rust_name, expected_rust_name);
    }
}

#[test]
fn resolver_uses_known_jdk_stream_hierarchy_for_overload_arguments() {
    let parsed = parse_java_source(
        r#"
class StreamHierarchyArgumentResolution {
    void consume(InputStream stream) {}
    void consume(String stream) {}

    void write(OutputStream stream) {}
    void write(String stream) {}

    void run(FileInputStream input, BufferedOutputStream output) {
        consume(input);
        write(output);
    }
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    assert!(
        !resolved.diagnostics.has_errors(),
        "{:?}",
        resolved.diagnostics
    );

    let ty = &resolved.program.types[0];
    let body = ty
        .members
        .iter()
        .find_map(|member| match member {
            Member::Method(method) if method.name == "run" => method.body.as_ref(),
            _ => None,
        })
        .unwrap();

    let expected = ["consume_input_stream", "write_output_stream"];
    for (stmt, expected_rust_name) in body.statements.iter().zip(expected) {
        let Stmt::Expr(Expr::Call {
            resolved: Some(call),
            ..
        }) = stmt
        else {
            panic!("expected resolved stream hierarchy overload call");
        };

        assert_eq!(call.declaring_type, "StreamHierarchyArgumentResolution");
        assert_eq!(call.rust_name, expected_rust_name);
    }
}

#[test]
fn resolver_uses_known_jdk_reader_writer_hierarchy_for_overload_arguments() {
    let parsed = parse_java_source(
        r#"
class CharacterIoHierarchyArgumentResolution {
    void read(Reader reader) {}
    void read(String reader) {}

    void write(Writer writer) {}
    void write(String writer) {}

    void run(FileReader reader, BufferedWriter writer) {
        read(reader);
        write(writer);
    }
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    assert!(
        !resolved.diagnostics.has_errors(),
        "{:?}",
        resolved.diagnostics
    );

    let ty = &resolved.program.types[0];
    let body = ty
        .members
        .iter()
        .find_map(|member| match member {
            Member::Method(method) if method.name == "run" => method.body.as_ref(),
            _ => None,
        })
        .unwrap();

    let expected = ["read_reader", "write_writer"];
    for (stmt, expected_rust_name) in body.statements.iter().zip(expected) {
        let Stmt::Expr(Expr::Call {
            resolved: Some(call),
            ..
        }) = stmt
        else {
            panic!("expected resolved character I/O hierarchy overload call");
        };

        assert_eq!(
            call.declaring_type,
            "CharacterIoHierarchyArgumentResolution"
        );
        assert_eq!(call.rust_name, expected_rust_name);
    }
}

#[test]
fn resolver_ignores_java_declaration_modifiers_in_known_type_assignability() {
    let parsed = parse_java_source(
        r#"
class ModifierTypeAssignabilityResolution {
    void load(InputStream input) {}
    void load(String input) {}

    void run(final InputStream input) {
        load(input);
    }
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    assert!(
        !resolved.diagnostics.has_errors(),
        "{:?}",
        resolved.diagnostics
    );

    let ty = &resolved.program.types[0];
    let body = ty
        .members
        .iter()
        .find_map(|member| match member {
            Member::Method(method) if method.name == "run" => method.body.as_ref(),
            _ => None,
        })
        .unwrap();
    let Stmt::Expr(Expr::Call {
        resolved: Some(call),
        ..
    }) = &body.statements[0]
    else {
        panic!("expected resolved load call");
    };

    assert_eq!(call.rust_name, "load_input_stream");
}

#[test]
fn resolver_uses_external_sequence_metadata_getters_for_overload_arguments() {
    let parsed = parse_java_source(
        r#"
class SequenceMetadataGetterResolution {
    void acceptName(String value) {}
    void acceptName(int value) {}
    void acceptIndex(int value) {}
    void acceptIndex(String value) {}

    void run(ExternalRead read, ExternalLocation location) {
        acceptName(read.getSampleName());
        acceptName(read.getBaseString());
        acceptIndex(read.getGQ());
        acceptIndex(location.getLineNumber());
    }
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    assert!(
        !resolved.diagnostics.has_errors(),
        "{:?}",
        resolved.diagnostics
    );

    let ty = &resolved.program.types[0];
    let body = ty
        .members
        .iter()
        .find_map(|member| match member {
            Member::Method(method) if method.name == "run" => method.body.as_ref(),
            _ => None,
        })
        .unwrap();
    let calls = body
        .statements
        .iter()
        .filter_map(|stmt| match stmt {
            Stmt::Expr(Expr::Call {
                resolved: Some(call),
                ..
            }) => Some(call.rust_name.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>();

    assert_eq!(
        calls,
        vec![
            "accept_name_string",
            "accept_name_string",
            "accept_index_int",
            "accept_index_int"
        ]
    );
}

#[test]
fn resolver_uses_external_variant_payload_getters_for_overload_arguments() {
    let parsed = parse_java_source(
        r#"
class VariantPayloadGetterResolution {
    void max(int[] values) {}
    void max(String values) {}
    void acceptAllele(Allele allele) {}
    void acceptAllele(String allele) {}
    void acceptAlleles(List<Allele> alleles) {}
    void acceptAlleles(String alleles) {}
    void acceptAlternateAlleles(List<Allele> alleles) {}
    void acceptAlternateAlleles(String alleles) {}

    void run(ExternalGenotype genotype, ExternalVariant variant) {
        max(genotype.getPL());
        acceptAllele(variant.getReference());
        acceptAlleles(variant.getAlleles());
        acceptAlternateAlleles(variant.getAlternateAlleles());
    }
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    assert!(
        !resolved.diagnostics.has_errors(),
        "{:?}",
        resolved.diagnostics
    );

    let ty = &resolved.program.types[0];
    let body = ty
        .members
        .iter()
        .find_map(|member| match member {
            Member::Method(method) if method.name == "run" => method.body.as_ref(),
            _ => None,
        })
        .unwrap();
    let calls = body
        .statements
        .iter()
        .filter_map(|stmt| match stmt {
            Stmt::Expr(Expr::Call {
                resolved: Some(call),
                ..
            }) => Some(call.rust_name.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>();

    assert_eq!(
        calls,
        vec![
            "max_int_array",
            "accept_allele_allele",
            "accept_alleles_list",
            "accept_alternate_alleles_list"
        ]
    );
}

#[test]
fn resolver_uses_external_read_and_interval_getters_for_overload_arguments() {
    let parsed = parse_java_source(
        r#"
class ExternalReadAndIntervalGetterResolution {
    void acceptReadGroup(SAMReadGroupRecord readGroup) {}
    void acceptReadGroup(String readGroup) {}
    void acceptIntervals(List<SimpleInterval> intervals) {}
    void acceptIntervals(String intervals) {}
    void acceptTraversal(TraversalParameters traversal) {}
    void acceptTraversal(String traversal) {}

    void run(ExternalRead read, IntervalSource source, SAMSequenceDictionary dictionary) {
        acceptReadGroup(read.getReadGroup());
        acceptIntervals(source.getIntervals(dictionary));
        acceptTraversal(source.getTraversalParameters(dictionary));
    }
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    assert!(
        !resolved.diagnostics.has_errors(),
        "{:?}",
        resolved.diagnostics
    );

    let ty = &resolved.program.types[0];
    let body = ty
        .members
        .iter()
        .find_map(|member| match member {
            Member::Method(method) if method.name == "run" => method.body.as_ref(),
            _ => None,
        })
        .unwrap();
    let calls = body
        .statements
        .iter()
        .filter_map(|stmt| match stmt {
            Stmt::Expr(Expr::Call {
                resolved: Some(call),
                ..
            }) => Some(call.rust_name.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>();

    assert_eq!(
        calls,
        vec![
            "accept_read_group_s_a_m_read_group_record",
            "accept_intervals_list",
            "accept_traversal_traversal_parameters"
        ]
    );
}

#[test]
fn resolver_uses_known_external_boolean_results_for_overload_arguments() {
    let parsed = parse_java_source(
        r#"
class ExternalBooleanResultArgumentResolution {
    void validate(boolean condition, String message) {}
    void validate(String condition, String message) {}

    void run(MapLike values, StreamLike stream) {
        validate(values.containsKey("id"), "missing id");
        validate(stream.allMatch(x -> true), "not all true");
    }
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    assert!(
        !resolved.diagnostics.has_errors(),
        "{:?}",
        resolved.diagnostics
    );

    let ty = &resolved.program.types[0];
    let body = ty
        .members
        .iter()
        .find_map(|member| match member {
            Member::Method(method) if method.name == "run" => method.body.as_ref(),
            _ => None,
        })
        .unwrap();
    let calls = body
        .statements
        .iter()
        .filter_map(|stmt| match stmt {
            Stmt::Expr(Expr::Call {
                resolved: Some(call),
                ..
            }) => Some(call.rust_name.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>();

    assert_eq!(
        calls,
        vec!["validate_boolean_string", "validate_boolean_string"]
    );
}

#[test]
fn resolver_uses_collection_predicate_results_for_validation_overloads() {
    let parsed = parse_java_source(
        r#"
class CollectionPredicateArgumentResolution {
    void validateArg(boolean condition, String message) {}
    void validateArg(String condition, String message) {}

    void run(Collection<String> available, Collection<String> required, String name) {
        validateArg(available.containsAll(required), "missing values");
        validateArg(!required.isEmpty(), "empty values");
        validateArg(!StringUtils.isEmpty(name), "empty name");
    }
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    assert!(
        !resolved.diagnostics.has_errors(),
        "{:?}",
        resolved.diagnostics
    );

    let ty = &resolved.program.types[0];
    let body = ty
        .members
        .iter()
        .find_map(|member| match member {
            Member::Method(method) if method.name == "run" => method.body.as_ref(),
            _ => None,
        })
        .unwrap();
    let calls = body
        .statements
        .iter()
        .filter_map(|stmt| match stmt {
            Stmt::Expr(Expr::Call {
                resolved: Some(call),
                ..
            }) => Some(call.rust_name.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>();

    assert_eq!(
        calls,
        vec![
            "validate_arg_boolean_string",
            "validate_arg_boolean_string",
            "validate_arg_boolean_string",
        ]
    );
}

#[test]
fn resolver_uses_external_collection_like_size_result_for_overloads() {
    let parsed = parse_java_source(
        r#"
class ExternalCollectionLikeSizeResolution {
    void requirePositive(int value, String message) {}
    void requirePositive(String value, String message) {}

    void run(IntervalList intervals) {
        requirePositive(intervals.size(), "must not be empty");
    }
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    assert!(
        !resolved.diagnostics.has_errors(),
        "{:?}",
        resolved.diagnostics
    );

    let Stmt::Expr(Expr::Call {
        resolved: Some(call),
        ..
    }) = first_stmt(&resolved.program.types[0]).unwrap()
    else {
        panic!("expected resolved external collection-like size overload");
    };

    assert_eq!(call.declaring_type, "ExternalCollectionLikeSizeResolution");
    assert_eq!(call.rust_name, "require_positive_int_string");
}

#[test]
fn resolver_uses_external_attribute_predicate_for_boolean_overloads() {
    let parsed = parse_java_source(
        r#"
class AttributePredicateResolution {
    void validateArg(boolean condition, String message) {}
    void validateArg(String condition, String message) {}

    void run(ExternalRecord record) {
        validateArg(record.hasAttribute("score"), "missing score");
    }
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    assert!(
        !resolved.diagnostics.has_errors(),
        "{:?}",
        resolved.diagnostics
    );

    let Stmt::Expr(Expr::Call {
        resolved: Some(call),
        ..
    }) = first_stmt(&resolved.program.types[0]).unwrap()
    else {
        panic!("expected resolved attribute predicate overload");
    };

    assert_eq!(call.declaring_type, "AttributePredicateResolution");
    assert_eq!(call.rust_name, "validate_arg_boolean_string");
}

#[test]
fn resolver_uses_null_branch_common_type_for_ternary_overload_arguments() {
    let parsed = parse_java_source(
        r#"
class TernaryNullCommonTypeResolution {
    void setAttribute(String key, String value) {}
    void setAttribute(String key, int value) {}

    void run(boolean useDefault, String name) {
        setAttribute("name", useDefault ? name : null);
    }
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    assert!(
        !resolved.diagnostics.has_errors(),
        "{:?}",
        resolved.diagnostics
    );

    let Stmt::Expr(Expr::Call {
        resolved: Some(call),
        ..
    }) = first_stmt(&resolved.program.types[0]).unwrap()
    else {
        panic!("expected resolved ternary null common-type overload");
    };

    assert_eq!(call.declaring_type, "TernaryNullCommonTypeResolution");
    assert_eq!(call.rust_name, "set_attribute_string_string");
}

#[test]
fn resolver_uses_collection_family_common_type_for_ternary_overload_arguments() {
    let parsed = parse_java_source(
        r#"
class TernaryCollectionCommonTypeResolution {
    void consume(List<String> values) {}
    void consume(String value) {}

    void run(boolean chooseFirst, ArrayList<String> first, LinkedList<String> second) {
        consume(chooseFirst ? first : second);
    }
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    assert!(
        !resolved.diagnostics.has_errors(),
        "{:?}",
        resolved.diagnostics
    );

    let Stmt::Expr(Expr::Call {
        resolved: Some(call),
        ..
    }) = first_stmt(&resolved.program.types[0]).unwrap()
    else {
        panic!("expected resolved ternary collection common-type overload");
    };

    assert_eq!(call.declaring_type, "TernaryCollectionCommonTypeResolution");
    assert_eq!(call.rust_name, "consume_list");
}

#[test]
fn resolver_uses_optional_presence_result_for_boolean_overloads() {
    let parsed = parse_java_source(
        r#"
class OptionalPresenceArgumentResolution {
    void validateArg(boolean condition, String message) {}
    void validateArg(String condition, String message) {}

    void run(Optional<String> value, OptionalInt count) {
        validateArg(value.isPresent(), "missing value");
        validateArg(count.isPresent(), "missing count");
    }
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    assert!(
        !resolved.diagnostics.has_errors(),
        "{:?}",
        resolved.diagnostics
    );

    let ty = &resolved.program.types[0];
    let body = ty
        .members
        .iter()
        .find_map(|member| match member {
            Member::Method(method) if method.name == "run" => method.body.as_ref(),
            _ => None,
        })
        .unwrap();
    let calls = body
        .statements
        .iter()
        .filter_map(|stmt| match stmt {
            Stmt::Expr(Expr::Call {
                resolved: Some(call),
                ..
            }) => Some(call.rust_name.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>();

    assert_eq!(
        calls,
        vec!["validate_arg_boolean_string", "validate_arg_boolean_string"]
    );
}

#[test]
fn resolver_uses_external_get_string_result_for_constructor_overloads() {
    let parsed = parse_java_source(
        r#"
class Interval {
    Interval(String label) {}
    Interval(int start) {}
}

class ExternalStringLookupConstructorResolution {
    Interval fromConfig(ConfigObject config) {
        return new Interval(config.getString("region"));
    }

    Interval fromRow(ResultRow row) {
        return new Interval(row.getString(1));
    }
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    assert!(
        !resolved.diagnostics.has_errors(),
        "{:?}",
        resolved.diagnostics
    );

    let ty = &resolved.program.types[1];
    let constructors = ty
        .members
        .iter()
        .filter_map(|member| match member {
            Member::Method(method) => method.body.as_ref(),
            _ => None,
        })
        .filter_map(|body| match &body.statements[0] {
            Stmt::Return(Some(Expr::New {
                resolved: Some(constructor),
                ..
            })) => Some(constructor.rust_name.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>();

    assert_eq!(constructors, vec!["new_string", "new_string"]);
}

#[test]
fn resolver_uses_external_attribute_string_result_for_constructor_overloads() {
    let parsed = parse_java_source(
        r#"
class Interval {
    Interval(String text) {}
    Interval(int start, int end) {}
}

class ExternalAttributeStringConstructorResolution {
    Interval make(ExternalRecord record) {
        return new Interval(record.getAttributeAsString("SPAN", ""));
    }
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    assert!(
        !resolved.diagnostics.has_errors(),
        "{:?}",
        resolved.diagnostics
    );

    let Expr::New {
        resolved: Some(constructor),
        ..
    } = first_return_expr(&resolved.program.types[1]).unwrap()
    else {
        panic!("expected resolved constructor from external attribute string result");
    };

    assert_eq!(constructor.declaring_type, "Interval");
    assert_eq!(constructor.rust_name, "new_string");
}

#[test]
fn resolver_uses_string_substring_result_type_for_constructor_overloads() {
    let parsed = parse_java_source(
        r#"
class Interval {
    Interval(String text) {}
    Interval(int start, int end) {}
}

class StringSubstringConstructorResolution {
    Interval make(String description, int offset) {
        return new Interval(description.substring(offset));
    }
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    assert!(
        !resolved.diagnostics.has_errors(),
        "{:?}",
        resolved.diagnostics
    );

    let Expr::New {
        resolved: Some(constructor),
        ..
    } = first_return_expr(&resolved.program.types[1]).unwrap()
    else {
        panic!("expected resolved constructor from substring result");
    };

    assert_eq!(constructor.declaring_type, "Interval");
    assert_eq!(constructor.rust_name, "new_string");
}

#[test]
fn resolver_uses_zero_arg_string_named_getter_result_for_overloads() {
    let parsed = parse_java_source(
        r#"
class ExternalStringNamedGetterResolution {
    void accept(String value, int index) {}
    void accept(int value, int index) {}

    void run(InputDescriptor input, DisplayRecord record) {
        accept(input.getRawInputString(), 0);
        accept(record.getDisplayString(), 1);
    }
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    assert!(
        !resolved.diagnostics.has_errors(),
        "{:?}",
        resolved.diagnostics
    );

    let ty = &resolved.program.types[0];
    let body = ty
        .members
        .iter()
        .find_map(|member| match member {
            Member::Method(method) if method.name == "run" => method.body.as_ref(),
            _ => None,
        })
        .unwrap();
    let calls = body
        .statements
        .iter()
        .filter_map(|stmt| match stmt {
            Stmt::Expr(Expr::Call {
                resolved: Some(call),
                ..
            }) => Some(call.rust_name.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>();

    assert_eq!(calls, vec!["accept_string_int", "accept_string_int"]);
}

#[test]
fn resolver_uses_external_return_type_from_argument_shape_for_quality_conversions() {
    let parsed = parse_java_source(
        r#"
class QualityConversionArgumentResolution {
    void add(int value) {}
    void add(String value) {}
    void setQualities(byte[] qualities) {}
    void setQualities(String qualities) {}

    void run(String encoded, char quality) {
        add(QualityCodec.fastqToPhred(quality));
        setQualities(QualityCodec.fastqToPhred(encoded));
    }
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    assert!(
        !resolved.diagnostics.has_errors(),
        "{:?}",
        resolved.diagnostics
    );

    let ty = &resolved.program.types[0];
    let body = ty
        .members
        .iter()
        .find_map(|member| match member {
            Member::Method(method) if method.name == "run" => method.body.as_ref(),
            _ => None,
        })
        .unwrap();
    let calls = body
        .statements
        .iter()
        .filter_map(|stmt| match stmt {
            Stmt::Expr(Expr::Call {
                resolved: Some(call),
                ..
            }) => Some(call.rust_name.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>();

    assert_eq!(calls, vec!["add_int", "set_qualities_byte_array"]);
}

#[test]
fn resolver_uses_external_quality_encoding_result_for_string_overloads() {
    let parsed = parse_java_source(
        r#"
class QualityEncodingArgumentResolution {
    void setAttribute(String key, String value) {}
    void setAttribute(String key, int value) {}

    void run(int quality, byte[] qualities) {
        setAttribute("Q", QualityCodec.phredToFastq(quality));
        setAttribute("QS", QualityCodec.phredToFastq(qualities));
    }
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    assert!(
        !resolved.diagnostics.has_errors(),
        "{:?}",
        resolved.diagnostics
    );

    let ty = &resolved.program.types[0];
    let body = ty
        .members
        .iter()
        .find_map(|member| match member {
            Member::Method(method) if method.name == "run" => method.body.as_ref(),
            _ => None,
        })
        .unwrap();
    let calls = body
        .statements
        .iter()
        .filter_map(|stmt| match stmt {
            Stmt::Expr(Expr::Call {
                resolved: Some(call),
                ..
            }) => Some(call.rust_name.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>();

    assert_eq!(
        calls,
        vec!["set_attribute_string_string", "set_attribute_string_string"]
    );
}

#[test]
fn resolver_uses_sequence_dictionary_getter_result_for_overload_arguments() {
    let parsed = parse_java_source(
        r#"
class SequenceDictionaryGetterArgumentResolution {
    void expandWithinContig(int padding, SAMSequenceDictionary dictionary) {}
    void expandWithinContig(int padding, String dictionary) {}

    void run(ExternalHeader header) {
        expandWithinContig(10, header.getSequenceDictionary());
    }
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    assert!(
        !resolved.diagnostics.has_errors(),
        "{:?}",
        resolved.diagnostics
    );

    let Stmt::Expr(Expr::Call {
        resolved: Some(call),
        ..
    }) = first_stmt(&resolved.program.types[0]).unwrap()
    else {
        panic!("expected resolved sequence-dictionary overload call");
    };

    assert_eq!(
        call.declaring_type,
        "SequenceDictionaryGetterArgumentResolution"
    );
    assert_eq!(
        call.rust_name,
        "expand_within_contig_int_s_a_m_sequence_dictionary"
    );
}

#[test]
fn resolver_uses_external_enum_like_constant_type_for_overload_arguments() {
    let parsed = parse_java_source(
        r#"
class ExternalEnumConstantArgumentResolution {
    void countClippedBases(Cigar cigar, CigarOperator operator) {}
    void countClippedBases(Cigar cigar, String operator) {}

    void run(Cigar cigar) {
        countClippedBases(cigar, CigarOperator.HARD_CLIP);
        countClippedBases(cigar, CigarOperator.MATCH_OR_MISMATCH);
    }
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    assert!(
        !resolved.diagnostics.has_errors(),
        "{:?}",
        resolved.diagnostics
    );

    let ty = &resolved.program.types[0];
    let body = ty
        .members
        .iter()
        .find_map(|member| match member {
            Member::Method(method) if method.name == "run" => method.body.as_ref(),
            _ => None,
        })
        .unwrap();
    let calls = body
        .statements
        .iter()
        .filter_map(|stmt| match stmt {
            Stmt::Expr(Expr::Call {
                resolved: Some(call),
                ..
            }) => Some(call),
            _ => None,
        })
        .collect::<Vec<_>>();

    assert_eq!(calls.len(), 2);
    for call in calls {
        assert_eq!(
            call.declaring_type,
            "ExternalEnumConstantArgumentResolution"
        );
        assert_eq!(call.rust_name, "count_clipped_bases_cigar_cigar_operator");
    }
}

#[test]
fn resolver_uses_external_allele_constant_type_for_overload_arguments() {
    let parsed = parse_java_source(
        r#"
class ExternalAlleleConstantArgumentResolution {
    void accept(Allele allele, String label) {}
    void accept(String allele, String label) {}

    void run() {
        accept(Allele.UNSPECIFIED_ALTERNATE_ALLELE, "alt");
        accept(Allele.NO_CALL, "missing");
        accept(Allele.SV_SIMPLE_DEL, "del");
        accept(Allele.SV_SIMPLE_DUP, "dup");
    }
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    assert!(
        !resolved.diagnostics.has_errors(),
        "{:?}",
        resolved.diagnostics
    );

    let ty = &resolved.program.types[0];
    let body = ty
        .members
        .iter()
        .find_map(|member| match member {
            Member::Method(method) if method.name == "run" => method.body.as_ref(),
            _ => None,
        })
        .unwrap();
    let calls = body
        .statements
        .iter()
        .filter_map(|stmt| match stmt {
            Stmt::Expr(Expr::Call {
                resolved: Some(call),
                ..
            }) => Some(call.rust_name.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>();

    assert_eq!(
        calls,
        vec![
            "accept_allele_string",
            "accept_allele_string",
            "accept_allele_string",
            "accept_allele_string"
        ]
    );
}

#[test]
fn resolver_uses_curated_external_enum_constant_types_for_overload_arguments() {
    let parsed = parse_java_source(
        r#"
class CuratedExternalEnumConstantArgumentResolution {
    void acceptOpen(OpenMode mode) {}
    void acceptOpen(String mode) {}

    void acceptLog(LogLevel level) {}
    void acceptLog(String level) {}

    void acceptSort(SortOrder order) {}
    void acceptSort(String order) {}

    void run() {
        acceptOpen(OpenMode.CREATE);
        acceptOpen(OpenMode.READ_ONLY);
        acceptLog(LogLevel.DEBUG);
        acceptSort(SortOrder.coordinate);
        acceptSort(SortOrder.unsorted);
    }
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    assert!(
        !resolved.diagnostics.has_errors(),
        "{:?}",
        resolved.diagnostics
    );

    let ty = &resolved.program.types[0];
    let body = ty
        .members
        .iter()
        .find_map(|member| match member {
            Member::Method(method) if method.name == "run" => method.body.as_ref(),
            _ => None,
        })
        .unwrap();
    let calls = body
        .statements
        .iter()
        .filter_map(|stmt| match stmt {
            Stmt::Expr(Expr::Call {
                resolved: Some(call),
                ..
            }) => Some(call.rust_name.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>();

    assert_eq!(
        calls,
        vec![
            "accept_open_open_mode",
            "accept_open_open_mode",
            "accept_log_log_level",
            "accept_sort_sort_order",
            "accept_sort_sort_order"
        ]
    );
}

#[test]
fn resolver_uses_external_header_and_genotype_enum_constant_types_for_overloads() {
    let parsed = parse_java_source(
        r#"
class ExternalHeaderAndGenotypeEnumConstantResolution {
    void acceptHeaderType(VCFHeaderLineType ty) {}
    void acceptHeaderType(String ty) {}

    void acceptHeaderCount(VCFHeaderLineCount count) {}
    void acceptHeaderCount(String count) {}

    void acceptGenotype(GenotypeType genotype) {}
    void acceptGenotype(String genotype) {}

    void acceptStrand(Strand strand) {}
    void acceptStrand(String strand) {}

    void run() {
        acceptHeaderType(VCFHeaderLineType.String);
        acceptHeaderType(VCFHeaderLineType.Integer);
        acceptHeaderType(VCFHeaderLineType.Float);
        acceptHeaderType(VCFHeaderLineType.Flag);
        acceptHeaderCount(VCFHeaderLineCount.A);
        acceptHeaderCount(VCFHeaderLineCount.UNBOUNDED);
        acceptGenotype(GenotypeType.HET);
        acceptGenotype(GenotypeType.HOM_REF);
        acceptStrand(Strand.POSITIVE);
        acceptStrand(Strand.NEGATIVE);
    }
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    assert!(
        !resolved.diagnostics.has_errors(),
        "{:?}",
        resolved.diagnostics
    );

    let body = resolved.program.types[0]
        .members
        .iter()
        .find_map(|member| match member {
            Member::Method(method) if method.name == "run" => method.body.as_ref(),
            _ => None,
        })
        .unwrap();
    let calls = body
        .statements
        .iter()
        .filter_map(|stmt| match stmt {
            Stmt::Expr(Expr::Call {
                resolved: Some(call),
                ..
            }) => Some(call.rust_name.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>();

    assert_eq!(
        calls,
        vec![
            "accept_header_type_v_c_f_header_line_type",
            "accept_header_type_v_c_f_header_line_type",
            "accept_header_type_v_c_f_header_line_type",
            "accept_header_type_v_c_f_header_line_type",
            "accept_header_count_v_c_f_header_line_count",
            "accept_header_count_v_c_f_header_line_count",
            "accept_genotype_genotype_type",
            "accept_genotype_genotype_type",
            "accept_strand_strand",
            "accept_strand_strand",
        ]
    );
}

#[test]
fn resolver_treats_curated_external_nested_state_enum_qualifiers_as_types() {
    let parsed = parse_java_source(
        r#"
class ExternalNestedStateEnumQualifierResolution {
    void acceptCall(CallState state) {}
    void acceptCall(String state) {}

    void acceptTruth(TruthState state) {}
    void acceptTruth(String state) {}

    void run() {
        acceptCall(GenotypeConcordanceStates.CallState.HOM_REF);
        acceptCall(GenotypeConcordanceStates.CallState.HET_REF_VAR1);
        acceptTruth(GenotypeConcordanceStates.TruthState.HOM_REF);
        acceptTruth(GenotypeConcordanceStates.TruthState.NO_CALL);
    }
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    assert!(
        !resolved.diagnostics.has_errors(),
        "{:?}",
        resolved.diagnostics
    );

    let unresolved_fields = resolved
        .facts
        .unresolved_target_facts
        .iter()
        .filter(|fact| fact.kind == UnresolvedTargetKind::FieldAccess)
        .collect::<Vec<_>>();
    assert!(
        unresolved_fields.is_empty(),
        "nested external state enum qualifiers should not be recorded as unresolved fields: {unresolved_fields:?}"
    );

    let body = resolved.program.types[0]
        .members
        .iter()
        .find_map(|member| match member {
            Member::Method(method) if method.name == "run" => method.body.as_ref(),
            _ => None,
        })
        .unwrap();
    let calls = body
        .statements
        .iter()
        .filter_map(|stmt| match stmt {
            Stmt::Expr(Expr::Call {
                resolved: Some(call),
                ..
            }) => Some(call.rust_name.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>();

    assert_eq!(
        calls,
        vec![
            "accept_call_call_state",
            "accept_call_call_state",
            "accept_truth_truth_state",
            "accept_truth_truth_state",
        ]
    );
}

#[test]
fn resolver_treats_curated_external_nested_enum_qualifiers_as_types() {
    let parsed = parse_java_source(
        r#"
class ExternalNestedEnumQualifierResolution {
    void acceptOpen(OpenMode mode) {}
    void acceptOpen(String mode) {}

    void acceptLog(LogLevel level) {}
    void acceptLog(String level) {}

    void acceptSort(SortOrder order) {}
    void acceptSort(String order) {}

    void run() {
        acceptOpen(HDF5File.OpenMode.CREATE);
        acceptOpen(HDF5File.OpenMode.READ_ONLY);
        acceptLog(Log.LogLevel.DEBUG);
        acceptLog(Log.LogLevel.INFO);
        acceptSort(SAMFileHeader.SortOrder.coordinate);
        acceptSort(SAMFileHeader.SortOrder.unsorted);
    }
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    assert!(
        !resolved.diagnostics.has_errors(),
        "{:?}",
        resolved.diagnostics
    );

    let unresolved_fields = resolved
        .facts
        .unresolved_target_facts
        .iter()
        .filter(|fact| fact.kind == UnresolvedTargetKind::FieldAccess)
        .collect::<Vec<_>>();
    assert!(
        unresolved_fields.is_empty(),
        "nested external enum qualifiers should not be recorded as unresolved fields: {unresolved_fields:?}"
    );

    let ty = &resolved.program.types[0];
    let body = ty
        .members
        .iter()
        .find_map(|member| match member {
            Member::Method(method) if method.name == "run" => method.body.as_ref(),
            _ => None,
        })
        .unwrap();
    let calls = body
        .statements
        .iter()
        .filter_map(|stmt| match stmt {
            Stmt::Expr(Expr::Call {
                resolved: Some(call),
                ..
            }) => Some(call.rust_name.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>();

    assert_eq!(
        calls,
        vec![
            "accept_open_open_mode",
            "accept_open_open_mode",
            "accept_log_log_level",
            "accept_log_log_level",
            "accept_sort_sort_order",
            "accept_sort_sort_order"
        ]
    );
}

#[test]
fn resolver_treats_known_external_nested_enum_families_as_type_qualifiers() {
    let parsed = parse_java_source(
        r#"
class ExternalNestedEnumFamilyQualifierResolution {
    void acceptAttribute(Attribute attribute) {}
    void acceptAttribute(String attribute) {}

    void acceptPriority(Priority priority) {}
    void acceptPriority(String priority) {}

    void acceptTermination(FileTermination termination) {}
    void acceptTermination(String termination) {}

    void acceptGroup(GroupOrder group) {}
    void acceptGroup(String group) {}

    void run() {
        acceptAttribute(FieldValue.Attribute.PRIMITIVE);
        acceptPriority(QueryJobConfiguration.Priority.BATCH);
        acceptPriority(QueryJobConfiguration.Priority.INTERACTIVE);
        acceptTermination(BlockCompressedInputStream.FileTermination.DEFECTIVE);
        acceptTermination(BlockCompressedInputStream.FileTermination.HAS_TERMINATOR_BLOCK);
        acceptGroup(SAMFileHeader.GroupOrder.query);
    }
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    assert!(
        !resolved.diagnostics.has_errors(),
        "{:?}",
        resolved.diagnostics
    );

    let unresolved_fields = resolved
        .facts
        .unresolved_target_facts
        .iter()
        .filter(|fact| fact.kind == UnresolvedTargetKind::FieldAccess)
        .collect::<Vec<_>>();
    assert!(
        unresolved_fields.is_empty(),
        "known external nested enum families should not be field misses: {unresolved_fields:?}"
    );

    let calls = resolved.program.types[0]
        .members
        .iter()
        .find_map(|member| match member {
            Member::Method(method) if method.name == "run" => method.body.as_ref(),
            _ => None,
        })
        .unwrap()
        .statements
        .iter()
        .filter_map(|stmt| match stmt {
            Stmt::Expr(Expr::Call {
                resolved: Some(call),
                ..
            }) => Some(call.rust_name.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>();

    assert_eq!(
        calls,
        vec![
            "accept_attribute_attribute",
            "accept_priority_priority",
            "accept_priority_priority",
            "accept_termination_file_termination",
            "accept_termination_file_termination",
            "accept_group_group_order",
        ]
    );
}

#[test]
fn resolver_uses_known_external_enum_and_byte_constant_families_for_overloads() {
    let parsed = parse_java_source(
        r#"
class ExternalEnumAndByteConstantFamilyResolution {
    void acceptType(StructuralVariantType type) {}
    void acceptType(String type) {}

    void acceptGoal(GoalType goal) {}
    void acceptGoal(String goal) {}

    void acceptEstimation(EstimationType type) {}
    void acceptEstimation(String type) {}

    void acceptOrientation(byte orientation) {}
    void acceptOrientation(String orientation) {}

    void run() {
        acceptType(StructuralVariantType.DEL);
        acceptType(StructuralVariantType.CNV);
        acceptGoal(GoalType.MAXIMIZE);
        acceptEstimation(Percentile.EstimationType.R_1);
        acceptOrientation(ReadEnds.FR);
        acceptOrientation(ReadEnds.RF);
    }
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    assert!(
        !resolved.diagnostics.has_errors(),
        "{:?}",
        resolved.diagnostics
    );

    let unresolved_fields = resolved
        .facts
        .unresolved_target_facts
        .iter()
        .filter(|fact| fact.kind == UnresolvedTargetKind::FieldAccess)
        .collect::<Vec<_>>();
    assert!(
        unresolved_fields.is_empty(),
        "known external enum and byte constants should not be field misses: {unresolved_fields:?}"
    );

    let calls = resolved.program.types[0]
        .members
        .iter()
        .find_map(|member| match member {
            Member::Method(method) if method.name == "run" => method.body.as_ref(),
            _ => None,
        })
        .unwrap()
        .statements
        .iter()
        .filter_map(|stmt| match stmt {
            Stmt::Expr(Expr::Call {
                resolved: Some(call),
                ..
            }) => Some(call.rust_name.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>();

    assert_eq!(
        calls,
        vec![
            "accept_type_structural_variant_type",
            "accept_type_structural_variant_type",
            "accept_goal_goal_type",
            "accept_estimation_estimation_type",
            "accept_orientation_byte",
            "accept_orientation_byte",
        ]
    );
}

#[test]
fn resolver_uses_jdk_empty_collection_static_field_types_for_overload_arguments() {
    let parsed = parse_java_source(
        r#"
class JdkEmptyCollectionFieldArgumentResolution {
    void acceptList(List values) {}
    void acceptList(String values) {}

    void acceptSet(Set values) {}
    void acceptSet(String values) {}

    void acceptMap(Map values) {}
    void acceptMap(String values) {}

    void run() {
        acceptList(Collections.EMPTY_LIST);
        acceptSet(Collections.EMPTY_SET);
        acceptMap(Collections.EMPTY_MAP);
    }
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    assert!(
        !resolved.diagnostics.has_errors(),
        "{:?}",
        resolved.diagnostics
    );

    let ty = &resolved.program.types[0];
    let body = ty
        .members
        .iter()
        .find_map(|member| match member {
            Member::Method(method) if method.name == "run" => method.body.as_ref(),
            _ => None,
        })
        .unwrap();
    let calls = body
        .statements
        .iter()
        .filter_map(|stmt| match stmt {
            Stmt::Expr(Expr::Call {
                resolved: Some(call),
                ..
            }) => Some(call.rust_name.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>();

    assert_eq!(
        calls,
        vec!["accept_list_list", "accept_set_set", "accept_map_map"]
    );
}

#[test]
fn resolver_uses_external_string_constant_type_for_overload_arguments() {
    let parsed = parse_java_source(
        r#"
class ExternalStringConstantArgumentResolution {
    void accept(String key) {}
    void accept(int key) {}

    void run() {
        accept(VCFConstants.GENOTYPE_POSTERIORS_KEY);
        accept(FileExtensions.BAI_INDEX);
    }
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    assert!(
        !resolved.diagnostics.has_errors(),
        "{:?}",
        resolved.diagnostics
    );

    let ty = &resolved.program.types[0];
    let body = ty
        .members
        .iter()
        .find_map(|member| match member {
            Member::Method(method) if method.name == "run" => method.body.as_ref(),
            _ => None,
        })
        .unwrap();
    let calls = body
        .statements
        .iter()
        .filter_map(|stmt| match stmt {
            Stmt::Expr(Expr::Call {
                resolved: Some(call),
                ..
            }) => Some(call.rust_name.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>();

    assert_eq!(calls, vec!["accept_string", "accept_string"]);
}

#[test]
fn resolver_uses_typed_external_static_field_constants_for_overload_arguments() {
    let parsed = parse_java_source(
        r#"
class ExternalTypedFieldConstantResolution {
    void accept(Cigar cigar) {}
    void accept(PrintStream stream) {}
    void accept(String value) {}

    void run() {
        accept(SAMRecord.NO_ALIGNMENT_CIGAR);
        accept(System.out);
    }
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    assert!(
        !resolved.diagnostics.has_errors(),
        "{:?}",
        resolved.diagnostics
    );

    let ty = &resolved.program.types[0];
    let body = ty
        .members
        .iter()
        .find_map(|member| match member {
            Member::Method(method) if method.name == "run" => method.body.as_ref(),
            _ => None,
        })
        .unwrap();
    let calls = body
        .statements
        .iter()
        .filter_map(|stmt| match stmt {
            Stmt::Expr(Expr::Call {
                resolved: Some(call),
                ..
            }) => Some(call.rust_name.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>();

    assert_eq!(calls, vec!["accept_cigar", "accept_print_stream"]);
}

#[test]
fn resolver_uses_known_jdk_static_constant_types_for_overload_arguments() {
    let parsed = parse_java_source(
        r#"
class JdkStaticConstantArgumentResolution {
    void accept(Charset charset) {}
    void accept(int flags) {}
    void accept(RoundingMode mode) {}
    void accept(String value) {}

    void run() {
        accept(StandardCharsets.US_ASCII);
        accept(Pattern.CASE_INSENSITIVE);
        accept(RoundingMode.HALF_UP);
    }
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    assert!(
        !resolved.diagnostics.has_errors(),
        "{:?}",
        resolved.diagnostics
    );

    let ty = &resolved.program.types[0];
    let body = ty
        .members
        .iter()
        .find_map(|member| match member {
            Member::Method(method) if method.name == "run" => method.body.as_ref(),
            _ => None,
        })
        .unwrap();
    let calls = body
        .statements
        .iter()
        .filter_map(|stmt| match stmt {
            Stmt::Expr(Expr::Call {
                resolved: Some(call),
                ..
            }) => Some(call.rust_name.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>();

    assert_eq!(
        calls,
        vec!["accept_charset", "accept_int", "accept_rounding_mode"]
    );
}

#[test]
fn resolver_uses_external_string_constant_and_supplier_lambda_for_overload_arguments() {
    let parsed = parse_java_source(
        r#"
class ExternalStringConstantSupplierArgumentResolution {
    void getAttributeAsDoubleArray(Genotype genotype, String key, Supplier<double[]> defaultValue, double missingValue) {}
    void getAttributeAsDoubleArray(Genotype genotype, int key, String defaultValue, double missingValue) {}

    void run(Genotype genotype) {
        getAttributeAsDoubleArray(genotype, VCFConstants.GENOTYPE_POSTERIORS_KEY, () -> null, 0.0);
    }
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    assert!(
        !resolved.diagnostics.has_errors(),
        "{:?}",
        resolved.diagnostics
    );

    let Stmt::Expr(Expr::Call {
        resolved: Some(call),
        ..
    }) = first_stmt(&resolved.program.types[0]).unwrap()
    else {
        panic!("expected resolved external string constant supplier overload call");
    };

    assert_eq!(
        call.declaring_type,
        "ExternalStringConstantSupplierArgumentResolution"
    );
    assert_eq!(
        call.rust_name,
        "get_attribute_as_double_array_genotype_string_supplier_double"
    );
}

#[test]
fn resolver_uses_known_external_static_decoder_return_type_for_overload_arguments() {
    let parsed = parse_java_source(
        r#"
class ExternalStaticDecoderArgumentResolution {
    void create(SAMFileHeader header, Cigar cigar) {}
    void create(SAMFileHeader header, String cigar) {}

    void run(SAMFileHeader header, String cigarText) {
        create(header, TextCigarCodec.decode(cigarText));
    }
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    assert!(
        !resolved.diagnostics.has_errors(),
        "{:?}",
        resolved.diagnostics
    );

    let Stmt::Expr(Expr::Call {
        resolved: Some(call),
        ..
    }) = first_stmt(&resolved.program.types[0]).unwrap()
    else {
        panic!("expected resolved external decoder overload call");
    };

    assert_eq!(
        call.declaring_type,
        "ExternalStaticDecoderArgumentResolution"
    );
    assert_eq!(call.rust_name, "create_s_a_m_file_header_cigar");
}

#[test]
fn resolver_uses_assignment_expression_target_type_for_overload_arguments() {
    let parsed = parse_java_source(
        r#"
class AssignmentExpressionArgumentResolution {
    static Nucleotide decode(byte base) { return null; }
    static Nucleotide decode(String base) { return null; }

    void accept(Nucleotide nucleotide) {}
    void accept(String nucleotide) {}

    void run(byte[] bases) {
        byte first;
        accept(decode(first = bases[0]));
    }
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    assert!(
        !resolved.diagnostics.has_errors(),
        "{:?}",
        resolved.diagnostics
    );

    let ty = &resolved.program.types[0];
    let body = ty
        .members
        .iter()
        .find_map(|member| match member {
            Member::Method(method) if method.name == "run" => method.body.as_ref(),
            _ => None,
        })
        .unwrap();
    let Stmt::Expr(Expr::Call {
        resolved: Some(call),
        ..
    }) = &body.statements[1]
    else {
        panic!("expected resolved assignment-expression overload call");
    };

    assert_eq!(call.rust_name, "accept_nucleotide");
}

#[test]
fn resolver_uses_external_enum_name_result_for_string_overloads() {
    let parsed = parse_java_source(
        r#"
class ExternalEnumNameArgumentResolution {
    void getOptionalIntAttribute(ReadRecord read, String tag) {}
    void getOptionalIntAttribute(ReadRecord read, int tag) {}

    void run(ReadRecord read) {
        getOptionalIntAttribute(read, SAMTag.NM.name());
    }
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    assert!(
        !resolved.diagnostics.has_errors(),
        "{:?}",
        resolved.diagnostics
    );

    let Stmt::Expr(Expr::Call {
        resolved: Some(call),
        ..
    }) = first_stmt(&resolved.program.types[0]).unwrap()
    else {
        panic!("expected resolved enum-name overload call");
    };

    assert_eq!(call.declaring_type, "ExternalEnumNameArgumentResolution");
    assert_eq!(
        call.rust_name,
        "get_optional_int_attribute_read_record_string"
    );
}

#[test]
fn resolver_uses_primitive_array_utility_to_array_result_for_overloads() {
    let parsed = parse_java_source(
        r#"
class PrimitiveArrayUtilityArgumentResolution {
    void median(int[] values) {}
    void median(String values) {}
    void acceptDoubles(double[] values) {}
    void acceptDoubles(String values) {}

    void run(Collection<Integer> values, Collection<Double> realValues) {
        median(Ints.toArray(values));
        acceptDoubles(Doubles.toArray(realValues));
    }
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    assert!(
        !resolved.diagnostics.has_errors(),
        "{:?}",
        resolved.diagnostics
    );

    let ty = &resolved.program.types[0];
    let body = ty
        .members
        .iter()
        .find_map(|member| match member {
            Member::Method(method) if method.name == "run" => method.body.as_ref(),
            _ => None,
        })
        .unwrap();
    let calls = body
        .statements
        .iter()
        .filter_map(|stmt| match stmt {
            Stmt::Expr(Expr::Call {
                resolved: Some(call),
                ..
            }) => Some(call.rust_name.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>();

    assert_eq!(
        calls,
        vec!["median_int_array", "accept_doubles_double_array"]
    );
}

#[test]
fn resolver_uses_array_clone_result_type_for_overloads() {
    let parsed = parse_java_source(
        r#"
class ArrayCloneArgumentResolution {
    void acceptBytes(byte[] values) {}
    void acceptBytes(String values) {}

    void acceptNames(String[] values) {}
    void acceptNames(String value) {}

    void run(byte[] bases, String[] names) {
        acceptBytes(bases.clone());
        acceptNames(names.clone());
    }
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    assert!(
        !resolved.diagnostics.has_errors(),
        "{:?}",
        resolved.diagnostics
    );

    let ty = &resolved.program.types[0];
    let body = ty
        .members
        .iter()
        .find_map(|member| match member {
            Member::Method(method) if method.name == "run" => method.body.as_ref(),
            _ => None,
        })
        .unwrap();
    let calls = body
        .statements
        .iter()
        .filter_map(|stmt| match stmt {
            Stmt::Expr(Expr::Call {
                resolved: Some(call),
                ..
            }) => Some(call.rust_name.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>();

    assert_eq!(
        calls,
        vec!["accept_bytes_byte_array", "accept_names_string_array"]
    );
}

#[test]
fn resolver_uses_collection_to_array_result_type_for_overloads() {
    let parsed = parse_java_source(
        r#"
class CollectionToArrayArgumentResolution {
    void acceptNames(String[] values) {}
    void acceptNames(String value) {}

    void acceptObjects(Object[] values) {}
    void acceptObjects(String value) {}

    void run(List<String> names, Collection<Object> objects) {
        acceptNames(names.toArray(new String[0]));
        acceptObjects(objects.toArray());
    }
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    assert!(
        !resolved.diagnostics.has_errors(),
        "{:?}",
        resolved.diagnostics
    );

    let ty = &resolved.program.types[0];
    let body = ty
        .members
        .iter()
        .find_map(|member| match member {
            Member::Method(method) if method.name == "run" => method.body.as_ref(),
            _ => None,
        })
        .unwrap();
    let calls = body
        .statements
        .iter()
        .filter_map(|stmt| match stmt {
            Stmt::Expr(Expr::Call {
                resolved: Some(call),
                ..
            }) => Some(call.rust_name.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>();

    assert_eq!(
        calls,
        vec!["accept_names_string_array", "accept_objects_object_array"]
    );
}

#[test]
fn resolver_uses_stream_to_array_generator_result_type_for_varargs_overloads() {
    let parsed = parse_java_source(
        r#"
class StreamToArrayGeneratorVarargsResolution {
    void validate(File... inputs) {}
    void validate(String... inputs) {}

    void run(Stream<File> files) {
        validate(files.toArray(File[]::new));
    }
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    assert!(
        !resolved.diagnostics.has_errors(),
        "{:?}",
        resolved.diagnostics
    );

    let Stmt::Expr(Expr::Call {
        resolved: Some(call),
        ..
    }) = first_stmt(&resolved.program.types[0]).unwrap()
    else {
        panic!("expected resolved stream-to-array varargs overload");
    };

    assert_eq!(
        call.declaring_type,
        "StreamToArrayGeneratorVarargsResolution"
    );
    assert_eq!(call.rust_name, "validate_file_array_varargs");
}

#[test]
fn resolver_uses_stream_flat_map_collection_method_reference_before_to_array_varargs() {
    let parsed = parse_java_source(
        r#"
class StreamFlatMapCollectionToArrayVarargsResolution {
    void validate(File... inputs) {}
    void validate(String... inputs) {}

    void run(List<File> first, List<File> second, File extra) {
        validate(Stream.of(first, second, Collections.singletonList(extra))
                .flatMap(Collection::stream)
                .toArray(File[]::new));
    }
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    assert!(
        !resolved.diagnostics.has_errors(),
        "{:?}",
        resolved.diagnostics
    );

    let Stmt::Expr(Expr::Call {
        resolved: Some(call),
        ..
    }) = first_stmt(&resolved.program.types[0]).unwrap()
    else {
        panic!("expected resolved flattened stream-to-array varargs overload");
    };

    assert_eq!(
        call.declaring_type,
        "StreamFlatMapCollectionToArrayVarargsResolution"
    );
    assert_eq!(call.rust_name, "validate_file_array_varargs");
}

#[test]
fn resolver_uses_external_parallelize_result_type_for_overloads() {
    let parsed = parse_java_source(
        r#"
class ExternalParallelizeArgumentResolution {
    void join(JavaRDD<Item> values, int count) {}
    void join(List<Item> values, String count) {}

    void run(JavaSparkContext context, List<Item> values) {
        join(context.parallelize(values), 3);
    }
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    assert!(
        !resolved.diagnostics.has_errors(),
        "{:?}",
        resolved.diagnostics
    );

    let ty = &resolved.program.types[0];
    let body = ty
        .members
        .iter()
        .find_map(|member| match member {
            Member::Method(method) if method.name == "run" => method.body.as_ref(),
            _ => None,
        })
        .unwrap();
    let calls = body
        .statements
        .iter()
        .filter_map(|stmt| match stmt {
            Stmt::Expr(Expr::Call {
                resolved: Some(call),
                ..
            }) => Some(call.rust_name.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>();

    assert_eq!(calls, vec!["join_java_r_d_d_int"]);
}

#[test]
fn resolver_uses_external_enumeration_iterator_adapter_for_stream_overloads() {
    let parsed = parse_java_source(
        r#"
class ExternalEnumerationStreamSourceResolution {
    <T> Stream<T> stream(Iterable<T> values) {
        return null;
    }

    <T> Stream<T> stream(Iterator<T> values) {
        return null;
    }

    Stream<Item> run(Enumeration<Item> values) {
        return stream(Iterators.forEnumeration(values));
    }
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    assert!(
        !resolved.diagnostics.has_errors(),
        "{:?}",
        resolved.diagnostics
    );

    let body = resolved.program.types[0]
        .members
        .iter()
        .find_map(|member| match member {
            Member::Method(method) if method.name == "run" => method.body.as_ref(),
            _ => None,
        })
        .unwrap();
    let Stmt::Return(Some(Expr::Call {
        resolved: Some(call),
        ..
    })) = &body.statements[0]
    else {
        panic!("expected resolved stream overload from enumeration adapter");
    };

    assert_eq!(call.rust_name, "stream_iterator");
}

#[test]
fn resolver_uses_external_directory_stream_result_for_iterable_overloads() {
    let parsed = parse_java_source(
        r#"
class ExternalDirectoryStreamSourceResolution {
    <T> Stream<T> stream(Iterable<T> values) {
        return null;
    }

    Stream<String> stream(String values) {
        return null;
    }

    Stream<Path> run(Path directory, String glob) {
        return stream(Files.newDirectoryStream(directory, glob));
    }
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    assert!(
        !resolved.diagnostics.has_errors(),
        "{:?}",
        resolved.diagnostics
    );

    let body = resolved.program.types[0]
        .members
        .iter()
        .find_map(|member| match member {
            Member::Method(method) if method.name == "run" => method.body.as_ref(),
            _ => None,
        })
        .unwrap();
    let Stmt::Return(Some(Expr::Call {
        resolved: Some(call),
        ..
    })) = &body.statements[0]
    else {
        panic!("expected resolved stream overload from directory stream");
    };

    assert_eq!(call.rust_name, "stream_iterable");
}

#[test]
fn resolver_uses_arrays_copy_of_range_result_type_for_overloads() {
    let parsed = parse_java_source(
        r#"
class ArraysCopyOfRangeArgumentResolution {
    void acceptBytes(byte[] values, byte[] reference) {}
    void acceptBytes(String values, byte[] reference) {}

    void acceptNames(String[] values) {}
    void acceptNames(String values) {}

    void run(byte[] bases, String[] names) {
        acceptBytes(Arrays.copyOfRange(bases, 1, 3), bases);
        acceptNames(Arrays.copyOfRange(names, 0, 2));
    }
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    assert!(
        !resolved.diagnostics.has_errors(),
        "{:?}",
        resolved.diagnostics
    );

    let ty = &resolved.program.types[0];
    let body = ty
        .members
        .iter()
        .find_map(|member| match member {
            Member::Method(method) if method.name == "run" => method.body.as_ref(),
            _ => None,
        })
        .unwrap();
    let calls = body
        .statements
        .iter()
        .filter_map(|stmt| match stmt {
            Stmt::Expr(Expr::Call {
                resolved: Some(call),
                ..
            }) => Some(call.rust_name.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>();

    assert_eq!(
        calls,
        vec![
            "accept_bytes_byte_array_byte_array",
            "accept_names_string_array"
        ]
    );
}

#[test]
fn resolver_uses_arrays_copy_of_result_type_for_constructor_overloads() {
    let parsed = parse_java_source(
        r#"
class ByteStore {
    ByteStore(byte[] values) {}
    ByteStore(String values) {}
}

class ArraysCopyOfConstructorResolution {
    ByteStore make(byte[] values, int length) {
        return new ByteStore(Arrays.copyOf(values, length));
    }
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    assert!(
        !resolved.diagnostics.has_errors(),
        "{:?}",
        resolved.diagnostics
    );

    let Expr::New {
        resolved: Some(constructor),
        ..
    } = first_return_expr(&resolved.program.types[1]).unwrap()
    else {
        panic!("expected resolved constructor from Arrays.copyOf result");
    };

    assert_eq!(constructor.declaring_type, "ByteStore");
    assert_eq!(constructor.rust_name, "new_byte_array");
}

#[test]
fn resolver_uses_primitive_array_add_all_result_type_for_constructor_overloads() {
    let parsed = parse_java_source(
        r#"
class ByteStore {
    ByteStore(byte[] values) {}
    ByteStore(String values) {}
}

class PrimitiveArrayAddAllConstructorResolution {
    ByteStore make(byte[] left, byte[] right) {
        return new ByteStore(ArrayUtils.addAll(left, right));
    }
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    assert!(
        !resolved.diagnostics.has_errors(),
        "{:?}",
        resolved.diagnostics
    );

    let Expr::New {
        resolved: Some(constructor),
        ..
    } = first_return_expr(&resolved.program.types[1]).unwrap()
    else {
        panic!("expected resolved constructor from primitive array addAll result");
    };

    assert_eq!(constructor.declaring_type, "ByteStore");
    assert_eq!(constructor.rust_name, "new_byte_array");
}

#[test]
fn resolver_uses_external_array_math_result_type_for_overload_arguments() {
    let parsed = parse_java_source(
        r#"
class ExternalArrayMathResultResolution {
    double sum(double[] values) { return 0.0; }
    double sum(String values) { return 0.0; }

    void run(double[] left, double[] right) {
        double total = sum(MathArrays.ebeMultiply(left, right));
    }
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    assert!(
        !resolved.diagnostics.has_errors(),
        "{:?}",
        resolved.diagnostics
    );

    let ty = &resolved.program.types[0];
    let body = ty
        .members
        .iter()
        .find_map(|member| match member {
            Member::Method(method) if method.name == "run" => method.body.as_ref(),
            _ => None,
        })
        .unwrap();
    let Stmt::VarDecl { initializer, .. } = &body.statements[0] else {
        panic!("expected local assignment");
    };
    let Some(Expr::Call {
        resolved: Some(call),
        ..
    }) = initializer
    else {
        panic!("expected resolved sum call");
    };

    assert_eq!(call.declaring_type, "ExternalArrayMathResultResolution");
    assert_eq!(call.rust_name, "sum_double_array");
}

#[test]
fn resolver_uses_core_enum_and_quality_utility_result_types_for_overloads() {
    let parsed = parse_java_source(
        r#"
class CoreEnumAndQualityUtilityResolution {
    void acceptIndex(int index, String label) {}
    void acceptIndex(String index, String label) {}
    void acceptProbability(double probability) {}
    void acceptProbability(String probability) {}

    void run(ExternalEnumLike state, byte quality, VariantRecord variant) {
        acceptIndex(state.ordinal(), "state");
        acceptProbability(QualityUtils.qualToProb(quality));
        acceptProbability(variant.getPhredScaledQual());
    }
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    assert!(
        !resolved.diagnostics.has_errors(),
        "{:?}",
        resolved.diagnostics
    );

    let body = resolved.program.types[0]
        .members
        .iter()
        .find_map(|member| match member {
            Member::Method(method) if method.name == "run" => method.body.as_ref(),
            _ => None,
        })
        .unwrap();
    let calls = body
        .statements
        .iter()
        .filter_map(|stmt| match stmt {
            Stmt::Expr(Expr::Call {
                resolved: Some(call),
                ..
            }) => Some(call.rust_name.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>();

    assert_eq!(
        calls,
        vec![
            "accept_index_int_string",
            "accept_probability_double",
            "accept_probability_double"
        ]
    );
}

#[test]
fn resolver_uses_external_cigar_and_string_utility_result_types_for_overloads() {
    let parsed = parse_java_source(
        r#"
class ExternalCigarAndStringUtilityResolution {
    void acceptElements(List<CigarElement> elements) {}
    void acceptElements(String elements) {}
    void acceptJoined(String value) {}
    void acceptJoined(int value) {}
    void acceptSplit(List<String> values) {}
    void acceptSplit(String values) {}

    void run(ExternalRead read, Object[] columns, String text) {
        acceptElements(read.getCigarElements());
        acceptJoined(Utils.join("\t", columns));
        acceptSplit(Utils.split(text, ','));
    }
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    assert!(
        !resolved.diagnostics.has_errors(),
        "{:?}",
        resolved.diagnostics
    );

    let body = resolved.program.types[0]
        .members
        .iter()
        .find_map(|member| match member {
            Member::Method(method) if method.name == "run" => method.body.as_ref(),
            _ => None,
        })
        .unwrap();
    let calls = body
        .statements
        .iter()
        .filter_map(|stmt| match stmt {
            Stmt::Expr(Expr::Call {
                resolved: Some(call),
                ..
            }) => Some(call.rust_name.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>();

    assert_eq!(
        calls,
        vec![
            "accept_elements_list",
            "accept_joined_string",
            "accept_split_list"
        ]
    );
}

#[test]
fn resolver_uses_external_predicate_naming_for_boolean_overloads() {
    let parsed = parse_java_source(
        r#"
class ExternalPredicateArgumentResolution {
    void validateArg(boolean condition, String message) {}
    void validateArg(String condition, String message) {}

    void run(GraphLike graph, Vertex vertex, Marker marker) {
        validateArg(graph.containsVertex(vertex), "missing vertex");
        validateArg(marker.isNonReference(), "unexpected marker");
        validateArg(marker.hasCoverage(), "missing coverage");
    }
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    assert!(
        !resolved.diagnostics.has_errors(),
        "{:?}",
        resolved.diagnostics
    );

    let ty = &resolved.program.types[0];
    let body = ty
        .members
        .iter()
        .find_map(|member| match member {
            Member::Method(method) if method.name == "run" => method.body.as_ref(),
            _ => None,
        })
        .unwrap();
    let calls = body
        .statements
        .iter()
        .filter_map(|stmt| match stmt {
            Stmt::Expr(Expr::Call {
                resolved: Some(call),
                ..
            }) => Some(call.rust_name.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>();

    assert_eq!(
        calls,
        vec![
            "validate_arg_boolean_string",
            "validate_arg_boolean_string",
            "validate_arg_boolean_string",
        ]
    );
}

#[test]
fn resolver_uses_external_count_array_getters_for_numeric_array_overloads() {
    let parsed = parse_java_source(
        r#"
class ExternalCountArrayGetterResolution {
    double sum(int[] values) { return 0.0; }
    double sum(String values) { return 0.0; }

    void run(DepthRecord record) {
        double total = sum(record.getAD());
    }
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    assert!(
        !resolved.diagnostics.has_errors(),
        "{:?}",
        resolved.diagnostics
    );

    let ty = &resolved.program.types[0];
    let body = ty
        .members
        .iter()
        .find_map(|member| match member {
            Member::Method(method) if method.name == "run" => method.body.as_ref(),
            _ => None,
        })
        .unwrap();
    let calls = body
        .statements
        .iter()
        .filter_map(|stmt| match stmt {
            Stmt::VarDecl {
                initializer:
                    Some(Expr::Call {
                        resolved: Some(call),
                        ..
                    }),
                ..
            } => Some(call.rust_name.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>();

    assert_eq!(calls, vec!["sum_int_array"]);
}

#[test]
fn resolver_uses_external_byte_array_getters_and_transforms_for_overloads() {
    let parsed = parse_java_source(
        r#"
class ExternalByteArrayGetterResolution {
    void acceptBytes(byte[] bases) {}
    void acceptBytes(String bases) {}

    void run(ExternalSequence sequence) {
        acceptBytes(ReadUtils.getBasesReverseComplement(sequence.getBases()));
    }
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    assert!(
        !resolved.diagnostics.has_errors(),
        "{:?}",
        resolved.diagnostics
    );

    let ty = &resolved.program.types[0];
    let body = ty
        .members
        .iter()
        .find_map(|member| match member {
            Member::Method(method) if method.name == "run" => method.body.as_ref(),
            _ => None,
        })
        .unwrap();
    let Stmt::Expr(Expr::Call {
        resolved: Some(call),
        ..
    }) = &body.statements[0]
    else {
        panic!("expected resolved byte-array overload");
    };

    assert_eq!(call.declaring_type, "ExternalByteArrayGetterResolution");
    assert_eq!(call.rust_name, "accept_bytes_byte_array");
}

#[test]
fn resolver_uses_external_scalar_length_result_for_int_overloads() {
    let parsed = parse_java_source(
        r#"
class ExternalScalarLengthResolution {
    void acceptLength(int length) {}
    void acceptLength(String length) {}

    void run(ExternalSequence sequence) {
        acceptLength(sequence.length());
    }
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    assert!(
        !resolved.diagnostics.has_errors(),
        "{:?}",
        resolved.diagnostics
    );

    let ty = &resolved.program.types[0];
    let body = ty
        .members
        .iter()
        .find_map(|member| match member {
            Member::Method(method) if method.name == "run" => method.body.as_ref(),
            _ => None,
        })
        .unwrap();
    let Stmt::Expr(Expr::Call {
        resolved: Some(call),
        ..
    }) = &body.statements[0]
    else {
        panic!("expected resolved int overload from external length() result");
    };

    assert_eq!(call.declaring_type, "ExternalScalarLengthResolution");
    assert_eq!(call.rust_name, "accept_length_int");
}

#[test]
fn resolver_uses_known_exception_hierarchy_for_constructor_assignability() {
    let parsed = parse_java_source(
        r#"
class WrappedFailure {
    WrappedFailure(String message) {}
    WrappedFailure(String message, Throwable cause) {}
}

class UseExceptionCauseConstructor {
    WrappedFailure make(IOException cause) {
        return new WrappedFailure("failed", cause);
    }
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    assert!(
        !resolved.diagnostics.has_errors(),
        "{:?}",
        resolved.diagnostics
    );

    let Expr::New {
        resolved: Some(constructor),
        ..
    } = first_return_expr(&resolved.program.types[1]).unwrap()
    else {
        panic!("expected resolved exception-cause constructor");
    };

    assert_eq!(constructor.declaring_type, "WrappedFailure");
    assert_eq!(constructor.rust_name, "new_string_throwable");
}

#[test]
fn resolver_uses_known_error_hierarchy_for_throwable_overload_assignability() {
    let parsed = parse_java_source(
        r#"
class NonUserFailureHandler {
    void handle(String message) {}
    void handle(Exception exception) {}
    void handle(Throwable throwable) {}

    void run(OutOfMemoryError error) {
        handle(error);
    }
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    assert!(
        !resolved.diagnostics.has_errors(),
        "{:?}",
        resolved.diagnostics
    );

    let Stmt::Expr(Expr::Call {
        resolved: Some(call),
        ..
    }) = first_stmt(&resolved.program.types[0]).unwrap()
    else {
        panic!("expected resolved throwable overload call");
    };

    assert_eq!(call.declaring_type, "NonUserFailureHandler");
    assert_eq!(call.rust_name, "handle_throwable");
}

#[test]
fn resolver_uses_io_exception_subclass_hierarchy_for_constructor_assignability() {
    let parsed = parse_java_source(
        r#"
class WrappedInputFailure {
    WrappedInputFailure(File file, String message) {}
    WrappedInputFailure(File file, IOException cause) {}
}

class UseIoExceptionSubclassConstructor {
    WrappedInputFailure make(File file, FileNotFoundException cause) {
        return new WrappedInputFailure(file, cause);
    }
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    assert!(
        !resolved.diagnostics.has_errors(),
        "{:?}",
        resolved.diagnostics
    );

    let Expr::New {
        resolved: Some(constructor),
        ..
    } = first_return_expr(&resolved.program.types[1]).unwrap()
    else {
        panic!("expected resolved io-exception-subclass constructor");
    };

    assert_eq!(constructor.declaring_type, "WrappedInputFailure");
    assert_eq!(constructor.rust_name, "new_file_i_o_exception");
}

#[test]
fn resolver_uses_known_external_exception_hierarchy_for_constructor_assignability() {
    let parsed = parse_java_source(
        r#"
class WrappedExternalFailure {
    WrappedExternalFailure(String message, Exception cause) {}
    WrappedExternalFailure(String message, String cause) {}
}

class UseExternalExceptionCauseConstructor {
    WrappedExternalFailure make(SAMException cause) {
        return new WrappedExternalFailure("failed", cause);
    }
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    assert!(
        !resolved.diagnostics.has_errors(),
        "{:?}",
        resolved.diagnostics
    );

    let Expr::New {
        resolved: Some(constructor),
        ..
    } = first_return_expr(&resolved.program.types[1]).unwrap()
    else {
        panic!("expected resolved external-exception constructor");
    };

    assert_eq!(constructor.declaring_type, "WrappedExternalFailure");
    assert_eq!(constructor.rust_name, "new_string_exception");
}

#[test]
fn resolver_uses_known_runtime_exception_hierarchy_for_throwable_constructor_assignability() {
    let parsed = parse_java_source(
        r#"
class WrappedCloudFailure {
    WrappedCloudFailure(Path path, String message, Throwable cause) {}
    WrappedCloudFailure(Path path, String message, String cause) {}
}

class UseRuntimeExceptionCauseConstructor {
    WrappedCloudFailure make(Path path, StorageException cause) {
        return new WrappedCloudFailure(path, "failed", cause);
    }
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    assert!(
        !resolved.diagnostics.has_errors(),
        "{:?}",
        resolved.diagnostics
    );

    let Expr::New {
        resolved: Some(constructor),
        ..
    } = first_return_expr(&resolved.program.types[1]).unwrap()
    else {
        panic!("expected resolved runtime-exception constructor");
    };

    assert_eq!(constructor.declaring_type, "WrappedCloudFailure");
    assert_eq!(constructor.rust_name, "new_path_string_throwable");
}

#[test]
fn resolver_uses_throwable_get_message_result_type_for_constructor_arguments() {
    let parsed = parse_java_source(
        r#"
class WrappedInputFailure {
    WrappedInputFailure(Path path, String message, IOException cause) {}
    WrappedInputFailure(Path path, int code, String message) {}
}

class UseExceptionMessageConstructor {
    WrappedInputFailure make(Path path, IOException cause) {
        return new WrappedInputFailure(path, cause.getMessage(), cause);
    }
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    assert!(
        !resolved.diagnostics.has_errors(),
        "{:?}",
        resolved.diagnostics
    );

    let Expr::New {
        resolved: Some(constructor),
        ..
    } = first_return_expr(&resolved.program.types[1]).unwrap()
    else {
        panic!("expected resolved exception-message constructor");
    };

    assert_eq!(constructor.declaring_type, "WrappedInputFailure");
    assert_eq!(constructor.rust_name, "new_path_string_i_o_exception");
}

#[test]
fn resolver_uses_boxed_primitive_unboxing_for_overload_arguments() {
    let parsed = parse_java_source(
        r#"
class BoxedPrimitiveArgumentResolution {
    void requirePositive(int value, String name) {}
    void requirePositive(String value, String name) {}

    void run(Integer value) {
        requirePositive(value, "value");
    }
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    assert!(
        !resolved.diagnostics.has_errors(),
        "{:?}",
        resolved.diagnostics
    );

    let Stmt::Expr(Expr::Call {
        resolved: Some(call),
        ..
    }) = first_stmt(&resolved.program.types[0]).unwrap()
    else {
        panic!("expected resolved boxed primitive overload");
    };

    assert_eq!(call.declaring_type, "BoxedPrimitiveArgumentResolution");
    assert_eq!(call.rust_name, "require_positive_int_string");
}

#[test]
fn resolver_uses_collection_superinterface_generic_assignability_for_overloads() {
    let parsed = parse_java_source(
        r#"
class CollectionInterfaceArgumentResolution {
    void join(CharSequence separator, Iterable<? extends CharSequence> values) {}
    void join(CharSequence separator, int value) {}

    void run(Set<String> names) {
        join(",", names);
    }
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    assert!(
        !resolved.diagnostics.has_errors(),
        "{:?}",
        resolved.diagnostics
    );

    let Stmt::Expr(Expr::Call {
        resolved: Some(call),
        ..
    }) = first_stmt(&resolved.program.types[0]).unwrap()
    else {
        panic!("expected resolved collection-interface overload");
    };

    assert_eq!(call.declaring_type, "CollectionInterfaceArgumentResolution");
    assert_eq!(call.rust_name, "join_char_sequence_iterable");
}

#[test]
fn resolver_treats_source_enum_types_as_assignable_to_enum_api_overloads() {
    let parsed = parse_java_source(
        r#"
enum Column {
    VALUE
}

class Row {
    double getDouble(Enum<?> column) {
        return 1.0;
    }

    double getDouble(int index) {
        return 2.0;
    }
}

class EnumApiArgumentResolution {
    double read(Row row, Column column) {
        return row.getDouble(column);
    }
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    assert!(
        !resolved.diagnostics.has_errors(),
        "{:?}",
        resolved.diagnostics
    );

    let Expr::Call {
        resolved: Some(call),
        ..
    } = first_return_expr(&resolved.program.types[2]).unwrap()
    else {
        panic!("expected resolved enum API overload");
    };

    assert_eq!(call.declaring_type, "Row");
    assert_eq!(call.rust_name, "get_double_enum");
}

#[test]
fn resolver_uses_non_null_passthrough_result_type_for_overload_arguments() {
    let parsed = parse_java_source(
        r#"
class NonNullPassthroughArgumentResolution {
    void acceptBytes(byte[] bases, String label) {}
    void acceptBytes(String bases, String label) {}

    void run(byte[] bases) {
        acceptBytes(nonNull(bases, "bases"), "ok");
    }
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    assert!(
        !resolved.diagnostics.has_errors(),
        "{:?}",
        resolved.diagnostics
    );

    let ty = &resolved.program.types[0];
    let body = ty
        .members
        .iter()
        .find_map(|member| match member {
            Member::Method(method) if method.name == "run" => method.body.as_ref(),
            _ => None,
        })
        .unwrap();
    let Stmt::Expr(Expr::Call {
        resolved: Some(call),
        ..
    }) = &body.statements[0]
    else {
        panic!("expected resolved nonNull passthrough overload call");
    };

    assert_eq!(call.declaring_type, "NonNullPassthroughArgumentResolution");
    assert_eq!(call.rust_name, "accept_bytes_byte_array_string");
}

#[test]
fn resolver_allows_boxed_primitive_actual_for_method_type_variable_parameter() {
    let parsed = parse_java_source(
        r#"
class GenericMethodPrimitiveBoxingResolution {
    <T> T require(T value, String message) { return value; }
    String require(String value, int code) { return value; }

    void acceptBoolean(boolean value) {}
    void acceptString(String value) {}

    void run(boolean ok) {
        acceptBoolean(require(ok, "ok"));
    }
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    assert!(
        !resolved.diagnostics.has_errors(),
        "{:?}",
        resolved.diagnostics
    );

    let ty = &resolved.program.types[0];
    let body = ty
        .members
        .iter()
        .find_map(|member| match member {
            Member::Method(method) if method.name == "run" => method.body.as_ref(),
            _ => None,
        })
        .unwrap();
    let Stmt::Expr(Expr::Call {
        resolved: Some(call),
        ..
    }) = &body.statements[0]
    else {
        panic!("expected resolved acceptBoolean call");
    };

    assert_eq!(call.rust_name, "accept_boolean");
}

#[test]
fn resolver_uses_known_external_to_string_result_type_after_unresolved_call_receiver() {
    let parsed = parse_java_source(
        r#"
class ExternalToStringChainArgumentResolution {
    void setColumn(int index, String value) {}
    void setColumn(int index, int value) {}

    void run(ExternalRecord record) {
        setColumn(0, record.status().toString());
    }
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    assert!(
        !resolved.diagnostics.has_errors(),
        "{:?}",
        resolved.diagnostics
    );

    let Stmt::Expr(Expr::Call {
        resolved: Some(call),
        ..
    }) = first_stmt(&resolved.program.types[0]).unwrap()
    else {
        panic!("expected resolved toString chain overload call");
    };

    assert_eq!(
        call.declaring_type,
        "ExternalToStringChainArgumentResolution"
    );
    assert_eq!(call.rust_name, "set_column_int_string");
}

#[test]
fn resolver_uses_known_file_path_result_type_after_non_null_passthrough() {
    let parsed = parse_java_source(
        r#"
class InputSource {
    InputSource(String path, String name) {}
    InputSource(File file, int queryLookahead) {}
}

class ExternalFilePathArgumentResolution {
    InputSource make(File file) {
        return new InputSource(nonNull(file).getAbsolutePath(), "features");
    }
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    assert!(
        !resolved.diagnostics.has_errors(),
        "{:?}",
        resolved.diagnostics
    );

    let Expr::New {
        resolved: Some(constructor),
        ..
    } = first_return_expr(&resolved.program.types[1]).unwrap()
    else {
        panic!("expected resolved constructor from getAbsolutePath result");
    };

    assert_eq!(constructor.declaring_type, "InputSource");
    assert_eq!(constructor.rust_name, "new_string_string");
}

#[test]
fn resolver_uses_known_file_get_path_result_type_for_overload_arguments() {
    let parsed = parse_java_source(
        r#"
class FilePathGetterArgumentResolution {
    void accept(String path) {}
    void accept(File path) {}
    void run(File file) {
        accept(IOUtils.replaceExtension(file.getPath(), FileExtensions.BAI_INDEX));
    }
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    assert!(
        !resolved.diagnostics.has_errors(),
        "{:?}",
        resolved.diagnostics
    );

    let Stmt::Expr(Expr::Call {
        resolved: Some(call),
        ..
    }) = first_stmt(&resolved.program.types[0]).unwrap()
    else {
        panic!("expected resolved File.getPath overload call");
    };

    assert_eq!(call.declaring_type, "FilePathGetterArgumentResolution");
    assert_eq!(call.rust_name, "accept_string");
}

#[test]
fn resolver_treats_package_qualified_external_static_utility_as_type_qualifier() {
    let parsed = parse_java_source(
        r#"
class PackageQualifiedExternalStaticUtilityResolution {
    void run(InputStream input, OutputStream output) {
        org.apache.commons.io.IOUtils.copy(input, output);
    }
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    assert!(
        !resolved.diagnostics.has_errors(),
        "{:?}",
        resolved.diagnostics
    );

    let unresolved_fields = resolved
        .facts
        .unresolved_target_facts
        .iter()
        .filter(|fact| fact.kind == UnresolvedTargetKind::FieldAccess)
        .collect::<Vec<_>>();
    assert!(
        unresolved_fields.is_empty(),
        "package-qualified external type names should not be recorded as field misses: {unresolved_fields:?}"
    );

    let method_facts = resolved
        .facts
        .unresolved_target_facts
        .iter()
        .filter(|fact| fact.kind == UnresolvedTargetKind::MethodCall)
        .collect::<Vec<_>>();
    assert_eq!(method_facts.len(), 1);
    assert_eq!(method_facts[0].target_name, "copy");
    assert_eq!(
        method_facts[0].receiver_type,
        Some(JavaType::Unknown("IOUtils".to_string()))
    );
}

#[test]
fn resolver_treats_package_qualified_scala_option_as_external_type_qualifier() {
    let parsed = parse_java_source(
        r#"
class ScalaOptionQualifierResolution {
    void accept(Option<String> value) {}
    void accept(String value) {}

    void run(String value) {
        accept(scala.Option.apply(value));
    }
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    assert!(
        !resolved.diagnostics.has_errors(),
        "{:?}",
        resolved.diagnostics
    );

    let unresolved_fields = resolved
        .facts
        .unresolved_target_facts
        .iter()
        .filter(|fact| fact.kind == UnresolvedTargetKind::FieldAccess)
        .collect::<Vec<_>>();
    assert!(
        unresolved_fields.is_empty(),
        "scala.Option should be modeled as an external type qualifier: {unresolved_fields:?}"
    );

    let Stmt::Expr(Expr::Call {
        resolved: Some(call),
        ..
    }) = first_stmt(&resolved.program.types[0]).unwrap()
    else {
        panic!("expected resolved scala Option overload call");
    };

    assert_eq!(call.rust_name, "accept_option");
}

#[test]
fn resolver_uses_scala_object_module_singleton_field_for_factory_calls() {
    let parsed = parse_java_source(
        r#"
class ScalaObjectModuleSingletonResolution {
    void accept(ClassTag value) {}
    void accept(String value) {}

    void run(Class cls) {
        accept(ClassTag$.MODULE$.apply(cls));
    }
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    assert!(
        !resolved.diagnostics.has_errors(),
        "{:?}",
        resolved.diagnostics
    );

    let unresolved_fields = resolved
        .facts
        .unresolved_target_facts
        .iter()
        .filter(|fact| fact.kind == UnresolvedTargetKind::FieldAccess)
        .collect::<Vec<_>>();
    assert!(
        unresolved_fields.is_empty(),
        "Scala object MODULE$ singleton fields should be modeled as external fields: {unresolved_fields:?}"
    );

    let Stmt::Expr(Expr::Call {
        resolved: Some(call),
        ..
    }) = first_stmt(&resolved.program.types[0]).unwrap()
    else {
        panic!("expected resolved Scala ClassTag overload call");
    };

    assert_eq!(call.rust_name, "accept_class_tag");
}

#[test]
fn resolver_uses_known_external_static_alignment_constants_for_overload_arguments() {
    let parsed = parse_java_source(
        r#"
class ExternalAlignmentConstantArgumentResolution {
    void acceptStart(int start) {}
    void acceptStart(String start) {}

    void acceptReferenceIndex(int referenceIndex) {}
    void acceptReferenceIndex(String referenceIndex) {}

    void acceptReferenceName(String referenceName) {}
    void acceptReferenceName(int referenceName) {}

    void run() {
        acceptStart(SAMRecord.NO_ALIGNMENT_START);
        acceptReferenceIndex(htsjdk.samtools.SAMRecord.NO_ALIGNMENT_REFERENCE_INDEX);
        acceptReferenceName(SAMRecord.NO_ALIGNMENT_REFERENCE_NAME);
    }
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    assert!(
        !resolved.diagnostics.has_errors(),
        "{:?}",
        resolved.diagnostics
    );

    let unresolved_fields = resolved
        .facts
        .unresolved_target_facts
        .iter()
        .filter(|fact| fact.kind == UnresolvedTargetKind::FieldAccess)
        .collect::<Vec<_>>();
    assert!(
        unresolved_fields.is_empty(),
        "known external static alignment constants should not be field misses: {unresolved_fields:?}"
    );

    let calls = resolved.program.types[0]
        .members
        .iter()
        .find_map(|member| match member {
            Member::Method(method) if method.name == "run" => method.body.as_ref(),
            _ => None,
        })
        .unwrap()
        .statements
        .iter()
        .filter_map(|stmt| match stmt {
            Stmt::Expr(Expr::Call {
                resolved: Some(call),
                ..
            }) => Some(call.rust_name.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>();

    assert_eq!(
        calls,
        vec![
            "accept_start_int",
            "accept_reference_index_int",
            "accept_reference_name_string",
        ]
    );
}

#[test]
fn resolver_uses_external_nested_enum_and_static_numeric_constants_for_overloads() {
    let parsed = parse_java_source(
        r#"
class ExternalNestedEnumAndNumericConstantResolution {
    void acceptVariant(Type ty) {}
    void acceptVariant(String ty) {}

    void acceptPolicy(DuplicateTaggingPolicy policy) {}
    void acceptPolicy(String policy) {}

    void acceptHeaderVersion(VCFHeaderVersion version) {}
    void acceptHeaderVersion(String version) {}

    void acceptLength(int length) {}
    void acceptLength(String length) {}

    void run() {
        acceptVariant(VariantContext.Type.INDEL);
        acceptVariant(VariantContext.Type.NO_VARIATION);
        acceptPolicy(MarkDuplicates.DuplicateTaggingPolicy.DontTag);
        acceptPolicy(MarkDuplicates.DuplicateTaggingPolicy.OpticalOnly);
        acceptHeaderVersion(VCFHeaderVersion.VCF4_2);
        acceptLength(BlockCompressedStreamConstants.BLOCK_HEADER_LENGTH);
    }
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    assert!(
        !resolved.diagnostics.has_errors(),
        "{:?}",
        resolved.diagnostics
    );

    let unresolved_fields = resolved
        .facts
        .unresolved_target_facts
        .iter()
        .filter(|fact| fact.kind == UnresolvedTargetKind::FieldAccess)
        .collect::<Vec<_>>();
    assert!(
        unresolved_fields.is_empty(),
        "curated nested enum/static constants should not be field misses: {unresolved_fields:?}"
    );

    let calls = resolved.program.types[0]
        .members
        .iter()
        .find_map(|member| match member {
            Member::Method(method) if method.name == "run" => method.body.as_ref(),
            _ => None,
        })
        .unwrap()
        .statements
        .iter()
        .filter_map(|stmt| match stmt {
            Stmt::Expr(Expr::Call {
                resolved: Some(call),
                ..
            }) => Some(call.rust_name.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>();

    assert_eq!(
        calls,
        vec![
            "accept_variant_type_",
            "accept_variant_type_",
            "accept_policy_duplicate_tagging_policy",
            "accept_policy_duplicate_tagging_policy",
            "accept_header_version_v_c_f_header_version",
            "accept_length_int",
        ]
    );
}

#[test]
fn resolver_uses_external_enum_constant_receiver_for_predicate_overloads() {
    let parsed = parse_java_source(
        r#"
class ExternalEnumPredicateReceiverResolution {
    void accept(boolean selected) {}
    void accept(String selected) {}

    void run(int flag) {
        accept(SAMFlag.SECONDARY_ALIGNMENT.isUnset(flag));
        accept(SAMFlag.READ_UNMAPPED.isSet(flag));
    }
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    assert!(
        !resolved.diagnostics.has_errors(),
        "{:?}",
        resolved.diagnostics
    );

    let unresolved_fields = resolved
        .facts
        .unresolved_target_facts
        .iter()
        .filter(|fact| fact.kind == UnresolvedTargetKind::FieldAccess)
        .collect::<Vec<_>>();
    assert!(
        unresolved_fields.is_empty(),
        "external enum constants used as method receivers should not be field misses: {unresolved_fields:?}"
    );

    let calls = resolved.program.types[0]
        .members
        .iter()
        .find_map(|member| match member {
            Member::Method(method) if method.name == "run" => method.body.as_ref(),
            _ => None,
        })
        .unwrap()
        .statements
        .iter()
        .filter_map(|stmt| match stmt {
            Stmt::Expr(Expr::Call {
                resolved: Some(call),
                ..
            }) => Some(call.rust_name.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>();

    assert_eq!(calls, vec!["accept_boolean", "accept_boolean"]);
}

#[test]
fn resolver_uses_source_enum_values_array_type_for_element_field_access() {
    let parsed = parse_java_source(
        r#"
enum Encoded {
    POSITIVE;

    char code;

    static void accept(char value) {}
    static void accept(String value) {}

    static void run() {
        accept(values()[0].code);
    }
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    assert!(
        !resolved.diagnostics.has_errors(),
        "{:?}",
        resolved.diagnostics
    );

    let unresolved_fields = resolved
        .facts
        .unresolved_target_facts
        .iter()
        .filter(|fact| fact.kind == UnresolvedTargetKind::FieldAccess)
        .collect::<Vec<_>>();
    assert!(
        unresolved_fields.is_empty(),
        "source enum values() should expose enum elements for field lookup: {unresolved_fields:?}"
    );

    let body = resolved.program.types[0]
        .members
        .iter()
        .find_map(|member| match member {
            Member::Method(method) if method.name == "run" => method.body.as_ref(),
            _ => None,
        })
        .unwrap();
    let Stmt::Expr(Expr::Call {
        resolved: Some(call),
        ..
    }) = &body.statements[0]
    else {
        panic!("expected resolved overload from enum values element field");
    };

    assert_eq!(call.rust_name, "accept_char");
}

#[test]
fn resolver_uses_stream_of_array_element_type_for_lambda_field_access() {
    let parsed = parse_java_source(
        r#"
enum Encoded {
    POSITIVE;

    char code;

    static void accept(IntStream values) {}
    static void accept(Stream<String> values) {}

    static void run() {
        accept(Stream.of(values()).mapToInt(value -> value.code));
    }
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    assert!(
        !resolved.diagnostics.has_errors(),
        "{:?}",
        resolved.diagnostics
    );

    let unresolved_fields = resolved
        .facts
        .unresolved_target_facts
        .iter()
        .filter(|fact| fact.kind == UnresolvedTargetKind::FieldAccess)
        .collect::<Vec<_>>();
    assert!(
        unresolved_fields.is_empty(),
        "Stream.of(array) should expose array elements to lambda bodies: {unresolved_fields:?}"
    );

    let body = resolved.program.types[0]
        .members
        .iter()
        .find_map(|member| match member {
            Member::Method(method) if method.name == "run" => method.body.as_ref(),
            _ => None,
        })
        .unwrap();
    let Stmt::Expr(Expr::Call {
        resolved: Some(call),
        ..
    }) = &body.statements[0]
    else {
        panic!("expected resolved overload from stream array element field");
    };

    assert_eq!(call.rust_name, "accept_int_stream");
}

#[test]
fn resolver_uses_jdk_nested_enum_constants_for_enum_set_overloads() {
    let parsed = parse_java_source(
        r#"
class JdkNestedEnumSetConstantResolution {
    void acceptSet(EnumSet<Characteristics> values) {}
    void acceptSet(String values) {}

    void acceptCharacteristic(Characteristics value) {}
    void acceptCharacteristic(String value) {}

    void run() {
        acceptSet(EnumSet.of(Characteristics.IDENTITY_FINISH, Characteristics.UNORDERED));
        acceptCharacteristic(Characteristics.UNORDERED);
    }
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    assert!(
        !resolved.diagnostics.has_errors(),
        "{:?}",
        resolved.diagnostics
    );

    let unresolved_fields = resolved
        .facts
        .unresolved_target_facts
        .iter()
        .filter(|fact| fact.kind == UnresolvedTargetKind::FieldAccess)
        .collect::<Vec<_>>();
    assert!(
        unresolved_fields.is_empty(),
        "JDK nested enum constants should not be field misses: {unresolved_fields:?}"
    );

    let calls = resolved.program.types[0]
        .members
        .iter()
        .find_map(|member| match member {
            Member::Method(method) if method.name == "run" => method.body.as_ref(),
            _ => None,
        })
        .unwrap()
        .statements
        .iter()
        .filter_map(|stmt| match stmt {
            Stmt::Expr(Expr::Call {
                resolved: Some(call),
                ..
            }) => Some(call.rust_name.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>();

    assert_eq!(
        calls,
        vec![
            "accept_set_enum_set",
            "accept_characteristic_characteristics",
        ]
    );
}

#[test]
fn resolver_uses_external_library_string_constants_for_overloads() {
    let parsed = parse_java_source(
        r#"
class ExternalLibraryStringConstantResolution {
    void accept(String value) {}
    void accept(int value) {}

    void run() {
        accept(Constants.CHROMOSOME_FOLDER_DELIMITER_SYMBOL_REGEX);
    }
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    assert!(
        !resolved.diagnostics.has_errors(),
        "{:?}",
        resolved.diagnostics
    );

    let unresolved_fields = resolved
        .facts
        .unresolved_target_facts
        .iter()
        .filter(|fact| fact.kind == UnresolvedTargetKind::FieldAccess)
        .collect::<Vec<_>>();
    assert!(
        unresolved_fields.is_empty(),
        "known external library constants should not be field misses: {unresolved_fields:?}"
    );

    let Stmt::Expr(Expr::Call {
        resolved: Some(call),
        ..
    }) = first_stmt(&resolved.program.types[0]).unwrap()
    else {
        panic!("expected resolved overload from external string constant");
    };

    assert_eq!(call.rust_name, "accept_string");
}

#[test]
fn resolver_uses_external_domain_constants_for_primitive_and_string_overloads() {
    let parsed = parse_java_source(
        r#"
class ExternalDomainConstantArgumentResolution {
    void acceptString(String value) {}
    void acceptString(int value) {}

    void acceptChar(char value) {}
    void acceptChar(String value) {}

    void acceptInt(int value) {}
    void acceptInt(String value) {}

    void acceptDouble(double value) {}
    void acceptDouble(String value) {}

    void acceptLong(long value) {}
    void acceptLong(String value) {}

    void acceptBytes(byte[] value) {}
    void acceptBytes(String value) {}

    void acceptReadFormat(ReadsFormatWriteOption value) {}
    void acceptReadFormat(String value) {}

    void acceptStringSet(Set<String> value) {}
    void acceptStringSet(String value) {}

    void acceptHeaderField(HEADER_FIELDS value) {}
    void acceptHeaderField(String value) {}

    void acceptHeaderFieldArray(HEADER_FIELDS[] value) {}
    void acceptHeaderFieldArray(String value) {}

    void acceptTimeUnit(TimeUnit value) {}
    void acceptTimeUnit(String value) {}

    void acceptOutputType(OutputType value) {}
    void acceptOutputType(String value) {}

    void acceptOption(Option value) {}
    void acceptOption(String value) {}

    void acceptHeaderLineType(SupportedHeaderLineType value) {}
    void acceptHeaderLineType(String value) {}

    void acceptEnumSet(EnumSet<Options> value) {}
    void acceptEnumSet(String value) {}

    void acceptSettings(HttpFileSystemProviderSettings value) {}
    void acceptSettings(String value) {}

    void acceptBoolean(boolean value) {}
    void acceptBoolean(String value) {}

    void acceptLevel(Level value) {}
    void acceptLevel(String value) {}

    void acceptLocale(Locale value) {}
    void acceptLocale(String value) {}

    void acceptElementKind(ElementKind value) {}
    void acceptElementKind(String value) {}

    void acceptExportConfiguration(ExportConfiguration value) {}
    void acceptExportConfiguration(String value) {}

    void acceptIndexType(IndexType value) {}
    void acceptIndexType(String value) {}

    void acceptIndexBalance(IndexBalanceApproach value) {}
    void acceptIndexBalance(String value) {}

    void acceptStandardOpenOption(StandardOpenOption value) {}
    void acceptStandardOpenOption(String value) {}

    void acceptAccessMode(AccessMode value) {}
    void acceptAccessMode(String value) {}

    void acceptPosixFilePermission(PosixFilePermission value) {}
    void acceptPosixFilePermission(String value) {}

    void acceptHttpMethod(HttpMethod value) {}
    void acceptHttpMethod(String value) {}

    void acceptMapperFeature(MapperFeature value) {}
    void acceptMapperFeature(String value) {}

    void acceptDataFormat(DataFormat value) {}
    void acceptDataFormat(String value) {}

    void acceptDataSeriesType(DataSeriesType value) {}
    void acceptDataSeriesType(String value) {}

    void acceptCloudStorageConfiguration(CloudStorageConfiguration value) {}
    void acceptCloudStorageConfiguration(String value) {}

    void acceptOutputStream(OutputStream value) {}
    void acceptOutputStream(String value) {}

    void acceptValidationStringency(ValidationStringency value) {}
    void acceptValidationStringency(String value) {}

    void acceptObject(Object value) {}
    void acceptObject(String value) {}

    void acceptFileCardinality(FileCardinalityWriteOption value) {}
    void acceptFileCardinality(String value) {}

    void acceptGenotypesContext(GenotypesContext value) {}
    void acceptGenotypesContext(String value) {}

    void acceptWriter(Writer value) {}
    void acceptWriter(String value) {}

    void acceptDeserializationFeature(DeserializationFeature value) {}
    void acceptDeserializationFeature(String value) {}

    void acceptJexlMissingValueTreatment(JexlMissingValueTreatment value) {}
    void acceptJexlMissingValueTreatment(String value) {}

    void run() {
        acceptString(VCFConstants.PASSES_FILTERS_v4);
        acceptString(VCFConstants.GENOTYPE_ALLELE_DEPTHS);
        acceptString(VCFConstants.SVTYPE);
        acceptString(VCFConstants.UNFILTERED);
        acceptChar(VCFConstants.INFO_FIELD_ARRAY_SEPARATOR_CHAR);
        acceptInt(VCFConstants.MAX_GENOTYPE_QUAL);
        acceptInt(GenotypeLikelihoods.MAX_DIPLOID_ALT_ALLELES_THAT_CAN_BE_GENOTYPED);
        acceptDouble(CommonInfo.NO_LOG10_PERROR);
        acceptBytes(BlockCompressedStreamConstants.EMPTY_GZIP_BLOCK);
        acceptInt(BlockCompressedStreamConstants.BLOCK_LENGTH_OFFSET);
        acceptString(VCFConstants.MISSING_VALUE_v4);
        acceptInt(SAMRecord.UNKNOWN_MAPPING_QUALITY);
        acceptInt(SAMRecord.NO_MAPPING_QUALITY);
        acceptBytes(SAMRecord.NULL_QUALS);
        acceptString(SAMRecord.NULL_QUALS_STRING);
        acceptInt(SAMUtils.MAX_PHRED_SCORE);
        acceptInt(SAMSequenceRecord.UNKNOWN_SEQUENCE_LENGTH);
        acceptString(TemplateProperties.ARGUMENTS_POSITIONAL);
        acceptString(TemplateProperties.ARGUMENT_DEFAULT_VALUE);
        acceptString(TemplateProperties.ARGUMENT_TYPE);
        acceptReadFormat(ReadsFormatWriteOption.BAM);
        acceptString(GenotypeConcordanceSummaryMetrics.GENOTYPE_CONCORDANCE);
        acceptString(GenotypeConcordanceSummaryMetrics.HET_PPV);
        acceptString(GenotypeConcordanceSummaryMetrics.VAR_SPECIFICITY);
        acceptHeaderField(VCFHeader.HEADER_FIELDS.INFO);
        acceptHeaderFieldArray(VCFHeader.HEADER_FIELDS.values());
        acceptString(VCFHeader.REFERENCE_KEY);
        acceptTimeUnit(TimeUnit.SECONDS);
        acceptOutputType(VariantContextWriterBuilder.OutputType);
        acceptString(ReservedTagConstants.XT);
        acceptBytes(SequenceUtil.VALID_BASES_UPPER);
        acceptInt(BAMFileConstants.FIXED_BLOCK_SIZE);
        acceptString(GATKConfig.CONFIG_FILE_VARIABLE_FILE_NAME);
        acceptString(CloudStorageFileSystem.URI_SCHEME);
        acceptInt(ReferenceContext.UNINITIALIZED_REFERENCE_ID);
        acceptInt(Slice.EMBEDDED_REFERENCE_ABSENT_CONTENT_ID);
        acceptInt(TabixFormat.GENERIC_FLAGS);
        acceptOption(Option.CACHE_FILE_BASED_INDEXES);
        acceptOption(Option.INCLUDE_SOURCE_IN_RECORDS);
        acceptHeaderLineType(SupportedHeaderLineType.FORMAT);
        acceptHeaderLineType(SupportedHeaderLineType.INFO);
        acceptString(GenotypeConcordance.CONTINGENCY_STATE_TAG);
        acceptEnumSet(VariantContextWriterBuilder.DEFAULT_OPTIONS);
        acceptString(HttpFileSystemProvider.SCHEME);
        acceptString(HttpsFileSystemProvider.SCHEME);
        acceptSettings(HttpFileSystemProviderSettings.DEFAULT_SETTINGS);
        acceptString(Allele.NON_REF_STRING);
        acceptString(Allele.UNSPECIFIED_ALTERNATE_ALLELE_STRING);
        acceptString(File.separator);
        acceptString(VCFConstants.NULL_ALLELE);
        acceptDouble(VariantContext.NO_LOG10_PERROR);
        acceptDouble(ReadLikelihoodCalculationEngine.DEFAULT_EXPECTED_ERROR_RATE_PER_BASE);
        acceptBoolean(SpecialArgumentsCollection.SHOW_HIDDEN);
        acceptChar(File.separatorChar);
        acceptInt(BlockCompressedStreamConstants.DEFAULT_COMPRESSION_LEVEL);
        acceptLong(SBIIndexWriter.DEFAULT_GRANULARITY);
        acceptInt(TarArchiveOutputStream.BIGNUMBER_STAR);
        acceptInt(Spliterator.IMMUTABLE);
        acceptInt(Spliterator.NONNULL);
        acceptStandardOpenOption(StandardOpenOption.CREATE_NEW);
        acceptAccessMode(AccessMode.READ);
        acceptAccessMode(AccessMode.WRITE);
        acceptPosixFilePermission(PosixFilePermission.GROUP_EXECUTE);
        acceptHttpMethod(HttpMethod.GET);
        acceptMapperFeature(MapperFeature.ACCEPT_CASE_INSENSITIVE_PROPERTIES);
        acceptDataFormat(DataFormat.AVRO);
        acceptDataSeriesType(DataSeriesType.BYTE_ARRAY);
        acceptCloudStorageConfiguration(CloudStorageConfiguration.DEFAULT);
        acceptOutputStream(NullOutputStream.INSTANCE);
        acceptOutputStream(NullOutputStream.NULL_OUTPUT_STREAM);
        acceptInt(TarArchiveOutputStream.LONGFILE_GNU);
        acceptDouble(Math.PI);
        acceptString(SystemUtils.OS_ARCH);
        acceptString(SystemUtils.OS_NAME);
        acceptBoolean(SystemUtils.IS_OS_MAC);
        acceptBoolean(SystemUtils.IS_OS_LINUX);
        acceptInt(Defaults.COMPRESSION_LEVEL);
        acceptBoolean(Defaults.USE_ASYNC_IO_READ_FOR_SAMTOOLS);
        acceptBoolean(Defaults.USE_ASYNC_IO_WRITE_FOR_SAMTOOLS);
        acceptBoolean(Defaults.USE_ASYNC_IO_WRITE_FOR_TRIBBLE);
        acceptObject(RetryHandler.DEFAULT_RETRYABLE_HTTP_CODES);
        acceptObject(RetryHandler.DEFAULT_RETRYABLE_EXCEPTIONS);
        acceptObject(RetryHandler.DEFALT_RETRYABLE_MESSAGES);
        acceptValidationStringency(ValidationStringency.LENIENT);
        acceptIndexType(IndexFactory.IndexType.TABIX);
        acceptIndexBalance(IndexFactory.IndexBalanceApproach.FOR_SEEK_TIME);
        acceptOption(SamReaderFactory.Option.CACHE_FILE_BASED_INDEXES);
        acceptOption(SamReaderFactory.Option.INCLUDE_SOURCE_IN_RECORDS);
        acceptHeaderLineType(VCFCompoundHeaderLine.SupportedHeaderLineType.INFO);
        acceptHeaderLineType(VCFCompoundHeaderLine.SupportedHeaderLineType.FORMAT);
        acceptInt(BwaMemAligner.MEM_F_ALL);
        acceptFileCardinality(FileCardinalityWriteOption.MULTIPLE);
        acceptFileCardinality(FileCardinalityWriteOption.SINGLE);
        acceptGenotypesContext(GenotypesContext.NO_GENOTYPES);
        acceptLong(FeatureCodecHeader.NO_HEADER_END);
        acceptString(MessageDigestAlgorithms.SHA_256);
        acceptString(SAMReadGroupRecord.READ_GROUP_ID_TAG);
        acceptString(SAMReadGroupRecord.READ_GROUP_SAMPLE_TAG);
        acceptWriter(NullWriter.INSTANCE);
        acceptDeserializationFeature(DeserializationFeature.UNWRAP_ROOT_VALUE);
        acceptJexlMissingValueTreatment(JexlMissingValueTreatment.TREAT_AS_MATCH);
        acceptJexlMissingValueTreatment(JexlMissingValueTreatment.TREAT_AS_MISMATCH);
        acceptString(WDLWorkUnitHandler.POSITIONAL_ARGS);
        acceptChar(Genotype.UNPHASED_ALLELE_SEPARATOR);
        acceptInt(TabixFormat.ZERO_BASED);
        acceptLevel(Level.DEBUG);
        acceptLevel(Level.INFO);
        acceptLevel(Level.ERROR);
        acceptLocale(Locale.ENGLISH);
        acceptString(FastqConstants.FIRST_OF_PAIR);
        acceptString(FastqConstants.SECOND_OF_PAIR);
        acceptElementKind(ElementKind.FIELD);
        acceptExportConfiguration(GenomicsDBExportConfiguration.ExportConfiguration);
        acceptInt(ReadLikelihoodCalculationEngine.MAX_REPEAT_LENGTH);
        acceptString(ReservedTagConstants.PROGRAM_GROUP_ID);
        acceptString(ReservedTagConstants.READ_GROUP_ID);
        acceptBytes(SAMRecord.NULL_SEQUENCE);
        acceptIndexType(IndexType.TABIX);
        acceptIndexBalance(IndexBalanceApproach.FOR_SEEK_TIME);
    }
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    assert!(
        !resolved.diagnostics.has_errors(),
        "{:?}",
        resolved.diagnostics
    );

    let unresolved_fields = resolved
        .facts
        .unresolved_target_facts
        .iter()
        .filter(|fact| fact.kind == UnresolvedTargetKind::FieldAccess)
        .collect::<Vec<_>>();
    assert!(
        unresolved_fields.is_empty(),
        "known external domain constants should not be field misses: {unresolved_fields:?}"
    );

    let calls = resolved.program.types[0]
        .members
        .iter()
        .find_map(|member| match member {
            Member::Method(method) if method.name == "run" => method.body.as_ref(),
            _ => None,
        })
        .unwrap()
        .statements
        .iter()
        .filter_map(|stmt| match stmt {
            Stmt::Expr(Expr::Call {
                resolved: Some(call),
                ..
            }) => Some(call.rust_name.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>();

    assert_eq!(
        calls,
        vec![
            "accept_string_string",
            "accept_string_string",
            "accept_string_string",
            "accept_string_string",
            "accept_char_char",
            "accept_int_int",
            "accept_int_int",
            "accept_double_double",
            "accept_bytes_byte_array",
            "accept_int_int",
            "accept_string_string",
            "accept_int_int",
            "accept_int_int",
            "accept_bytes_byte_array",
            "accept_string_string",
            "accept_int_int",
            "accept_int_int",
            "accept_string_string",
            "accept_string_string",
            "accept_string_string",
            "accept_read_format_reads_format_write_option",
            "accept_string_string",
            "accept_string_string",
            "accept_string_string",
            "accept_header_field_h_e_a_d_e_r_f_i_e_l_d_s",
            "accept_header_field_array_h_e_a_d_e_r_f_i_e_l_d_s_array",
            "accept_string_string",
            "accept_time_unit_time_unit",
            "accept_output_type_output_type",
            "accept_string_string",
            "accept_bytes_byte_array",
            "accept_int_int",
            "accept_string_string",
            "accept_string_string",
            "accept_int_int",
            "accept_int_int",
            "accept_int_int",
            "accept_option_option",
            "accept_option_option",
            "accept_header_line_type_supported_header_line_type",
            "accept_header_line_type_supported_header_line_type",
            "accept_string_string",
            "accept_enum_set_enum_set",
            "accept_string_string",
            "accept_string_string",
            "accept_settings_http_file_system_provider_settings",
            "accept_string_string",
            "accept_string_string",
            "accept_string_string",
            "accept_string_string",
            "accept_double_double",
            "accept_double_double",
            "accept_boolean_boolean",
            "accept_char_char",
            "accept_int_int",
            "accept_long_long",
            "accept_int_int",
            "accept_int_int",
            "accept_int_int",
            "accept_standard_open_option_standard_open_option",
            "accept_access_mode_access_mode",
            "accept_access_mode_access_mode",
            "accept_posix_file_permission_posix_file_permission",
            "accept_http_method_http_method",
            "accept_mapper_feature_mapper_feature",
            "accept_data_format_data_format",
            "accept_data_series_type_data_series_type",
            "accept_cloud_storage_configuration_cloud_storage_configuration",
            "accept_output_stream_output_stream",
            "accept_output_stream_output_stream",
            "accept_int_int",
            "accept_double_double",
            "accept_string_string",
            "accept_string_string",
            "accept_boolean_boolean",
            "accept_boolean_boolean",
            "accept_int_int",
            "accept_boolean_boolean",
            "accept_boolean_boolean",
            "accept_boolean_boolean",
            "accept_object_object",
            "accept_object_object",
            "accept_object_object",
            "accept_validation_stringency_validation_stringency",
            "accept_index_type_index_type",
            "accept_index_balance_index_balance_approach",
            "accept_option_option",
            "accept_option_option",
            "accept_header_line_type_supported_header_line_type",
            "accept_header_line_type_supported_header_line_type",
            "accept_int_int",
            "accept_file_cardinality_file_cardinality_write_option",
            "accept_file_cardinality_file_cardinality_write_option",
            "accept_genotypes_context_genotypes_context",
            "accept_long_long",
            "accept_string_string",
            "accept_string_string",
            "accept_string_string",
            "accept_writer_writer",
            "accept_deserialization_feature_deserialization_feature",
            "accept_jexl_missing_value_treatment_jexl_missing_value_treatment",
            "accept_jexl_missing_value_treatment_jexl_missing_value_treatment",
            "accept_string_string",
            "accept_char_char",
            "accept_int_int",
            "accept_level_level",
            "accept_level_level",
            "accept_level_level",
            "accept_locale_locale",
            "accept_string_string",
            "accept_string_string",
            "accept_element_kind_element_kind",
            "accept_export_configuration_export_configuration",
            "accept_int_int",
            "accept_string_string",
            "accept_string_string",
            "accept_bytes_byte_array",
            "accept_index_type_index_type",
            "accept_index_balance_index_balance_approach",
        ]
    );
}

#[test]
fn resolver_uses_external_enum_constants_for_collection_and_alignment_overloads() {
    let parsed = parse_java_source(
        r#"
class ExternalEnumConstantArgumentResolution {
    void acceptOption(Options option) {}
    void acceptOption(String option) {}

    void acceptStrategy(SWOverhangStrategy strategy) {}
    void acceptStrategy(String strategy) {}

    void run() {
        acceptOption(Options.INDEX_ON_THE_FLY);
        acceptOption(Options.DO_NOT_WRITE_GENOTYPES);
        acceptStrategy(SWOverhangStrategy.INDEL);
        acceptStrategy(SWOverhangStrategy.SOFTCLIP);
    }
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    assert!(
        !resolved.diagnostics.has_errors(),
        "{:?}",
        resolved.diagnostics
    );

    let unresolved_fields = resolved
        .facts
        .unresolved_target_facts
        .iter()
        .filter(|fact| fact.kind == UnresolvedTargetKind::FieldAccess)
        .collect::<Vec<_>>();
    assert!(
        unresolved_fields.is_empty(),
        "known external enum constants should not be field misses: {unresolved_fields:?}"
    );

    let calls = resolved.program.types[0]
        .members
        .iter()
        .find_map(|member| match member {
            Member::Method(method) if method.name == "run" => method.body.as_ref(),
            _ => None,
        })
        .unwrap()
        .statements
        .iter()
        .filter_map(|stmt| match stmt {
            Stmt::Expr(Expr::Call {
                resolved: Some(call),
                ..
            }) => Some(call.rust_name.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>();

    assert_eq!(
        calls,
        vec![
            "accept_option_options",
            "accept_option_options",
            "accept_strategy_s_w_overhang_strategy",
            "accept_strategy_s_w_overhang_strategy",
        ]
    );
}

#[test]
fn resolver_prefers_external_type_qualifier_over_same_snake_case_local_for_static_constants() {
    let parsed = parse_java_source(
        r#"
class ExternalTypeQualifierLocalCollisionResolution {
    void acceptOption(Options option) {}
    void acceptOption(EnumSet option) {}
    void acceptOption(String option) {}

    void run(EnumSet<Options> options) {
        options.add(Options.INDEX_ON_THE_FLY);
        acceptOption(Options.INDEX_ON_THE_FLY);
    }
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    assert!(
        !resolved.diagnostics.has_errors(),
        "{:?}",
        resolved.diagnostics
    );

    let unresolved_fields = resolved
        .facts
        .unresolved_target_facts
        .iter()
        .filter(|fact| fact.kind == UnresolvedTargetKind::FieldAccess)
        .collect::<Vec<_>>();
    assert!(
        unresolved_fields.is_empty(),
        "capitalized type qualifier should not be resolved through same snake-case local: {unresolved_fields:?}"
    );

    let calls = resolved.program.types[0]
        .members
        .iter()
        .find_map(|member| match member {
            Member::Method(method) if method.name == "run" => method.body.as_ref(),
            _ => None,
        })
        .unwrap()
        .statements
        .iter()
        .filter_map(|stmt| match stmt {
            Stmt::Expr(Expr::Call {
                resolved: Some(call),
                ..
            }) => Some(call.rust_name.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>();

    assert_eq!(calls, vec!["accept_option_options"]);
}

#[test]
fn resolver_uses_known_external_public_data_fields_for_overload_arguments() {
    let parsed = parse_java_source(
        r#"
class ExternalPublicDataFieldArgumentResolution {
    void acceptName(String name) {}
    void acceptName(int name) {}

    void acceptTag(String tag) {}
    void acceptTag(int tag) {}

    void acceptValue(Object value) {}
    void acceptValue(String value) {}

    void acceptBatch(int batch) {}
    void acceptBatch(String batch) {}

    void acceptBytes(byte[] value) {}
    void acceptBytes(String value) {}

    void acceptAvx(AVXLevel value) {}
    void acceptAvx(String value) {}

    void acceptOpenMp(OpenMPSetting value) {}
    void acceptOpenMp(String value) {}

    void acceptCallState(CallState value) {}
    void acceptCallState(String value) {}

    void acceptTruthState(TruthState value) {}
    void acceptTruthState(String value) {}

    void acceptThreads(int value) {}
    void acceptThreads(String value) {}

    void acceptBoolean(boolean value) {}
    void acceptBoolean(String value) {}

    void acceptCigarText(String value) {}
    void acceptCigarText(Cigar value) {}

    void run(
        JexlVCMatchExp expression,
        SAMTagAndValue tagAndValue,
        SAMBinaryTagAndValue binaryTagAndValue,
        BatchCompletionCallbackFunctionArgument batch,
        ReadDataHolder readData,
        HaplotypeDataHolder haplotypeData,
        PDHMMNativeArguments nativeArgs,
        PairHMMNativeArguments pairArgs,
        GenotypeLikelihoodsAllelePair allelePair,
        SWNativeAlignerResult nativeAlignment,
        TruthAndCallStates states
    ) {
        acceptName(expression.name);
        acceptTag(tagAndValue.tag);
        acceptValue(tagAndValue.value);
        acceptTag(binaryTagAndValue.tag);
        acceptValue(binaryTagAndValue.value);
        acceptBatch(batch.batchCount);
        acceptBatch(batch.totalBatchCount);
        acceptBytes(readData.readBases);
        acceptBytes(readData.readQuals);
        acceptBytes(readData.insertionGOP);
        acceptBytes(readData.deletionGOP);
        acceptBytes(readData.overallGCP);
        acceptBytes(haplotypeData.haplotypeBases);
        acceptBytes(haplotypeData.haplotypePDBases);
        acceptAvx(nativeArgs.avxLevel);
        acceptOpenMp(nativeArgs.openMPSetting);
        acceptThreads(nativeArgs.maxNumberOfThreads);
        acceptThreads(pairArgs.maxNumberOfThreads);
        acceptBoolean(pairArgs.useDoublePrecision);
        acceptThreads(allelePair.alleleIndex1);
        acceptThreads(allelePair.alleleIndex2);
        acceptThreads(nativeAlignment.alignment_offset);
        acceptCigarText(nativeAlignment.cigar);
        acceptCallState(states.callState);
        acceptTruthState(states.truthState);
    }
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    assert!(
        !resolved.diagnostics.has_errors(),
        "{:?}",
        resolved.diagnostics
    );

    let unresolved_fields = resolved
        .facts
        .unresolved_target_facts
        .iter()
        .filter(|fact| fact.kind == UnresolvedTargetKind::FieldAccess)
        .collect::<Vec<_>>();
    assert!(
        unresolved_fields.is_empty(),
        "known external public data fields should not be field misses: {unresolved_fields:?}"
    );

    let calls = resolved.program.types[0]
        .members
        .iter()
        .find_map(|member| match member {
            Member::Method(method) if method.name == "run" => method.body.as_ref(),
            _ => None,
        })
        .unwrap()
        .statements
        .iter()
        .filter_map(|stmt| match stmt {
            Stmt::Expr(Expr::Call {
                resolved: Some(call),
                ..
            }) => Some(call.rust_name.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>();

    assert_eq!(
        calls,
        vec![
            "accept_name_string",
            "accept_tag_string",
            "accept_value_object",
            "accept_tag_string",
            "accept_value_object",
            "accept_batch_int",
            "accept_batch_int",
            "accept_bytes_byte_array",
            "accept_bytes_byte_array",
            "accept_bytes_byte_array",
            "accept_bytes_byte_array",
            "accept_bytes_byte_array",
            "accept_bytes_byte_array",
            "accept_bytes_byte_array",
            "accept_avx_a_v_x_level",
            "accept_open_mp_open_m_p_setting",
            "accept_threads_int",
            "accept_threads_int",
            "accept_boolean_boolean",
            "accept_threads_int",
            "accept_threads_int",
            "accept_threads_int",
            "accept_cigar_text_string",
            "accept_call_state_call_state",
            "accept_truth_state_truth_state",
        ]
    );
}

#[test]
fn resolver_treats_imported_external_nested_types_as_type_qualifiers() {
    let parsed = parse_java_source(
        r#"
class ImportedExternalNestedTypeQualifierResolution {
    void run() {
        Coordinates.GenomicsDBColumn.newBuilder();
        Coordinates.ContigPosition.newBuilder();
        Storage.SignUrlOption.httpMethod();
        GenomicsDBImportConfiguration.ImportConfiguration.newBuilder();
        GenomicsDBImportConfiguration.Partition.newBuilder();
        GenomicsDBVidMapProto.VidMappingPB.newBuilder();
        GenomicsDBVidMapProto.FieldLengthDescriptorComponentPB.newBuilder();
        ReadSession.TableReadOptions.newBuilder();
    }
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    assert!(
        !resolved.diagnostics.has_errors(),
        "{:?}",
        resolved.diagnostics
    );

    let unresolved_fields = resolved
        .facts
        .unresolved_target_facts
        .iter()
        .filter(|fact| fact.kind == UnresolvedTargetKind::FieldAccess)
        .collect::<Vec<_>>();
    assert!(
        unresolved_fields.is_empty(),
        "known imported external nested type qualifiers should not be field misses: {unresolved_fields:?}"
    );

    let method_facts = resolved
        .facts
        .unresolved_target_facts
        .iter()
        .filter(|fact| fact.kind == UnresolvedTargetKind::MethodCall)
        .collect::<Vec<_>>();
    assert_eq!(method_facts.len(), 8);
    assert_eq!(
        method_facts
            .iter()
            .map(|fact| fact.target_name.as_str())
            .collect::<Vec<_>>(),
        vec![
            "newBuilder",
            "newBuilder",
            "httpMethod",
            "newBuilder",
            "newBuilder",
            "newBuilder",
            "newBuilder",
            "newBuilder",
        ]
    );
}

#[test]
fn resolver_uses_external_nested_pair_orientation_enum_constants() {
    let parsed = parse_java_source(
        r#"
class ExternalNestedPairOrientationResolution {
    void accept(PairOrientation orientation) {}
    void accept(String orientation) {}

    void run() {
        accept(SamPairUtil.PairOrientation.FR);
        accept(SamPairUtil.PairOrientation.RF);
        accept(SamPairUtil.PairOrientation.TANDEM);
    }
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    assert!(
        !resolved.diagnostics.has_errors(),
        "{:?}",
        resolved.diagnostics
    );

    let unresolved_fields = resolved
        .facts
        .unresolved_target_facts
        .iter()
        .filter(|fact| fact.kind == UnresolvedTargetKind::FieldAccess)
        .collect::<Vec<_>>();
    assert!(
        unresolved_fields.is_empty(),
        "known nested pair-orientation enum constants should not be field misses: {unresolved_fields:?}"
    );

    let calls = resolved.program.types[0]
        .members
        .iter()
        .find_map(|member| match member {
            Member::Method(method) if method.name == "run" => method.body.as_ref(),
            _ => None,
        })
        .unwrap()
        .statements
        .iter()
        .filter_map(|stmt| match stmt {
            Stmt::Expr(Expr::Call {
                resolved: Some(call),
                ..
            }) => Some(call.rust_name.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>();

    assert_eq!(
        calls,
        vec![
            "accept_pair_orientation",
            "accept_pair_orientation",
            "accept_pair_orientation",
        ]
    );
}

#[test]
fn resolver_uses_known_static_integer_parse_result_type_for_overload_arguments() {
    let parsed = parse_java_source(
        r#"
class StaticIntegerParserArgumentResolution {
    void accept(int value, String label) {}
    void accept(String value, String label) {}
    void run(String text) {
        accept(Integer.parseInt(text), "count");
    }
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    assert!(
        !resolved.diagnostics.has_errors(),
        "{:?}",
        resolved.diagnostics
    );

    let Stmt::Expr(Expr::Call {
        resolved: Some(call),
        ..
    }) = first_stmt(&resolved.program.types[0]).unwrap()
    else {
        panic!("expected resolved Integer.parseInt overload call");
    };

    assert_eq!(call.declaring_type, "StaticIntegerParserArgumentResolution");
    assert_eq!(call.rust_name, "accept_int_string");
}

#[test]
fn resolver_uses_system_get_property_result_type_for_constructor_overloads() {
    let parsed = parse_java_source(
        r#"
class PathHolder {
    PathHolder(String path) {}
    PathHolder(int path) {}
}

class SystemPropertyConstructorResolution {
    PathHolder make() {
        return new PathHolder(System.getProperty("java.io.tmpdir"));
    }
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    assert!(
        !resolved.diagnostics.has_errors(),
        "{:?}",
        resolved.diagnostics
    );

    let Expr::New {
        resolved: Some(constructor),
        ..
    } = first_return_expr(&resolved.program.types[1]).unwrap()
    else {
        panic!("expected resolved constructor from System.getProperty result");
    };

    assert_eq!(constructor.declaring_type, "PathHolder");
    assert_eq!(constructor.rust_name, "new_string");
}

#[test]
fn resolver_uses_external_to_path_result_type_for_overload_arguments() {
    let parsed = parse_java_source(
        r#"
class ExternalToPathArgumentResolution {
    void open(Path path, boolean create) {}
    void open(String path, boolean create) {}

    void run(File file, ExternalPathSource source) {
        open(file.toPath(), true);
        open(source.toPath(), false);
    }
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    assert!(
        !resolved.diagnostics.has_errors(),
        "{:?}",
        resolved.diagnostics
    );

    let ty = &resolved.program.types[0];
    let body = ty
        .members
        .iter()
        .find_map(|member| match member {
            Member::Method(method) if method.name == "run" => method.body.as_ref(),
            _ => None,
        })
        .unwrap();

    for stmt in &body.statements {
        let Stmt::Expr(Expr::Call {
            resolved: Some(call),
            ..
        }) = stmt
        else {
            panic!("expected resolved toPath overload call");
        };

        assert_eq!(call.declaring_type, "ExternalToPathArgumentResolution");
        assert_eq!(call.rust_name, "open_path_boolean");
    }
}

#[test]
fn resolver_uses_external_path_to_file_result_type_for_constructor_overloads() {
    let parsed = parse_java_source(
        r#"
class File {}
class Path {}

class OutputFailure {
    OutputFailure(File file, String message) {}
    OutputFailure(String file, String message) {}
}

class ExternalPathToFileConstructorResolution {
    OutputFailure make(Path path) {
        return new OutputFailure(path.toFile(), "open failed");
    }
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    assert!(
        !resolved.diagnostics.has_errors(),
        "{:?}",
        resolved.diagnostics
    );

    let ty = resolved
        .program
        .types
        .iter()
        .find(|ty| ty.name == "ExternalPathToFileConstructorResolution")
        .unwrap();
    let Stmt::Return(Some(Expr::New {
        resolved: Some(ctor),
        ..
    })) = first_stmt(ty).unwrap()
    else {
        panic!("expected resolved constructor from Path.toFile result");
    };

    assert_eq!(ctor.declaring_type, "OutputFailure");
    assert_eq!(ctor.rust_name, "new_file_string");
}

#[test]
fn resolver_uses_string_split_result_type_for_constructor_overloads() {
    let parsed = parse_java_source(
        r#"
class SplitRecord {
    SplitRecord(String[] fields, String label) {}
    SplitRecord(String fields, String label) {}
}

class StringSplitArgumentResolution {
    SplitRecord make(String line) {
        return new SplitRecord(line.split(","), "csv");
    }
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    assert!(
        !resolved.diagnostics.has_errors(),
        "{:?}",
        resolved.diagnostics
    );

    let Expr::New {
        resolved: Some(constructor),
        ..
    } = first_return_expr(&resolved.program.types[1]).unwrap()
    else {
        panic!("expected resolved constructor from String.split result");
    };

    assert_eq!(constructor.declaring_type, "SplitRecord");
    assert_eq!(constructor.rust_name, "new_string_array_string");
}

#[test]
fn resolver_uses_generic_pair_accessor_result_types_for_overload_arguments() {
    let parsed = parse_java_source(
        r#"
class GenericPairAccessorArgumentResolution {
    void set(int index, String heading, String value) {}
    void set(int index, int heading, String value) {}

    void run(Pair<String, Integer> pair) {
        set(0, pair.getLeft(), "value");
    }
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    assert!(
        !resolved.diagnostics.has_errors(),
        "{:?}",
        resolved.diagnostics
    );

    let Stmt::Expr(Expr::Call {
        resolved: Some(call),
        ..
    }) = first_stmt(&resolved.program.types[0]).unwrap()
    else {
        panic!("expected resolved generic pair accessor overload call");
    };

    assert_eq!(call.declaring_type, "GenericPairAccessorArgumentResolution");
    assert_eq!(call.rust_name, "set_int_string_string");
}

#[test]
fn resolver_uses_generic_map_value_type_before_chained_external_accessors() {
    let parsed = parse_java_source(
        r#"
class GenericMapValueChainArgumentResolution {
    void set(int index, String heading, String value) {}
    void set(int index, int heading, String value) {}

    void run(Map<String, Pair<String, Integer>> values) {
        set(0, values.get("key").getLeft(), "value");
    }
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    assert!(
        !resolved.diagnostics.has_errors(),
        "{:?}",
        resolved.diagnostics
    );

    let Stmt::Expr(Expr::Call {
        resolved: Some(call),
        ..
    }) = first_stmt(&resolved.program.types[0]).unwrap()
    else {
        panic!("expected resolved chained map value overload call");
    };

    assert_eq!(
        call.declaring_type,
        "GenericMapValueChainArgumentResolution"
    );
    assert_eq!(call.rust_name, "set_int_string_string");
}

#[test]
fn resolver_uses_generic_map_value_type_before_to_string_chain() {
    let parsed = parse_java_source(
        r#"
class GenericMapValueToStringArgumentResolution {
    void setColumn(int index, String value) {}
    void setColumn(int index, int value) {}

    void run(Map<String, ExternalStatus> statuses) {
        setColumn(0, statuses.get("key").toString());
    }
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    assert!(
        !resolved.diagnostics.has_errors(),
        "{:?}",
        resolved.diagnostics
    );

    let Stmt::Expr(Expr::Call {
        resolved: Some(call),
        ..
    }) = first_stmt(&resolved.program.types[0]).unwrap()
    else {
        panic!("expected resolved chained map toString overload call");
    };

    assert_eq!(
        call.declaring_type,
        "GenericMapValueToStringArgumentResolution"
    );
    assert_eq!(call.rust_name, "set_column_int_string");
}

#[test]
fn resolver_uses_generic_map_get_or_default_value_type_for_overload_arguments() {
    let parsed = parse_java_source(
        r#"
class GenericMapGetOrDefaultArgumentResolution {
    void add(String name, Entry value) {}
    void add(int index, Entry value) {}

    void run(Map<String, String> aliases, Entry entry) {
        add(aliases.getOrDefault("primary", "fallback"), entry);
    }
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    assert!(
        !resolved.diagnostics.has_errors(),
        "{:?}",
        resolved.diagnostics
    );

    let Stmt::Expr(Expr::Call {
        resolved: Some(call),
        ..
    }) = first_stmt(&resolved.program.types[0]).unwrap()
    else {
        panic!("expected resolved map getOrDefault overload call");
    };

    assert_eq!(
        call.declaring_type,
        "GenericMapGetOrDefaultArgumentResolution"
    );
    assert_eq!(call.rust_name, "add_string_entry");
}

#[test]
fn resolver_uses_map_entry_value_type_for_constructor_overloads() {
    let parsed = parse_java_source(
        r#"
class Report {
    Report(File file) {}
    Report(String path, String label) {}
}

class MapEntryValueConstructorResolution {
    Report make(Map.Entry<String, File> entry) {
        return new Report(entry.getValue());
    }
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    assert!(
        !resolved.diagnostics.has_errors(),
        "{:?}",
        resolved.diagnostics
    );

    let Expr::New {
        resolved: Some(constructor),
        ..
    } = first_return_expr(&resolved.program.types[1]).unwrap()
    else {
        panic!("expected resolved constructor from map entry value result");
    };

    assert_eq!(constructor.declaring_type, "Report");
    assert_eq!(constructor.rust_name, "new_file");
}

#[test]
fn resolver_uses_enhanced_for_map_entry_value_type_for_constructor_overloads() {
    let parsed = parse_java_source(
        r#"
class Report {
    Report(File file) {}
    Report(String path, String label) {}
}

class EnhancedForMapEntryValueConstructorResolution {
    void put(String key, Report report) {}

    void run(Map<String, File> reportFiles) {
        for (final Map.Entry<String,File> entry : reportFiles.entrySet()) {
            put(entry.getKey(), new Report(entry.getValue()));
        }
    }
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    assert!(
        !resolved.diagnostics.has_errors(),
        "{:?}",
        resolved.diagnostics
    );

    let body = resolved.program.types[1]
        .members
        .iter()
        .find_map(|member| match member {
            Member::Method(method) if method.name == "run" => method.body.as_ref(),
            _ => None,
        })
        .unwrap();
    let Stmt::EnhancedFor { body, .. } = &body.statements[0] else {
        panic!("expected enhanced for");
    };
    let Stmt::Expr(Expr::Call { args, .. }) = &body.statements[0] else {
        panic!("expected put call");
    };
    let Expr::New {
        resolved: Some(constructor),
        ..
    } = &args[1]
    else {
        panic!("expected resolved constructor from enhanced-for map entry value");
    };

    assert_eq!(constructor.declaring_type, "Report");
    assert_eq!(constructor.rust_name, "new_file");
}

#[test]
fn resolver_uses_enhanced_for_map_entry_value_type_in_multi_arg_constructor_overload() {
    let parsed = parse_java_source(
        r#"
class Key {}
class Evidence {}

class EvidenceBatch {
    EvidenceBatch(Key key, Iterable<Evidence> values) {}
    EvidenceBatch(Key left, Key right) {}
}

class EnhancedForEntryValueMultiArgConstructorResolution {
    void add(EvidenceBatch batch) {}

    void run(Map<Key, List<Evidence>> values) {
        for (final Map.Entry<Key, List<Evidence>> entry : values.entrySet()) {
            add(new EvidenceBatch(entry.getKey(), entry.getValue()));
        }
    }
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    assert!(
        !resolved.diagnostics.has_errors(),
        "{:?}",
        resolved.diagnostics
    );

    let body = resolved.program.types[3]
        .members
        .iter()
        .find_map(|member| match member {
            Member::Method(method) if method.name == "run" => method.body.as_ref(),
            _ => None,
        })
        .unwrap();
    let Stmt::EnhancedFor { body, .. } = &body.statements[0] else {
        panic!("expected enhanced for");
    };
    let Stmt::Expr(Expr::Call { args, .. }) = &body.statements[0] else {
        panic!("expected add call");
    };
    let Expr::New {
        resolved: Some(constructor),
        ..
    } = &args[0]
    else {
        panic!("expected resolved constructor from map entry key/value");
    };

    assert_eq!(constructor.declaring_type, "EvidenceBatch");
    assert_eq!(constructor.rust_name, "new_key_iterable");
}

#[test]
fn resolver_uses_wrapped_enhanced_for_map_entry_value_type_in_chained_receiver_constructor() {
    let parsed = parse_java_source(
        r#"
class Key {}
class Evidence {}
class Product {}

class EvidenceBatch {
    EvidenceBatch(Key key, Iterable<Evidence> values) {}
    EvidenceBatch(Key left, Key right) {}

    Product build() {
        return new Product();
    }
}

class ProductList {
    void add(Product product) {}
}

class WrappedEnhancedForEntryValueConstructorResolution {
    void run(Map<Key, List<Evidence>> values, ProductList products) {
        for (final Map.Entry<Key, List<Evidence>> entry :
                values.entrySet()) {
            products.add(new EvidenceBatch(entry.getKey(), entry.getValue()).build());
        }
    }
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    assert!(
        !resolved.diagnostics.has_errors(),
        "{:?}",
        resolved.diagnostics
    );

    let body = resolved.program.types[5]
        .members
        .iter()
        .find_map(|member| match member {
            Member::Method(method) if method.name == "run" => method.body.as_ref(),
            _ => None,
        })
        .unwrap();
    let Stmt::EnhancedFor { body, .. } = &body.statements[0] else {
        panic!("expected enhanced for");
    };
    let Stmt::Expr(Expr::Call { args, .. }) = &body.statements[0] else {
        panic!("expected add call");
    };
    let Expr::Call {
        receiver: Some(receiver),
        ..
    } = &args[0]
    else {
        panic!("expected chained build call");
    };
    let Expr::New {
        resolved: Some(constructor),
        ..
    } = receiver.as_ref()
    else {
        panic!("expected resolved constructor in chained receiver");
    };

    assert_eq!(constructor.declaring_type, "EvidenceBatch");
    assert_eq!(constructor.rust_name, "new_key_iterable");
}

#[test]
fn resolver_uses_external_builder_make_product_type_for_overload_arguments() {
    let parsed = parse_java_source(
        r#"
class ExternalBuilderMakeArgumentResolution {
    void consume(Widget widget, boolean trimLeft, boolean trimRight) {}
    void consume(String widget, boolean trimLeft, boolean trimRight) {}

    void run(WidgetBuilder builder) {
        consume(builder.make(), false, true);
    }
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    assert!(
        !resolved.diagnostics.has_errors(),
        "{:?}",
        resolved.diagnostics
    );

    let Stmt::Expr(Expr::Call {
        resolved: Some(call),
        ..
    }) = first_stmt(&resolved.program.types[0]).unwrap()
    else {
        panic!("expected resolved external builder make overload call");
    };

    assert_eq!(call.declaring_type, "ExternalBuilderMakeArgumentResolution");
    assert_eq!(call.rust_name, "consume_widget_boolean_boolean");
}

#[test]
fn resolver_uses_external_builder_fluent_result_before_make_product_type() {
    let parsed = parse_java_source(
        r#"
class ExternalBuilderFluentMakeArgumentResolution {
    void consume(Widget widget, Options options) {}
    void consume(String widget, Options options) {}

    void run(WidgetBuilder builder, Part part, Options options) {
        consume(builder.withPart(part).make(), options);
    }
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    assert!(
        !resolved.diagnostics.has_errors(),
        "{:?}",
        resolved.diagnostics
    );

    let Stmt::Expr(Expr::Call {
        resolved: Some(call),
        ..
    }) = first_stmt(&resolved.program.types[0]).unwrap()
    else {
        panic!("expected resolved fluent builder make overload call");
    };

    assert_eq!(
        call.declaring_type,
        "ExternalBuilderFluentMakeArgumentResolution"
    );
    assert_eq!(call.rust_name, "consume_widget_options");
}

#[test]
fn resolver_uses_boxing_reference_conversion_for_object_table_setters() {
    let parsed = parse_java_source(
        r#"
class ObjectTableSetterArgumentResolution {
    void set(Object rowId, String columnName, Object value) {}
    void set(int rowIndex, int columnIndex, String value) {}

    void run(int row, byte code) {
        set(row, "code", code);
    }
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    assert!(
        !resolved.diagnostics.has_errors(),
        "{:?}",
        resolved.diagnostics
    );

    let Stmt::Expr(Expr::Call {
        resolved: Some(call),
        ..
    }) = first_stmt(&resolved.program.types[0]).unwrap()
    else {
        panic!("expected resolved Object table setter overload");
    };

    assert_eq!(call.declaring_type, "ObjectTableSetterArgumentResolution");
    assert_eq!(call.rust_name, "set_object_string_object");
}

#[test]
fn resolver_uses_tuple_field_type_for_map_arguments() {
    let parsed = parse_java_source(
        r#"
class TupleFieldArgumentResolution {
    void put(String key, int value) {}
    void put(int key, String value) {}

    void run(Tuple2<String, Integer> entry) {
        put(entry._1, entry._2);
    }
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    assert!(
        !resolved.diagnostics.has_errors(),
        "{:?}",
        resolved.diagnostics
    );

    let Stmt::Expr(Expr::Call {
        resolved: Some(call),
        ..
    }) = first_stmt(&resolved.program.types[0]).unwrap()
    else {
        panic!("expected resolved tuple-field overload");
    };

    assert_eq!(call.declaring_type, "TupleFieldArgumentResolution");
    assert_eq!(call.rust_name, "put_string_int");
}

#[test]
fn resolver_uses_named_tuple_field_type_for_overload_arguments() {
    let parsed = parse_java_source(
        r#"
class NamedTupleFieldArgumentResolution {
    void setAttribute(String key, String value) {}
    void setAttribute(String key, Integer value) {}
    void setAttribute(String key, Object value) {}

    void run(Tuple<Integer, String> entry, ImmutablePair<String, Integer> pair) {
        setAttribute("MC", entry.b);
        setAttribute("LEFT", pair.left);
        setAttribute("RIGHT", pair.right);
    }
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    assert!(
        !resolved.diagnostics.has_errors(),
        "{:?}",
        resolved.diagnostics
    );

    let calls = resolved.program.types[0]
        .members
        .iter()
        .find_map(|member| match member {
            Member::Method(method) if method.name == "run" => method.body.as_ref(),
            _ => None,
        })
        .unwrap()
        .statements
        .iter()
        .filter_map(|stmt| match stmt {
            Stmt::Expr(Expr::Call {
                resolved: Some(call),
                ..
            }) => Some(call.rust_name.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>();

    assert_eq!(
        calls,
        vec![
            "set_attribute_string_string",
            "set_attribute_string_string",
            "set_attribute_string_integer",
        ]
    );
}

#[test]
fn resolver_uses_local_tuple_field_type_for_chained_field_access() {
    let parsed = parse_java_source(
        r#"
class Evidence {
    int count;
}

class LocalTupleFieldChainResolution {
    void accept(int value) {}
    void accept(String value) {}

    void run(Tuple2<String, Evidence> next) {
        Evidence evidence = next._2;
        accept(evidence.count);
    }
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    assert!(
        !resolved.diagnostics.has_errors(),
        "{:?}",
        resolved.diagnostics
    );

    let unresolved_fields = resolved
        .facts
        .unresolved_target_facts
        .iter()
        .filter(|fact| fact.kind == UnresolvedTargetKind::FieldAccess)
        .collect::<Vec<_>>();
    assert!(
        unresolved_fields.is_empty(),
        "tuple field values assigned to locals should preserve source field types: {unresolved_fields:?}"
    );

    let ty = &resolved.program.types[1];
    let body = ty
        .members
        .iter()
        .find_map(|member| match member {
            Member::Method(method) if method.name == "run" => method.body.as_ref(),
            _ => None,
        })
        .unwrap();
    let Stmt::Expr(Expr::Call {
        resolved: Some(call),
        ..
    }) = &body.statements[1]
    else {
        panic!("expected resolved overload from local tuple field");
    };

    assert_eq!(call.rust_name, "accept_int");
}

#[test]
fn resolver_uses_uppercase_source_instance_field_type_for_overload_arguments() {
    let parsed = parse_java_source(
        r#"
class UppercaseSourceFieldArgumentResolution {
    void accept(double value) {}
    void accept(String value) {}

    private static final class Result {
        private final double C;

        Result(double value) {
            this.C = value;
        }
    }

    private static Result calculate() {
        return new Result(1.0);
    }

    void run() {
        accept(calculate().C);
    }
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    assert!(
        !resolved.diagnostics.has_errors(),
        "{:?}",
        resolved.diagnostics
    );

    let unresolved_fields = resolved
        .facts
        .unresolved_target_facts
        .iter()
        .filter(|fact| fact.kind == UnresolvedTargetKind::FieldAccess)
        .collect::<Vec<_>>();
    assert!(
        unresolved_fields.is_empty(),
        "uppercase source instance fields should resolve before fallback facts: {unresolved_fields:?}"
    );

    let Stmt::Expr(Expr::Call {
        resolved: Some(call),
        ..
    }) = resolved.program.types[0]
        .members
        .iter()
        .find_map(|member| match member {
            Member::Method(method) if method.name == "run" => method.body.as_ref(),
            _ => None,
        })
        .and_then(|body| body.statements.first())
        .unwrap()
    else {
        panic!("expected resolved overload from uppercase source field");
    };

    assert_eq!(call.rust_name, "accept_double");
}

#[test]
fn resolver_uses_iterator_next_element_type_for_field_access_arguments() {
    let parsed = parse_java_source(
        r#"
class Item {
    int value;
}

class IteratorNextElementFieldArgumentResolution {
    void accept(int value) {}
    void accept(String value) {}

    void run(Iterator<Item> items) {
        accept(items.next().value);
    }
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    assert!(
        !resolved.diagnostics.has_errors(),
        "{:?}",
        resolved.diagnostics
    );

    let unresolved_fields = resolved
        .facts
        .unresolved_target_facts
        .iter()
        .filter(|fact| fact.kind == UnresolvedTargetKind::FieldAccess)
        .collect::<Vec<_>>();
    assert!(
        unresolved_fields.is_empty(),
        "iterator element fields should resolve through the generic next() result: {unresolved_fields:?}"
    );

    let Stmt::Expr(Expr::Call {
        resolved: Some(call),
        ..
    }) = first_stmt(&resolved.program.types[1]).unwrap()
    else {
        panic!("expected resolved overload from iterator element field");
    };

    assert_eq!(call.rust_name, "accept_int");
}

#[test]
fn resolver_uses_queue_peek_element_type_for_field_access_arguments() {
    let parsed = parse_java_source(
        r#"
class Item {
    String label;
}

class QueuePeekElementFieldArgumentResolution {
    void accept(String value) {}
    void accept(int value) {}

    void run(Queue<Item> items) {
        accept(items.peek().label);
    }
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    assert!(
        !resolved.diagnostics.has_errors(),
        "{:?}",
        resolved.diagnostics
    );

    let unresolved_fields = resolved
        .facts
        .unresolved_target_facts
        .iter()
        .filter(|fact| fact.kind == UnresolvedTargetKind::FieldAccess)
        .collect::<Vec<_>>();
    assert!(
        unresolved_fields.is_empty(),
        "queue element fields should resolve through the generic peek() result: {unresolved_fields:?}"
    );

    let Stmt::Expr(Expr::Call {
        resolved: Some(call),
        ..
    }) = first_stmt(&resolved.program.types[1]).unwrap()
    else {
        panic!("expected resolved overload from queue element field");
    };

    assert_eq!(call.rust_name, "accept_string");
}

#[test]
fn resolver_uses_thread_local_get_value_type_for_field_access_arguments() {
    let parsed = parse_java_source(
        r#"
class Worker {
    boolean destroyed;
}

class ThreadLocalGetValueFieldArgumentResolution {
    void accept(boolean value) {}
    void accept(String value) {}

    void run(ThreadLocal<Worker> local) {
        accept(local.get().destroyed);
    }
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    assert!(
        !resolved.diagnostics.has_errors(),
        "{:?}",
        resolved.diagnostics
    );

    let unresolved_fields = resolved
        .facts
        .unresolved_target_facts
        .iter()
        .filter(|fact| fact.kind == UnresolvedTargetKind::FieldAccess)
        .collect::<Vec<_>>();
    assert!(
        unresolved_fields.is_empty(),
        "ThreadLocal<T>.get() should expose T for following field lookup: {unresolved_fields:?}"
    );

    let Stmt::Expr(Expr::Call {
        resolved: Some(call),
        ..
    }) = first_stmt(&resolved.program.types[1]).unwrap()
    else {
        panic!("expected resolved overload from thread-local value field");
    };

    assert_eq!(call.rust_name, "accept_boolean");
}

#[test]
fn resolver_keeps_raw_thread_local_get_field_access_conservative() {
    let parsed = parse_java_source(
        r#"
class RawThreadLocalGetFieldArgumentResolution {
    void accept(boolean value) {}

    void run(ThreadLocal local) {
        accept(local.get().destroyed);
    }
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    assert!(
        !resolved.diagnostics.has_errors(),
        "{:?}",
        resolved.diagnostics
    );

    let unresolved_fields = resolved
        .facts
        .unresolved_target_facts
        .iter()
        .filter(|fact| fact.kind == UnresolvedTargetKind::FieldAccess)
        .collect::<Vec<_>>();
    assert_eq!(
        unresolved_fields.len(),
        1,
        "raw ThreadLocal.get() should not fabricate a value shape"
    );
}

#[test]
fn resolver_keeps_raw_iterator_next_field_access_conservative() {
    let parsed = parse_java_source(
        r#"
class RawIteratorNextFieldArgumentResolution {
    void accept(int value) {}

    void run(Iterator items) {
        accept(items.next().value);
    }
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    assert!(
        !resolved.diagnostics.has_errors(),
        "{:?}",
        resolved.diagnostics
    );

    let unresolved_fields = resolved
        .facts
        .unresolved_target_facts
        .iter()
        .filter(|fact| fact.kind == UnresolvedTargetKind::FieldAccess)
        .collect::<Vec<_>>();
    assert_eq!(
        unresolved_fields.len(),
        1,
        "raw iterator element shape should not fabricate a field type"
    );
}

#[test]
fn resolver_uses_extends_wildcard_iterator_next_bound_for_chained_iterator() {
    let parsed = parse_java_source(
        r#"
class Item {
    int value;
}

class ExtendsWildcardIteratorChainResolution {
    void accept(int value) {}
    void accept(String value) {}

    void run(Iterator<? extends Iterable<Item>> groups) {
        accept(groups.next().iterator().next().value);
    }
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    assert!(
        !resolved.diagnostics.has_errors(),
        "{:?}",
        resolved.diagnostics
    );

    let unresolved_fields = resolved
        .facts
        .unresolved_target_facts
        .iter()
        .filter(|fact| fact.kind == UnresolvedTargetKind::FieldAccess)
        .collect::<Vec<_>>();
    assert!(
        unresolved_fields.is_empty(),
        "extends-wildcard iterator bounds should project to their readable upper bound: {unresolved_fields:?}"
    );

    let Stmt::Expr(Expr::Call {
        resolved: Some(call),
        ..
    }) = first_stmt(&resolved.program.types[1]).unwrap()
    else {
        panic!("expected resolved overload from chained wildcard iterator element");
    };

    assert_eq!(call.rust_name, "accept_int");
}

#[test]
fn resolver_uses_source_iterable_supertype_element_for_utils_stream_tuple_projection() {
    let parsed = parse_java_source(
        r#"
class Key {
}

class Evidence {
    int reads;
}

interface Iterable<T> {
}

class SourceTree<V> implements Iterable<Tuple2<Key, V>> {
}

class SourceIterableStreamTupleProjectionResolution {
    void accept(int value) {}
    void accept(String value) {}

    void run(SourceTree<Evidence> tree) {
        accept(Utils.stream(tree).mapToInt(p -> p._2.reads).sum());
    }
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    assert!(
        !resolved.diagnostics.has_errors(),
        "{:?}",
        resolved.diagnostics
    );

    let unresolved_fields = resolved
        .facts
        .unresolved_target_facts
        .iter()
        .filter(|fact| fact.kind == UnresolvedTargetKind::FieldAccess)
        .collect::<Vec<_>>();
    assert!(
        unresolved_fields.is_empty(),
        "source Iterable<T> supertypes should feed Utils.stream element types for tuple projection: {unresolved_fields:?}"
    );

    let ty = &resolved.program.types[4];
    let body = ty
        .members
        .iter()
        .find_map(|member| match member {
            Member::Method(method) if method.name == "run" => method.body.as_ref(),
            _ => None,
        })
        .unwrap();
    let Stmt::Expr(Expr::Call {
        resolved: Some(call),
        ..
    }) = &body.statements[0]
    else {
        panic!("expected resolved overload from source Iterable stream tuple projection");
    };

    assert_eq!(call.rust_name, "accept_int");
}

#[test]
fn resolver_substitutes_source_generic_method_parameter_from_source_iterable_supertype() {
    let parsed = parse_java_source(
        r#"
class Key {
}

class Evidence {
    int reads;
}

interface Iterable<T> {
}

class Stream<T> {
    IntStream mapToInt(ToIntFunction<T> mapper) {
        return null;
    }
}

interface ToIntFunction<T> {
    int applyAsInt(T value);
}

class IntStream {
    int sum() {
        return 0;
    }
}

class Utils {
    static <T> Stream<T> stream(Iterable<T> values) {
        return null;
    }
}

class SourceTree<V> implements Iterable<Tuple2<Key, V>> {
}

class SourceGenericStreamMethodTupleProjectionResolution {
    void accept(int value) {}
    void accept(String value) {}

    void run(SourceTree<Evidence> tree) {
        accept(Utils.stream(tree).mapToInt(p -> p._2.reads).sum());
    }
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    assert!(
        !resolved.diagnostics.has_errors(),
        "{:?}",
        resolved.diagnostics
    );

    let unresolved_fields = resolved
        .facts
        .unresolved_target_facts
        .iter()
        .filter(|fact| fact.kind == UnresolvedTargetKind::FieldAccess)
        .collect::<Vec<_>>();
    assert!(
        unresolved_fields.is_empty(),
        "source generic method parameters should bind from source Iterable<T> supertypes: {unresolved_fields:?}"
    );

    let ty = &resolved.program.types[8];
    let body = ty
        .members
        .iter()
        .find_map(|member| match member {
            Member::Method(method) if method.name == "run" => method.body.as_ref(),
            _ => None,
        })
        .unwrap();
    let Stmt::Expr(Expr::Call {
        resolved: Some(call),
        ..
    }) = &body.statements[0]
    else {
        panic!("expected resolved overload from source generic stream method projection");
    };

    assert_eq!(call.rust_name, "accept_int");
}

#[test]
fn resolver_uses_source_iterable_supertype_with_nested_generic_element_for_utils_stream() {
    let parsed = parse_java_source(
        r#"
class Payload {
    int reads;
}

class SourceIntervalTree<V> implements Iterable<SourceIntervalTree.Entry<V>> {
    interface Entry<V1> {
        V1 getValue();
    }
}

class SourceIterableNestedGenericElementResolution {
    void accept(int value) {}
    void accept(String value) {}

    void run(SourceIntervalTree<Tuple2<Boolean, Payload>> tree) {
        accept(Utils.stream(tree).mapToInt(e -> e.getValue()._2.reads).sum());
    }
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    assert!(
        !resolved.diagnostics.has_errors(),
        "{:?}",
        resolved.diagnostics
    );

    let unresolved_fields = resolved
        .facts
        .unresolved_target_facts
        .iter()
        .filter(|fact| fact.kind == UnresolvedTargetKind::FieldAccess)
        .collect::<Vec<_>>();
    assert!(
        unresolved_fields.is_empty(),
        "qualified nested generic Iterable elements should feed Utils.stream element types: {unresolved_fields:?}"
    );

    let ty = &resolved.program.types[2];
    let body = ty
        .members
        .iter()
        .find_map(|member| match member {
            Member::Method(method) if method.name == "run" => method.body.as_ref(),
            _ => None,
        })
        .unwrap();
    let Stmt::Expr(Expr::Call {
        resolved: Some(call),
        ..
    }) = &body.statements[0]
    else {
        panic!("expected resolved overload from nested generic Iterable stream projection");
    };

    assert_eq!(call.rust_name, "accept_int");
}

#[test]
fn resolver_substitutes_source_generic_receiver_return_type_for_constructor_arguments() {
    let parsed = parse_java_source(
        r#"
class Span {
}

class OrientedSpan {
    OrientedSpan(Span span, boolean forward) {}
    OrientedSpan(String span, String forward) {}
}

class Entry<T> {
    Span getInterval() {
        return new Span();
    }

    T getValue() {
        return null;
    }
}

class SourceGenericReceiverArgumentResolution {
    OrientedSpan make(Entry<Tuple2<Boolean, String>> entry) {
        return new OrientedSpan(entry.getInterval(), entry.getValue()._1());
    }
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    assert!(
        !resolved.diagnostics.has_errors(),
        "{:?}",
        resolved.diagnostics
    );

    let Expr::New {
        resolved: Some(constructor),
        ..
    } = first_return_expr(&resolved.program.types[3]).unwrap()
    else {
        panic!("expected resolved constructor from source generic receiver result");
    };

    assert_eq!(constructor.declaring_type, "OrientedSpan");
    assert_eq!(constructor.rust_name, "new_span_boolean");
}

#[test]
fn resolver_substitutes_source_generic_method_return_type_for_overload_arguments() {
    let parsed = parse_java_source(
        r#"
class SourceGenericMethodArgumentResolution {
    static <T> T require(T value, String message) {
        return value;
    }

    void acceptBytes(byte[] bases, String label) {}
    void acceptBytes(String bases, String label) {}

    void run(byte[] bases) {
        acceptBytes(require(bases, "bases"), "ok");
    }
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    assert!(
        !resolved.diagnostics.has_errors(),
        "{:?}",
        resolved.diagnostics
    );

    let ty = &resolved.program.types[0];
    let body = ty
        .members
        .iter()
        .find_map(|member| match member {
            Member::Method(method) if method.name == "run" => method.body.as_ref(),
            _ => None,
        })
        .unwrap();
    let Stmt::Expr(Expr::Call {
        resolved: Some(call),
        ..
    }) = &body.statements[0]
    else {
        panic!("expected resolved generic passthrough overload call");
    };

    assert_eq!(call.declaring_type, "SourceGenericMethodArgumentResolution");
    assert_eq!(call.rust_name, "accept_bytes_byte_array_string");
}

#[test]
fn resolver_matches_nested_method_type_variables_inside_generic_collection_parameters() {
    let parsed = parse_java_source(
        r#"
class FeatureInput<T> {}
class Record {}

class FeatureLookup {
    <T> List<T> getValues(FeatureInput<T> input) {
        return null;
    }

    <T> List<T> getValues(Collection<FeatureInput<T>> inputs) {
        return null;
    }

    List<Record> run(List<FeatureInput<Record>> inputs) {
        return getValues(inputs);
    }
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    assert!(
        !resolved.diagnostics.has_errors(),
        "{:?}",
        resolved.diagnostics
    );

    let ty = &resolved.program.types[2];
    let body = ty
        .members
        .iter()
        .find_map(|member| match member {
            Member::Method(method) if method.name == "run" => method.body.as_ref(),
            _ => None,
        })
        .unwrap();
    let Stmt::Return(Some(Expr::Call {
        resolved: Some(call),
        ..
    })) = &body.statements[0]
    else {
        panic!("expected resolved nested generic method call");
    };

    assert_eq!(call.declaring_type, "FeatureLookup");
    assert_eq!(call.rust_name, "get_values_collection");
    assert_eq!(
        call.return_type,
        JavaType::Named {
            name: "List".to_string(),
            args: vec![JavaType::Named {
                name: "Record".to_string(),
                args: Vec::new(),
            }],
        }
    );
}

#[test]
fn resolver_benchmark_reports_resolver_diagnostic_summaries() {
    let root = std::env::temp_dir().join(format!(
        "new_java2rust_resolver_benchmark_diagnostics_{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap();
    std::fs::write(
        root.join("AmbiguousCall.java"),
        r#"
class AmbiguousCall {
    void choose(Left value) {}
    void choose(Right value) {}

    void run(External value) {
        choose(value);
    }
}
"#,
    )
    .unwrap();
    std::fs::write(
        root.join("SecondAmbiguousCall.java"),
        r#"
class SecondAmbiguousCall {
    void choose(Left value) {}
    void choose(Right value) {}

    void run(External value) {
        choose(value);
    }
}
"#,
    )
    .unwrap();

    let report = benchmark_java_resolver_dir(&root).unwrap();
    let rendered = render_resolver_benchmark_report(&report);
    std::fs::remove_dir_all(&root).unwrap();

    assert_eq!(report.resolver_diagnostics, 2);
    assert!(rendered.contains("top_resolver_diagnostics"));
    assert!(rendered.contains("resolve_call_ambiguous"));
    assert!(rendered.contains("top_resolver_diagnostic_files"));
    assert!(rendered.contains("AmbiguousCall.java"));
    assert!(rendered.contains("SecondAmbiguousCall.java"));
}

fn first_return_expr(ty: &new_java2rust::java_ir::TypeDecl) -> Option<&Expr> {
    ty.members.iter().find_map(|member| match member {
        Member::Method(method) => method.body.as_ref().and_then(|body| {
            body.statements.iter().find_map(|stmt| match stmt {
                Stmt::Return(Some(expr)) => Some(expr),
                _ => None,
            })
        }),
        _ => None,
    })
}

fn first_stmt(ty: &new_java2rust::java_ir::TypeDecl) -> Option<&Stmt> {
    ty.members.iter().find_map(|member| match member {
        Member::Method(method) => method
            .body
            .as_ref()
            .and_then(|body| body.statements.first()),
        _ => None,
    })
}

fn collect_calls<'a>(expr: &'a Expr, out: &mut Vec<&'a Expr>) {
    match expr {
        Expr::Call { receiver, args, .. } => {
            out.push(expr);
            if let Some(receiver) = receiver {
                collect_calls(receiver, out);
            }
            for arg in args {
                collect_calls(arg, out);
            }
        }
        Expr::Binary { left, right, .. } => {
            collect_calls(left, out);
            collect_calls(right, out);
        }
        Expr::Ternary {
            condition,
            then_expr,
            else_expr,
        } => {
            collect_calls(condition, out);
            collect_calls(then_expr, out);
            collect_calls(else_expr, out);
        }
        Expr::FieldAccess(receiver, _) => collect_calls(receiver, out),
        Expr::ArrayAccess(array, index) => {
            collect_calls(array, out);
            collect_calls(index, out);
        }
        Expr::Unary { expr, .. } => collect_calls(expr, out),
        Expr::New { args, .. } => {
            for arg in args {
                collect_calls(arg, out);
            }
        }
        _ => {}
    }
}
