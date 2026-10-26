#[path = "common/mod.rs"]
mod common;

#[test]
fn enum_class_getter_contract_is_shared_with_separately_emitted_callers() {
    common::cargo_run_translated_batch_sources(
        "enum_class_getter_assembly",
        &[
            new_java2rust::BatchSource::new(
                "ClassTokenProbe.java",
                r#"
class ClassTokenProbe {}
enum ClassTokenHolder {
    ENTRY(ClassTokenProbe.class);
    final Class<ClassTokenProbe> token;
    ClassTokenHolder(Class<ClassTokenProbe> token) { this.token = token; }
    Class<ClassTokenProbe> token() { return token; }
}
"#,
            ),
            new_java2rust::BatchSource::new(
                "ClassTokenConsumer.java",
                "class ClassTokenConsumer { static int run() { return ClassTokenHolder.ENTRY.token() == ClassTokenProbe.class ? 0 : 1; } }",
            ),
        ],
        r#"
trait Exit { fn value(self) -> i32; }
impl Exit for i32 { fn value(self) -> i32 { self } }
impl<E: std::fmt::Debug> Exit for Result<i32,E> { fn value(self) -> i32 { self.unwrap() } }
fn main() { assert_eq!(translated::ClassTokenConsumer::run().value(), 0); }
"#,
    );
}

#[test]
fn package_assembly_uses_producer_owned_class_token_storage_and_reflection() {
    common::cargo_run_translated_batch_sources(
        "class_token_assembly",
        &[new_java2rust::BatchSource::new(
            "ClassTokenProbe.java",
            "class ClassTokenProbe { static Class<ClassTokenProbe> token() { return ClassTokenProbe.class; } }",
        )],
        r#"
use translated::JavaClass;
struct NoTraits;
type LocalInstance = std::rc::Rc<std::cell::RefCell<NoTraits>>;
static TOKEN: std::sync::OnceLock<JavaClass<LocalInstance>> = std::sync::OnceLock::new();
fn main() {
    let token = TOKEN.get_or_init(|| JavaClass::literal("ClassTokenProbe"));
    let copy = token.clone();
    assert!(format!("{:?}", copy).contains("ClassTokenProbe"));
    assert_eq!(std::thread::spawn(move || copy.get_name()).join().unwrap(), "ClassTokenProbe");
    let array = JavaClass::<()>::literal("int[]");
    assert_eq!(array.get_name(), "[I");
    assert!(array.is_array());
}
"#,
    );
}

#[test]
fn class_tokens_can_be_published_without_owning_the_named_instance_type() {
    let translation = new_java2rust::translate_source(
        r#"
class ClassTokenProbe { static Class<ClassTokenProbe> token() { return ClassTokenProbe.class; } }
enum ClassTokenHolder {
    ENTRY(ClassTokenProbe.class);
    final Class<ClassTokenProbe> token;
    ClassTokenHolder(Class<ClassTokenProbe> token) { this.token = token; }
    Class<ClassTokenProbe> token() { return token; }
}
"#,
    );
    assert!(
        !translation.diagnostics.has_errors(),
        "{:?}",
        translation.diagnostics
    );
    common::cargo_test_generated_modules(
        "class_token_non_owning",
        vec![common::GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: r#"
struct NoTraits;
type LocalInstance = std::rc::Rc<std::cell::RefCell<NoTraits>>;
static TOKEN: std::sync::OnceLock<JavaClass<LocalInstance>> = std::sync::OnceLock::new();
#[test] fn token_is_independent_of_instance_traits() {
    let token = TOKEN.get_or_init(|| JavaClass::literal("ClassTokenProbe"));
    let copy = token.clone();
    assert!(format!("{:?}", copy).contains("ClassTokenProbe"));
    let name = std::thread::spawn(move || copy.get_name()).join().unwrap();
    assert_eq!(name, "ClassTokenProbe");
    assert_eq!(token, &JavaClass::<NoTraits>::literal("ClassTokenProbe"));
    assert_eq!(ClassTokenHolder::ENTRY.token().unwrap().get_name(), "ClassTokenProbe");
}
"#,
        }],
    );
}

#[test]
fn enum_varargs_share_the_canonical_array_builder_and_keep_explicit_aliases() {
    common::verify_java_fixture_zero_contract(
        "enum_varargs_array",
        "EnumVarargsArrayProbe",
        "tests/fixtures/EnumVarargsArrayProbe.java",
        include_str!("fixtures/EnumVarargsArrayProbe.java"),
        r#"
trait Exit { fn value(self) -> i32; }
impl Exit for i32 { fn value(self) -> i32 { self } }
impl<E: std::fmt::Debug> Exit for Result<i32,E> { fn value(self) -> i32 { self.unwrap() } }
#[test] fn arrays() { assert_eq!(EnumVarargsArrayProbe::run().value(), 0); }
"#,
    );
}

#[test]
fn enum_constants_are_not_confused_with_self_typed_fields_or_static_aliases() {
    common::verify_java_fixture_zero_contract(
        "enum_declaration_roles",
        "EnumDeclarationProbe",
        "tests/corpus/numbers/EnumDeclarationProbe.java",
        include_str!("corpus/numbers/EnumDeclarationProbe.java"),
        r#"
trait Value { fn value(self) -> i32; }
impl Value for i32 { fn value(self) -> i32 { self } }
impl<E: std::fmt::Debug> Value for Result<i32, E> { fn value(self) -> i32 { self.unwrap() } }
#[test] fn roles() { assert_eq!(EnumDeclarationProbe::run().value(), 0); }
"#,
    );
}

#[test]
fn enum_payloads_preserve_construction_effects_alias_identity_and_mutable_state() {
    common::verify_java_fixture_zero_contract(
        "enum_state",
        "EnumStateProbe",
        "tests/corpus/numbers/EnumStateProbe.java",
        include_str!("corpus/numbers/EnumStateProbe.java"),
        r#"
trait Value { fn value(self) -> i32; }
impl Value for i32 { fn value(self) -> i32 { self } }
impl<E: std::fmt::Debug> Value for Result<i32, E> { fn value(self) -> i32 { self.unwrap() } }
#[test] fn state() { assert_eq!(EnumStateProbe::run().value(), 0); }
"#,
    );
}

#[test]
fn enum_constructors_initialize_once_delegate_and_publish_in_order() {
    common::verify_java_fixture_zero_contract(
        "enum_construction",
        "EnumConstructionProbe",
        "tests/corpus/numbers/EnumConstructionProbe.java",
        include_str!("corpus/numbers/EnumConstructionProbe.java"),
        r#"
trait Value { fn value(self) -> i32; }
impl Value for i32 { fn value(self) -> i32 { self } }
impl<E: std::fmt::Debug> Value for Result<i32, E> { fn value(self) -> i32 { self.unwrap() } }
#[test] fn construction() { assert_eq!(EnumConstructionProbe::run().value(), 0); }
"#,
    );
}

#[test]
fn source_sam_references_use_typed_overloads_and_retained_array_contracts() {
    common::verify_java_fixture_zero_contract(
        "source_sam_references",
        "SourceSamReferenceProbe",
        "tests/corpus/numbers/SourceSamReferenceProbe.java",
        include_str!("corpus/numbers/SourceSamReferenceProbe.java"),
        r#"
trait Value { fn value(self) -> i32; }
impl Value for i32 { fn value(self) -> i32 { self } }
impl<E: std::fmt::Debug> Value for Result<i32, E> { fn value(self) -> i32 { self.unwrap() } }
#[test] fn references() { assert_eq!(SourceSamReferenceProbe::run().value(), 0); }
"#,
    );
}
