use new_java2rust::translate_source;

#[path = "common/mod.rs"]
mod common;
use common::{GeneratedCrateModule, cargo_check_generated_modules, cargo_test_generated_modules};

#[test]
fn source_boxed_primitive_name_keeps_its_declared_value_method() {
    let translation = translate_source(
        r#"
class Character {
    char charValue() { return 'x'; }
    static char valueOf(char ignored) { return 's'; }
}
class CharacterUse {
    static char run() { return new Character().charValue(); }
    static char sourceStaticRun() { return Character.valueOf('z'); }
    static char nativeRun() {
        java.lang.Character boxed = java.lang.Character.valueOf('q');
        return boxed.charValue();
    }
}
"#,
    );
    cargo_test_generated_modules(
        "source_character_value_method",
        vec![GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: "#[test] fn source_method_not_native_unboxing() { assert_eq!(CharacterUse::run(), 'x'); assert_eq!(CharacterUse::source_static_run(), 's'); assert_eq!(CharacterUse::native_run(), 'q'); }",
        }],
    );
}

#[test]
fn source_interface_extending_platform_marker_keeps_its_trait_abi() {
    let translation = translate_source(
        r#"
package neutral.storage;
import java.io.Serializable;
interface StoragePort<Value, Item> extends Serializable { int size(); }
class SmallStorage implements StoragePort<String, String> {
    public int size() { return 9; }
}
class StorageUse {
    static int probe() {
        StoragePort<String, String> port = new SmallStorage();
        return port.size();
    }
}
"#,
    );
    assert!(
        translation.rust.contains("pub trait StoragePort<"),
        "{}",
        translation.rust
    );
    assert!(!translation.rust.contains("pub struct StoragePort<"));
    cargo_test_generated_modules(
        "source_marker_interface_abi",
        vec![GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: r#"
#[test]
fn marker_interface_does_not_replace_source_dispatch() {
    assert_eq!(StorageUse::probe(), 9);
}
"#,
        }],
    );
}

#[test]
fn generic_interface_return_keeps_its_instantiation_in_the_trait_abi() {
    let translation = translate_source(
        r#"
interface Port<T> { int size(); }
interface PortOwner<T> { Port<T> port(); }
"#,
    );
    assert!(
        translation.rust.contains("dyn Port<T>"),
        "{}",
        translation
            .rust
            .lines()
            .filter(|line| line.contains("fn port("))
            .collect::<Vec<_>>()
            .join("\n")
    );
    cargo_check_generated_modules(
        "generic_interface_return_abi",
        vec![GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: "",
        }],
    );
}

#[test]
fn source_generic_class_retains_arguments_when_its_name_shadows_a_builtin_facade() {
    // This name also has a built-in external model. A co-translated source
    // declaration, not that unavailable dependency, owns its generic ABI.
    let translation = translate_source(
        r#"
import java.util.List;
class ImmutablePair<T> { T value; }
class SourceHolder {
    List<ImmutablePair<String>> values;
}
"#,
    );
    assert!(
        translation.rust.contains("ImmutablePair<String>"),
        "{}",
        translation
            .rust
            .lines()
            .filter(|line| line.contains("values:"))
            .collect::<Vec<_>>()
            .join("\n")
    );
    cargo_check_generated_modules(
        "source_generic_facade_precedence",
        vec![GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: "",
        }],
    );
}
