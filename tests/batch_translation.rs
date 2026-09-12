mod common;

use common::{GeneratedCrateModule, cargo_check_generated_modules};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

fn unique_temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "new_java2rust_{name}_{}_{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn write_batch_sources(root: &Path) {
    fs::create_dir_all(root).unwrap();
    fs::write(
        root.join("A.java"),
        r#"
package demo;

public class A {
    public int inc(int value) {
        return value + 1;
    }

    public int callB() {
        return new B().two();
    }
}
"#,
    )
    .unwrap();
    fs::write(
        root.join("B.java"),
        r#"
package demo;

public class B {
    public int two() {
        return 2;
    }
}
"#,
    )
    .unwrap();
}

fn generated_source_items_from_marker(rust: &str, marker: &str) -> String {
    let start = rust
        .find(marker)
        .unwrap_or_else(|| panic!("missing generated source marker {marker:?}:\n{rust}"));
    rust[start..].to_string()
}

fn generated_source_item_marker_count(rust: &str, marker: &str) -> usize {
    rust.matches(marker).count()
}

#[test]
fn batch_parallel_emission_matches_serial_output_order() {
    let primary_sources = vec![
        new_java2rust::BatchSource::new(
            "A.java".to_string(),
            r#"
public class A {
    public int inc(int value) {
        return value + 1;
    }
}
"#
            .to_string(),
        ),
        new_java2rust::BatchSource::new(
            "B.java".to_string(),
            r#"
public class B {
    public int two() {
        return 2;
    }
}
"#
            .to_string(),
        ),
    ];

    let serial =
        new_java2rust::translate_batch_sources_with_context_sources_summary_sources_and_options(
            &primary_sources,
            &[],
            &[],
            true,
            &new_java2rust::facts::FactSet::default(),
            &new_java2rust::BatchTranslationOptions::new(1, None),
        );
    let parallel =
        new_java2rust::translate_batch_sources_with_context_sources_summary_sources_and_options(
            &primary_sources,
            &[],
            &[],
            true,
            &new_java2rust::facts::FactSet::default(),
            &new_java2rust::BatchTranslationOptions::new(2, None),
        );

    assert_eq!(serial.translations().len(), parallel.translations().len());
    assert_eq!(serial.events().len(), parallel.events().len());
    for (serial, parallel) in serial.translations().iter().zip(parallel.translations()) {
        assert_eq!(serial.path(), parallel.path());
        assert_eq!(serial.rust(), parallel.rust());
        assert_eq!(serial.diagnostics().len(), parallel.diagnostics().len());
    }
    assert_eq!(
        parallel
            .events()
            .iter()
            .map(|event| (event.phase(), event.index(), event.status()))
            .collect::<Vec<_>>(),
        vec![
            ("emit_start", 1, "running"),
            ("emit_end", 1, "ok"),
            ("emit_start", 2, "running"),
            ("emit_end", 2, "ok")
        ]
    );
}

#[test]
fn batch_translation_reports_inner_class_constructor_panics() {
    let translations =
        new_java2rust::translate_batch_sources_with_context_sources_and_summary_sources(
            &[new_java2rust::BatchSource::new(
                "MarkerInnerCapture.java".to_string(),
                r#"
public class MarkerInnerCapture {
    private int base = 10;
    class Adder {
        int add(int x) { return base + x; }
    }
    public int run() {
        Adder a = new Adder();
        base = 20;
        return a.add(1);
    }
}
"#
                .to_string(),
            )],
            &[],
            &[],
            true,
            &new_java2rust::facts::FactSet::default(),
        );

    assert_eq!(translations.len(), 1);
    assert!(
        translations[0]
            .diagnostics()
            .has_code("inner_class_construction_panics"),
        "batch translation should report the runtime panic marker as a \
         diagnostic:\n{:?}",
        translations[0].diagnostics().iter().collect::<Vec<_>>()
    );
}

#[test]
fn batch_duplicate_simple_type_names_keep_distinct_signature_keys() {
    let primary_sources = vec![
        new_java2rust::BatchSource::new(
            "a/DuplicateOwner.java".to_string(),
            r#"
package a;

public class DuplicateOwner {
    private int first;

    public int firstValue() {
        return first;
    }
}
"#
            .to_string(),
        ),
        new_java2rust::BatchSource::new(
            "b/DuplicateOwner.java".to_string(),
            r#"
package b;

public class DuplicateOwner {
    private int second;

    public int secondValue() {
        return second;
    }
}
"#
            .to_string(),
        ),
    ];

    let report =
        new_java2rust::translate_batch_sources_with_context_sources_summary_sources_and_options(
            &primary_sources,
            &[],
            &[],
            true,
            &new_java2rust::facts::FactSet::default(),
            &new_java2rust::BatchTranslationOptions::new(2, None),
        );

    assert_eq!(report.translations().len(), 2);
    assert!(
        report.translations()[0].rust().contains("first_value")
            && report.translations()[1].rust().contains("second_value"),
        "{:#?}",
        report
            .translations()
            .iter()
            .map(|translation| (translation.path(), translation.rust()))
            .collect::<Vec<_>>()
    );
    cargo_check_generated_modules(
        "batch_duplicate_simple_type_names",
        report
            .translations()
            .iter()
            .enumerate()
            .map(|(index, translation)| GeneratedCrateModule {
                name: if index == 0 { "a_owner" } else { "b_owner" },
                rust: translation.rust().to_string(),
                tests: "",
            })
            .collect(),
    );
}

#[test]
fn explicit_import_aliases_survive_duplicate_context_simple_names() {
    let primary_sources = vec![
        new_java2rust::BatchSource::new(
            "a/Cfg.java".to_string(),
            r#"
package a;

public class Cfg {
    public static final int N = 10;

    public static int who() {
        return 1;
    }

    public int id() {
        return 100;
    }
}
"#
            .to_string(),
        ),
        new_java2rust::BatchSource::new(
            "b/Cfg.java".to_string(),
            r#"
package b;

public class Cfg {
    public static final int N = 20;

    public static int who() {
        return 2;
    }

    public int id() {
        return 200;
    }
}
"#
            .to_string(),
        ),
        new_java2rust::BatchSource::new(
            "UseA.java".to_string(),
            r#"
import a.Cfg;

public class UseA {
    public static int run() {
        Cfg c = new Cfg();
        return Cfg.N + Cfg.who() + c.id();
    }
}
"#
            .to_string(),
        ),
        new_java2rust::BatchSource::new(
            "UseB.java".to_string(),
            r#"
import b.Cfg;

public class UseB {
    public static int run() {
        Cfg c = new Cfg();
        return Cfg.N + Cfg.who() + c.id();
    }
}
"#
            .to_string(),
        ),
    ];

    let report =
        new_java2rust::translate_batch_sources_with_context_sources_summary_sources_and_options(
            &primary_sources,
            &[],
            &[],
            true,
            &new_java2rust::facts::FactSet::default(),
            &new_java2rust::BatchTranslationOptions::new(2, None),
        );

    let use_a = report
        .translations()
        .iter()
        .find(|translation| translation.path() == "UseA.java")
        .unwrap_or_else(|| panic!("{:#?}", report.translations()));
    let use_b = report
        .translations()
        .iter()
        .find(|translation| translation.path() == "UseB.java")
        .unwrap_or_else(|| panic!("{:#?}", report.translations()));

    assert!(
        use_a.rust().contains("ACfg::N")
            && use_a.rust().contains("ACfg::who()")
            && !use_a.rust().contains("BCfg::N")
            && !use_a.rust().contains("BCfg::who()"),
        "{}",
        use_a.rust()
    );
    assert!(
        use_b.rust().contains("BCfg::N")
            && use_b.rust().contains("BCfg::who()")
            && !use_b.rust().contains("ACfg::N")
            && !use_b.rust().contains("ACfg::who()"),
        "{}",
        use_b.rust()
    );
}

#[test]
fn package_qualified_source_type_references_keep_package_identity() {
    let primary_sources = vec![
        new_java2rust::BatchSource::new(
            "a/Item.java".to_string(),
            r#"
package a;

public class Item {
    public static final int KIND = 1;

    public int who() {
        return 1;
    }
}
"#
            .to_string(),
        ),
        new_java2rust::BatchSource::new(
            "b/Item.java".to_string(),
            r#"
package b;

public class Item {
    public static final int KIND = 2;

    public int who() {
        return 2;
    }
}
"#
            .to_string(),
        ),
        new_java2rust::BatchSource::new(
            "Main.java".to_string(),
            r#"
public class Main {
    public static int run() {
        a.Item ai = new a.Item();
        b.Item bi = new b.Item();
        return ai.who() + bi.who() + a.Item.KIND + b.Item.KIND;
    }
}
"#
            .to_string(),
        ),
    ];

    let report =
        new_java2rust::translate_batch_sources_with_context_sources_summary_sources_and_options(
            &primary_sources,
            &[],
            &[],
            true,
            &new_java2rust::facts::FactSet::default(),
            &new_java2rust::BatchTranslationOptions::new(2, None),
        );

    let main = report
        .translations()
        .iter()
        .find(|translation| translation.path() == "Main.java")
        .unwrap_or_else(|| panic!("{:#?}", report.translations()));
    let a_item = report
        .translations()
        .iter()
        .find(|translation| translation.path() == "a/Item.java")
        .unwrap_or_else(|| panic!("{:#?}", report.translations()));
    let b_item = report
        .translations()
        .iter()
        .find(|translation| translation.path() == "b/Item.java")
        .unwrap_or_else(|| panic!("{:#?}", report.translations()));

    assert!(main.rust().contains("AItem::new()"), "{}", main.rust());
    assert!(main.rust().contains("BItem::new()"), "{}", main.rust());
    assert!(main.rust().contains("AItem::KIND"), "{}", main.rust());
    assert!(main.rust().contains("BItem::KIND"), "{}", main.rust());
    assert!(!main.rust().contains("let ai: Item"), "{}", main.rust());
    assert!(!main.rust().contains("let bi: Item"), "{}", main.rust());
    assert!(!main.rust().contains("= Item::new()"), "{}", main.rust());
    assert!(
        a_item.rust().contains("pub struct AItem"),
        "{}",
        a_item.rust()
    );
    assert!(
        b_item.rust().contains("pub struct BItem"),
        "{}",
        b_item.rust()
    );
    assert_eq!(
        generated_source_item_marker_count(&a_item.rust(), "pub struct AItem"),
        1,
        "{}",
        a_item.rust()
    );
    assert_eq!(
        generated_source_item_marker_count(&b_item.rust(), "pub struct BItem"),
        1,
        "{}",
        b_item.rust()
    );
    assert_eq!(
        generated_source_item_marker_count(&a_item.rust(), "pub struct BItem")
            + generated_source_item_marker_count(&b_item.rust(), "pub struct AItem"),
        0,
        "package-peer emitted items should not cross owners:\n{}\n{}",
        a_item.rust(),
        b_item.rust()
    );
    assert!(
        !a_item.rust().contains("pub type Item = AItem;"),
        "{}",
        a_item.rust()
    );
    assert!(
        !b_item.rust().contains("pub type Item = BItem;"),
        "{}",
        b_item.rust()
    );
    assert!(
        !a_item.rust().contains("pub struct Item") && !b_item.rust().contains("pub struct Item"),
        "declarations should not collapse to the simple name:\n{}\n{}",
        a_item.rust(),
        b_item.rust()
    );

    cargo_check_generated_modules(
        "batch_same_simple_package_peers",
        vec![GeneratedCrateModule {
            name: "fixture",
            rust: format!(
                "pub struct JavaClass<T> {{ name: &'static str, _marker: std::marker::PhantomData<T> }}\n\
                 impl<T> JavaClass<T> {{ pub fn literal(name: &'static str) -> Self {{ Self {{ name, _marker: std::marker::PhantomData }} }} pub fn get_name(&self) -> String {{ self.name.to_string() }} }}\n\
                 pub fn java_erased_object_to_string<T>(_value: &T) -> String {{ panic!(\"unsupported Java erased object display: runtime target is erased\") }}\n\
                 {}\n{}\n{}",
                generated_source_items_from_marker(&a_item.rust(), "pub struct AItem"),
                generated_source_items_from_marker(&b_item.rust(), "pub struct BItem"),
                generated_source_items_from_marker(&main.rust(), "pub struct Main")
            ),
            tests: "",
        }],
    );
}

#[test]
fn stream_static_constant_support_keeps_package_qualified_owner_identity() {
    let primary_sources = vec![
        new_java2rust::BatchSource::new(
            "a/Item.java".to_string(),
            r#"
package a;

public class Item {
    public static final int KIND = 1;
}
"#
            .to_string(),
        ),
        new_java2rust::BatchSource::new(
            "b/Item.java".to_string(),
            r#"
package b;

public class Item {
    public static final int KIND = 2;
}
"#
            .to_string(),
        ),
        new_java2rust::BatchSource::new(
            "Main.java".to_string(),
            r#"
public class Main {
    public static java.util.List<Integer> stream(java.util.List<Integer> xs) {
        return xs.stream().map(x -> a.Item.KIND + b.Item.KIND + x).toList();
    }
}
"#
            .to_string(),
        ),
    ];

    let report =
        new_java2rust::translate_batch_sources_with_context_sources_summary_sources_and_options(
            &primary_sources,
            &[],
            &[],
            true,
            &new_java2rust::facts::FactSet::default(),
            &new_java2rust::BatchTranslationOptions::new(2, None),
        );

    let main = report
        .translations()
        .iter()
        .find(|translation| translation.path() == "Main.java")
        .unwrap_or_else(|| panic!("{:#?}", report.translations()));
    let a_item = report
        .translations()
        .iter()
        .find(|translation| translation.path() == "a/Item.java")
        .unwrap_or_else(|| panic!("{:#?}", report.translations()));
    let b_item = report
        .translations()
        .iter()
        .find(|translation| translation.path() == "b/Item.java")
        .unwrap_or_else(|| panic!("{:#?}", report.translations()));
    assert!(main.rust().contains("AItem::KIND"), "{}", main.rust());
    assert!(main.rust().contains("BItem::KIND"), "{}", main.rust());
    assert!(!main.rust().contains("pub struct Item;"), "{}", main.rust());
    assert!(
        !main.rust().contains("pub type Item = AItem;"),
        "{}",
        main.rust()
    );
    assert!(
        !main.rust().contains("pub type Item = BItem;"),
        "{}",
        main.rust()
    );
    assert!(
        !a_item.rust().contains("pub type Item = AItem;"),
        "{}",
        a_item.rust()
    );
    assert!(
        !b_item.rust().contains("pub type Item = BItem;"),
        "{}",
        b_item.rust()
    );
    assert!(
        !main.rust().contains("pub const KIND: KIND = KIND;"),
        "{}",
        main.rust()
    );
}

#[test]
fn imported_source_enum_value_of_keeps_package_owner_identity() {
    let primary_sources = vec![
        new_java2rust::BatchSource::new(
            "a/Mode.java".to_string(),
            r#"
package a;

public enum Mode {
    FIRST,
    SECOND
}
"#
            .to_string(),
        ),
        new_java2rust::BatchSource::new(
            "b/Mode.java".to_string(),
            r#"
package b;

public enum Mode {
    OTHER
}
"#
            .to_string(),
        ),
        new_java2rust::BatchSource::new(
            "Main.java".to_string(),
            r#"
import a.Mode;

public class Main {
    public static int run(String name) {
        return Mode.valueOf(name).ordinal();
    }
}
"#
            .to_string(),
        ),
    ];

    let report =
        new_java2rust::translate_batch_sources_with_context_sources_summary_sources_and_options(
            &primary_sources,
            &[],
            &[],
            true,
            &new_java2rust::facts::FactSet::default(),
            &new_java2rust::BatchTranslationOptions::new(2, None),
        );

    let main = report
        .translations()
        .iter()
        .find(|translation| translation.path() == "Main.java")
        .unwrap_or_else(|| panic!("{:#?}", report.translations()));
    let a_mode = report
        .translations()
        .iter()
        .find(|translation| translation.path() == "a/Mode.java")
        .unwrap_or_else(|| panic!("{:#?}", report.translations()));
    let b_mode = report
        .translations()
        .iter()
        .find(|translation| translation.path() == "b/Mode.java")
        .unwrap_or_else(|| panic!("{:#?}", report.translations()));

    assert!(
        main.rust().contains("AMode::value_of(name.as_str())"),
        "{}",
        main.rust()
    );
    assert!(
        !main.rust().contains("return Mode::value_of("),
        "{}",
        main.rust()
    );
    assert!(!main.rust().contains("BMode::value_of("), "{}", main.rust());

    cargo_check_generated_modules(
        "batch_imported_source_enum_value_of_package_owner",
        vec![GeneratedCrateModule {
            name: "fixture",
            rust: format!(
                "pub struct JavaClass<T> {{ name: &'static str, _marker: std::marker::PhantomData<T> }}\n\
                 impl<T> JavaClass<T> {{ pub fn literal(name: &'static str) -> Self {{ Self {{ name, _marker: std::marker::PhantomData }} }} pub fn get_name(&self) -> String {{ self.name.to_string() }} }}\n\
                 pub fn java_erased_object_to_string<T>(_value: &T) -> String {{ panic!(\"unsupported Java erased object display: runtime target is erased\") }}\n\
                 {}\n{}\n{}",
                generated_source_items_from_marker(&a_mode.rust(), "pub enum AMode"),
                generated_source_items_from_marker(&b_mode.rust(), "pub enum BMode"),
                generated_source_items_from_marker(&main.rust(), "pub struct Main")
            ),
            tests: "",
        }],
    );
}

#[test]
fn package_qualified_class_literals_keep_package_identity() {
    let primary_sources = vec![
        new_java2rust::BatchSource::new(
            "a/Item.java".to_string(),
            r#"
package a;

public class Item {}
"#
            .to_string(),
        ),
        new_java2rust::BatchSource::new(
            "b/Item.java".to_string(),
            r#"
package b;

public class Item {}
"#
            .to_string(),
        ),
        new_java2rust::BatchSource::new(
            "Main.java".to_string(),
            r#"
public class Main {
    public static String run() {
        return a.Item.class.getName() + "," + b.Item.class.getName();
    }
}
"#
            .to_string(),
        ),
    ];

    let report =
        new_java2rust::translate_batch_sources_with_context_sources_summary_sources_and_options(
            &primary_sources,
            &[],
            &[],
            true,
            &new_java2rust::facts::FactSet::default(),
            &new_java2rust::BatchTranslationOptions::new(2, None),
        );

    let main = report
        .translations()
        .iter()
        .find(|translation| translation.path() == "Main.java")
        .unwrap_or_else(|| panic!("{:#?}", report.translations()));
    let a_item = report
        .translations()
        .iter()
        .find(|translation| translation.path() == "a/Item.java")
        .unwrap_or_else(|| panic!("{:#?}", report.translations()));
    let b_item = report
        .translations()
        .iter()
        .find(|translation| translation.path() == "b/Item.java")
        .unwrap_or_else(|| panic!("{:#?}", report.translations()));
    let class_literal_lines = main
        .rust()
        .lines()
        .filter(|line| line.contains("JavaClass"))
        .collect::<Vec<_>>()
        .join("\n");

    assert!(
        main.rust()
            .contains("JavaClass::<AItem>::literal(\"a.Item\")"),
        "missing a.Item class literal:\n{class_literal_lines}"
    );
    assert!(
        main.rust()
            .contains("JavaClass::<BItem>::literal(\"b.Item\")"),
        "missing b.Item class literal:\n{class_literal_lines}"
    );
    assert!(
        !main.rust().contains("JavaClass::<Item>::literal(\"Item\")"),
        "class literal collapsed to simple Item"
    );
    assert!(
        a_item.rust().contains("pub struct AItem"),
        "{}",
        a_item.rust()
    );
    assert!(
        b_item.rust().contains("pub struct BItem"),
        "{}",
        b_item.rust()
    );
}

#[test]
fn same_package_simple_class_literal_keeps_java_package_name() {
    let translation = new_java2rust::translate_source(
        r#"
package a;

public class Item {
    public static String run() {
        return Item.class.getName();
    }
}
"#,
    );

    assert!(
        translation
            .rust
            .contains("JavaClass::<Item>::literal(\"a.Item\")"),
        "{}",
        translation.rust
    );
    assert!(
        !translation
            .rust
            .contains("JavaClass::<Item>::literal(\"Item\")"),
        "{}",
        translation.rust
    );
    cargo_check_generated_modules(
        "same_package_simple_class_literal",
        vec![GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: "",
        }],
    );
}

#[test]
fn external_pack_object_facts_do_not_erase_primary_source_type_emission() {
    let primary_sources = vec![new_java2rust::BatchSource::new(
        "modeled/ModeledPrimary.java".to_string(),
        r#"
package modeled;

public class ModeledPrimary {
    private int value;

    public int getValue() {
        return value;
    }
}
"#
        .to_string(),
    )];
    let facts = new_java2rust::external_packs::parse_external_pack_manifest(
        r#"
        external-pack modeled modeled external-fact-model semantic
        external-pack-object modeled modeled.ModeledPrimary erased-object
        external-pack-object modeled ModeledPrimary erased-object
        "#,
    )
    .facts()
    .clone();

    let report =
        new_java2rust::translate_batch_sources_with_context_sources_summary_sources_and_options(
            &primary_sources,
            &[],
            &[],
            true,
            &facts,
            &new_java2rust::BatchTranslationOptions::default(),
        );

    assert_eq!(report.translations().len(), 1);
    let rust = report.translations()[0].rust();
    assert!(rust.contains("pub struct ModeledPrimary"), "{rust}");
    assert!(rust.contains("pub fn get_value(&self) -> i32"), "{rust}");
    assert!(
        !rust.contains("pub type ModeledPrimary = Box<dyn std::any::Any>;"),
        "{rust}"
    );
}

#[test]
fn count_invalid_patterns_cli_uses_registry_counter() {
    let dir = unique_temp_dir("count_invalid_patterns_cli");
    let rust_path = dir.join("generated.rs");
    fs::write(
        &rust_path,
        r#"
pub fn bad() {
    compile_error!("unsupported Java expression: lambda");
    let literal = "compile_error!";
}
"#,
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_new-java2rust"))
        .arg("count-invalid-patterns")
        .arg(&rust_path)
        .output()
        .expect("run count-invalid-patterns");
    assert!(
        output.status.success(),
        "count-invalid-patterns failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), "1");
}

#[test]
fn translate_batch_cli_writes_phase_and_worker_events() {
    let dir = unique_temp_dir("translate_batch_events");
    let source_dir = dir.join("java");
    let out_dir = dir.join("out");
    let generated_dir = out_dir.join("generated");
    let phase_events = out_dir.join("phase-events.tsv");
    let worker_events = out_dir.join("worker-events.tsv");
    write_batch_sources(&source_dir);
    fs::create_dir_all(&out_dir).unwrap();
    fs::write(&phase_events, "phase\tstatus\telapsed_ms\tdetail\n").unwrap();
    fs::write(
        &worker_events,
        "event\tpid\tindex\tsource\telapsed_ms\tstatus\trust_path\n",
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_new-java2rust"))
        .arg("translate-batch")
        .arg("--primary")
        .arg(&source_dir)
        .arg("--out-dir")
        .arg(&out_dir)
        .arg("--generated-dir")
        .arg(&generated_dir)
        .arg("--phase-events")
        .arg(&phase_events)
        .arg("--worker-events")
        .arg(&worker_events)
        .output()
        .expect("run translate-batch");
    assert!(
        output.status.success(),
        "translate-batch failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let status = fs::read_to_string(out_dir.join("status.tsv")).unwrap();
    assert_eq!(status.lines().count(), 3, "{status}");
    assert!(
        status.starts_with(
            "source\trust_path\ttranslate_status\tdiagnostics\tinvalid_patterns\tunsupported_members\tstubbed_constants\n"
        ),
        "{status}"
    );
    assert!(
        status.lines().skip(1).all(|line| {
            let fields = line.split('\t').collect::<Vec<_>>();
            fields.len() == 7
        }),
        "{status}"
    );
    assert!(status.contains("\tok\t"), "{status}");

    let manifest = fs::read_to_string(out_dir.join("batch-manifest.tsv")).unwrap();
    assert_eq!(manifest.lines().count(), 3, "{manifest}");
    assert!(
        manifest.lines().skip(1).all(|line| {
            let fields = line.split('\t').collect::<Vec<_>>();
            fields.len() == 7 && fields[3].len() == 16
        }),
        "{manifest}"
    );

    let phase_events = fs::read_to_string(phase_events).unwrap();
    assert!(
        phase_events.contains("batch_primary_parse\tok\t"),
        "{phase_events}"
    );
    assert!(
        phase_events.contains("batch_global_analysis\tok\t"),
        "{phase_events}"
    );
    assert!(
        phase_events.contains("batch_global_resolve_context\tok\t"),
        "{phase_events}"
    );
    assert!(
        phase_events.contains("batch_global_semantic\tok\t"),
        "{phase_events}"
    );
    assert!(
        phase_events.contains("batch_global_representation\tok\t"),
        "{phase_events}"
    );
    assert!(
        phase_events.contains("batch_global_signature\tok\t"),
        "{phase_events}"
    );
    assert!(
        phase_events.contains("batch_global_signature_fixpoint\tok\t"),
        "{phase_events}"
    );
    assert!(
        phase_events.contains("batch_emission\tok\t"),
        "{phase_events}"
    );
    assert!(
        phase_events.contains("\tjobs=10"),
        "translate-batch should default to 10 jobs\n{phase_events}"
    );

    let analysis_profile = fs::read_to_string(out_dir.join("analysis-profile.tsv")).unwrap();
    assert!(
        analysis_profile.contains("metric\tvalue\n"),
        "{analysis_profile}"
    );
    assert!(
        analysis_profile.contains("primary_files\t2\n"),
        "{analysis_profile}"
    );
    assert!(
        analysis_profile.contains("program_types\t2\n"),
        "{analysis_profile}"
    );
    assert!(
        analysis_profile.contains("signature_slot_count\t"),
        "{analysis_profile}"
    );
    assert!(
        analysis_profile.contains("signature_fixpoint_iterations\t"),
        "{analysis_profile}"
    );
    assert!(
        analysis_profile.contains("signature_component_sweeps\t"),
        "{analysis_profile}"
    );
    assert!(
        analysis_profile.contains("signature_state_clones\t"),
        "{analysis_profile}"
    );
    assert!(
        analysis_profile.contains("signature_propagation_arm_call_site_sweeps\t"),
        "{analysis_profile}"
    );

    let worker_events = fs::read_to_string(worker_events).unwrap();
    assert!(
        worker_events
            .lines()
            .any(|line| line.starts_with("batch_global_signature\t") && line.contains("\t0\t")),
        "{worker_events}"
    );
    assert!(
        worker_events.lines().any(|line| {
            line.starts_with("batch_global_signature_fixpoint_iteration_live\t")
                && line.contains("\t0\t")
        }),
        "{worker_events}"
    );
    assert!(
        worker_events
            .lines()
            .any(|line| line.starts_with("batch_emission\t") && line.contains("\t0\t")),
        "{worker_events}"
    );
    assert!(
        worker_events
            .lines()
            .any(|line| line.starts_with("emit_start\t") && line.contains("\t1\t")),
        "{worker_events}"
    );
    assert!(
        worker_events
            .lines()
            .any(|line| line.starts_with("emit_end\t") && line.contains("\t2\t")),
        "{worker_events}"
    );
}

#[test]
fn translate_cli_exits_nonzero_on_java_syntax_errors() {
    let dir = unique_temp_dir("translate_cli_syntax_error");
    let source = dir.join("Broken.java");
    fs::write(
        &source,
        r#"
public class Broken {
    int bad(int a, int b) {
        return a + b ~;
    }
}
"#,
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_new-java2rust"))
        .arg(&source)
        .output()
        .expect("run translator");
    assert!(
        !output.status.success(),
        "translator unexpectedly succeeded\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("java_syntax"),
        "stderr:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn translate_batch_status_reports_error_diagnostics() {
    let dir = unique_temp_dir("translate_batch_syntax_error_status");
    let source_dir = dir.join("java");
    let out_dir = dir.join("out");
    let generated_dir = out_dir.join("generated");
    fs::create_dir_all(&source_dir).unwrap();
    fs::write(
        source_dir.join("Broken.java"),
        r#"
public class Broken {
    int bad(int a, int b) {
        return a + b ~;
    }
}
"#,
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_new-java2rust"))
        .arg("translate-batch")
        .arg("--primary")
        .arg(&source_dir)
        .arg("--out-dir")
        .arg(&out_dir)
        .arg("--generated-dir")
        .arg(&generated_dir)
        .output()
        .expect("run translate-batch");
    assert!(
        output.status.success(),
        "translate-batch failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let status = fs::read_to_string(out_dir.join("status.tsv")).unwrap();
    let row = status
        .lines()
        .nth(1)
        .unwrap_or_else(|| panic!("missing status row\n{status}"));
    let fields = row.split('\t').collect::<Vec<_>>();
    assert_eq!(fields.len(), 7, "{status}");
    assert_eq!(fields[2], "errors", "{status}");
    assert!(fields[3].parse::<usize>().unwrap_or(0) > 0, "{status}");
}

#[test]
fn translate_batch_cli_counts_unsupported_member_diagnostics_in_status() {
    let dir = unique_temp_dir("translate_batch_unsupported_member_status");
    let source_dir = dir.join("java");
    let out_dir = dir.join("out");
    let generated_dir = out_dir.join("generated");
    fs::create_dir_all(&source_dir).unwrap();
    fs::write(
        source_dir.join("InstanceInit.java"),
        r#"
public class InstanceInit {
    {
        int value = 1;
    }
}
"#,
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_new-java2rust"))
        .arg("translate-batch")
        .arg("--primary")
        .arg(&source_dir)
        .arg("--out-dir")
        .arg(&out_dir)
        .arg("--generated-dir")
        .arg(&generated_dir)
        .output()
        .expect("run translate-batch");
    assert!(
        output.status.success(),
        "translate-batch failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let status = fs::read_to_string(out_dir.join("status.tsv")).unwrap();
    let row = status
        .lines()
        .nth(1)
        .unwrap_or_else(|| panic!("missing status row\n{status}"));
    let fields = row.split('\t').collect::<Vec<_>>();
    assert_eq!(fields.len(), 7, "{status}");
    assert!(fields[5].parse::<usize>().unwrap_or(0) > 0, "{status}");
}

#[test]
fn translate_batch_cli_counts_unmodelled_external_constants_in_status() {
    let dir = unique_temp_dir("translate_batch_stubbed_constant_status");
    let source_dir = dir.join("java");
    let out_dir = dir.join("out");
    let generated_dir = out_dir.join("generated");
    fs::create_dir_all(&source_dir).unwrap();
    fs::write(
        source_dir.join("UseExternalLimit.java"),
        r#"
public class UseExternalLimit {
    int limit() {
        return ExternalLimits.MAX_COUNT;
    }
}
"#,
    )
    .unwrap();
    let manifest = dir.join("pack.txt");
    fs::write(
        &manifest,
        "external-pack external external semantic\n\
         external-pack-field external ExternalLimits MAX_COUNT semantic type=int\n",
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_new-java2rust"))
        .arg("translate-batch")
        .arg("--primary")
        .arg(&source_dir)
        .arg("--out-dir")
        .arg(&out_dir)
        .arg("--generated-dir")
        .arg(&generated_dir)
        .arg("--external-pack-manifest")
        .arg(&manifest)
        .output()
        .expect("run translate-batch");
    assert!(
        output.status.success(),
        "translate-batch failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let status = fs::read_to_string(out_dir.join("status.tsv")).unwrap();
    let row = status
        .lines()
        .nth(1)
        .unwrap_or_else(|| panic!("missing status row\n{status}"));
    let fields = row.split('\t').collect::<Vec<_>>();
    assert_eq!(fields.len(), 7, "{status}");
    assert_eq!(fields[6], "1", "{status}");

    let manifest = fs::read_to_string(out_dir.join("batch-manifest.tsv")).unwrap();
    let row = manifest
        .lines()
        .nth(1)
        .unwrap_or_else(|| panic!("missing manifest row\n{manifest}"));
    let fields = row.split('\t').collect::<Vec<_>>();
    assert_eq!(fields.len(), 7, "{manifest}");
    assert_eq!(fields[6], "1", "{manifest}");
}

fn write_comparable_batch_output(root: &Path, hash: &str) {
    fs::create_dir_all(root).unwrap();
    fs::write(
        root.join("batch-manifest.tsv"),
        format!(
            "index\tsource\trust_path\trust_hash\tdiagnostics\tinvalid_patterns\tstubbed_constants\n1\tA.java\t{}/generated/00001_A.rs\t{}\t0\t0\t0\n",
            root.display(),
            hash
        ),
    )
    .unwrap();
    fs::write(
        root.join("status.tsv"),
        format!(
            "source\trust_path\ttranslate_status\tdiagnostics\tinvalid_patterns\tunsupported_members\tstubbed_constants\nA.java\t{}/generated/00001_A.rs\tok\t0\t0\t0\t0\n",
            root.display()
        ),
    )
    .unwrap();
    fs::write(
        root.join("invalid-patterns.tsv"),
        "pattern\tcount\tfiles\ncompile_error_placeholder\t0\t0\n",
    )
    .unwrap();
    fs::write(root.join("top-compile-errors.tsv"), "count\terror\n").unwrap();
    fs::write(
        root.join("summary.tsv"),
        "files\t1\ntranslate_failed_files\t0\ndiagnostic_files\t0\ninvalid_pattern_files\t0\ncargo_check\tok\ncargo_check_errors\t0\n",
    )
    .unwrap();
}

#[test]
fn focused_batch_artifact_comparator_ignores_output_prefixes_and_checks_hashes() {
    let dir = unique_temp_dir("batch_artifact_compare");
    let serial = dir.join("serial");
    let parallel = dir.join("parallel");
    write_comparable_batch_output(&serial, "0123456789abcdef");
    write_comparable_batch_output(&parallel, "0123456789abcdef");

    let script =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("scripts/compare-focused-batch-artifacts.py");
    let output = Command::new("python3")
        .arg(&script)
        .arg("--serial-dir")
        .arg(&serial)
        .arg("--parallel-dir")
        .arg(&parallel)
        .output()
        .expect("run comparator");
    assert!(
        output.status.success(),
        "comparator failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    write_comparable_batch_output(&parallel, "fedcba9876543210");
    let output = Command::new("python3")
        .arg(script)
        .arg("--serial-dir")
        .arg(&serial)
        .arg("--parallel-dir")
        .arg(&parallel)
        .output()
        .expect("run comparator");
    assert!(
        !output.status.success(),
        "comparator should reject changed hash\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        String::from_utf8_lossy(&output.stdout).contains("batch-manifest differs"),
        "{}",
        String::from_utf8_lossy(&output.stdout)
    );
}

#[test]
fn focused_package_offline_cargo_uses_local_ndarray_shim() {
    let dir = unique_temp_dir("focused_ndarray_shim");
    let primary = dir.join("java");
    let out = dir.join("out");
    fs::create_dir_all(&primary).unwrap();
    fs::write(
        primary.join("MatrixSource.java"),
        r#"
public class MatrixSource {
    public double[][] literal() {
        return new double[][] {{1.0, 2.0}, {3.0, 4.0}};
    }
}
"#,
    )
    .unwrap();

    let script = Path::new(env!("CARGO_MANIFEST_DIR")).join("scripts/translate-focused-package.sh");
    let mut command = Command::new(&script);
    command
        .arg("--skip-resolver-bench")
        .arg("--offline-cargo")
        .arg("--jobs")
        .arg("1")
        .arg("--primary")
        .arg(&primary)
        .arg("--out")
        .arg(&out);
    if let Ok(binary) = std::env::var("CARGO_BIN_EXE_new-java2rust") {
        command.env("NEW_JAVA2RUST_BIN", binary);
    }
    let output = command.output().expect("run focused package wrapper");
    assert!(
        output.status.success(),
        "focused wrapper failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let cargo_toml = fs::read_to_string(out.join("generated-crate/Cargo.toml")).unwrap();
    assert!(
        cargo_toml.contains("ndarray = { path = \"ndarray-shim\" }"),
        "{cargo_toml}"
    );
    assert!(
        out.join("generated-crate/ndarray-shim/src/lib.rs").exists(),
        "missing local ndarray shim"
    );
    let ndarray_shim =
        fs::read_to_string(out.join("generated-crate/ndarray-shim/src/lib.rs")).unwrap();
    assert!(
        ndarray_shim.contains("pub fn from_rows(rows: &[&[T]]) -> Self")
            && ndarray_shim.contains("pub fn iter(&self) -> Rows<'_, T>"),
        "{ndarray_shim}"
    );
    let summary = fs::read_to_string(out.join("summary.tsv")).unwrap();
    assert!(summary.contains("cargo_check\tok"), "{summary}");
    assert!(summary.contains("cargo_failure_kind\tnone"), "{summary}");
    assert!(summary.contains("parse_failures\t0"), "{summary}");
    assert!(summary.contains("cargo_type_errors_valid\t1"), "{summary}");
    assert!(summary.contains("unsupported_statements\t0"), "{summary}");
    assert!(summary.contains("unsupported_members\t0"), "{summary}");
    assert!(summary.contains("unsupported_expressions\t0"), "{summary}");
    assert!(
        summary.lines().any(|line| line
            .strip_prefix("unmodelled_types\t")
            .is_some_and(|count| count.parse::<usize>().is_ok())),
        "{summary}"
    );
    assert!(summary.contains("unmodelled_call_sites\t0"), "{summary}");
    assert!(summary.contains("noop_facade_methods\t"), "{summary}");
    assert!(summary.contains("noop_facade_call_sites\t"), "{summary}");
    assert!(
        summary.lines().any(|line| line
            .strip_prefix("unmodelled_upcast_conversions\t")
            .is_some_and(|count| count.parse::<usize>().is_ok_and(|count| count > 0))),
        "{summary}"
    );
    assert!(summary.contains("stubbed_constants\t0"), "{summary}");
    assert!(
        out.join("parse-failures.tsv").exists(),
        "missing parse failure report"
    );
}

#[cfg(unix)]
#[test]
fn focused_package_generated_rust_parse_error_invalidates_type_error_count() {
    let dir = unique_temp_dir("focused_parse_gate");
    let primary = dir.join("java");
    let out = dir.join("out");
    fs::create_dir_all(&primary).unwrap();
    fs::write(
        primary.join("Broken.java"),
        r#"
public class Broken {
    public int value() {
        return 1;
    }
}
"#,
    )
    .unwrap();

    let fake_translator = dir.join("fake-translator.sh");
    fs::write(
        &fake_translator,
        r#"#!/usr/bin/env bash
set -u
set -o pipefail
if [[ "${1:-}" == "count-invalid-patterns" ]]; then
  printf '0\n'
  exit 0
fi
printf 'Warning unsupported_member: static initializer\n' >&2
printf 'pub struct Broken {\n'
"#,
    )
    .unwrap();
    let mut permissions = fs::metadata(&fake_translator).unwrap().permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(&fake_translator, permissions).unwrap();

    let script = Path::new(env!("CARGO_MANIFEST_DIR")).join("scripts/translate-focused-package.sh");
    let output = Command::new(&script)
        .arg("--skip-resolver-bench")
        .arg("--no-batch-translate")
        .arg("--jobs")
        .arg("1")
        .arg("--primary")
        .arg(&primary)
        .arg("--out")
        .arg(&out)
        .env("NEW_JAVA2RUST_BIN", &fake_translator)
        .output()
        .expect("run focused package wrapper");
    assert!(
        !output.status.success(),
        "focused wrapper should fail the gate when generated Rust does not parse\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let summary = fs::read_to_string(out.join("summary.tsv")).unwrap();
    assert!(summary.contains("cargo_check\tfailed"), "{summary}");
    assert!(
        summary.contains("cargo_failure_kind\tgenerated_rust"),
        "{summary}"
    );
    assert!(summary.contains("cargo_type_errors_valid\t0"), "{summary}");
    assert!(summary.contains("unsupported_members\t1"), "{summary}");
    assert!(!summary.contains("parse_failures\t0"), "{summary}");
    let parse_failures = fs::read_to_string(out.join("parse-failures.tsv")).unwrap();
    assert!(
        parse_failures.contains("this file contains an unclosed delimiter"),
        "{parse_failures}"
    );
}

#[cfg(unix)]
#[test]
fn focused_package_missing_idiom_sweep_signal_fails_summary() {
    let dir = unique_temp_dir("focused_missing_idiom_signal");
    let primary = dir.join("java");
    let out = dir.join("out");
    fs::create_dir_all(&primary).unwrap();
    fs::write(
        primary.join("Clean.java"),
        r#"
public class Clean {
    public int value() {
        return 1;
    }
}
"#,
    )
    .unwrap();

    let fake_translator = dir.join("fake-translator.sh");
    fs::write(
        &fake_translator,
        r#"#!/usr/bin/env bash
set -u
set -o pipefail
if [[ "${1:-}" == "count-invalid-patterns" ]]; then
  printf '0\n'
  exit 0
fi
printf 'pub struct Clean;\n'
"#,
    )
    .unwrap();
    let mut translator_permissions = fs::metadata(&fake_translator).unwrap().permissions();
    translator_permissions.set_mode(0o755);
    fs::set_permissions(&fake_translator, translator_permissions).unwrap();

    let real_python = Command::new("which")
        .arg("python3")
        .output()
        .expect("locate python3");
    assert!(
        real_python.status.success(),
        "python3 is required for focused wrapper test"
    );
    let real_python = String::from_utf8(real_python.stdout)
        .unwrap()
        .trim()
        .to_string();

    let fake_bin = dir.join("bin");
    fs::create_dir_all(&fake_bin).unwrap();
    let fake_python = fake_bin.join("python3");
    fs::write(
        &fake_python,
        format!(
            r#"#!/usr/bin/env bash
set -euo pipefail
if [[ "${{1:-}}" == *"/scripts/sweep-generated-idioms.py" ]]; then
  out_tsv=""
  out_md=""
  while [[ "$#" -gt 0 ]]; do
    case "$1" in
      --out-tsv)
        out_tsv="$2"
        shift 2
        ;;
      --out-md)
        out_md="$2"
        shift 2
        ;;
      *)
        shift
        ;;
    esac
  done
  printf 'signal\tcount\tfiles\n' > "$out_tsv"
  printf 'unsupported_statements\t0\t0\n' >> "$out_tsv"
  printf 'unsupported_members\t0\t0\n' >> "$out_tsv"
  printf 'unsupported_expressions\t0\t0\n' >> "$out_tsv"
  printf 'unmodelled_types\t0\t0\n' >> "$out_tsv"
  printf 'unmodelled_call_sites\t0\t0\n' >> "$out_tsv"
  printf 'noop_facade_methods\t0\t0\n' >> "$out_tsv"
  printf 'noop_facade_call_sites\t0\t0\n' >> "$out_tsv"
  printf 'unmodelled_upcast_conversions\t0\t0\n' >> "$out_tsv"
  printf '# fake sweep\n' > "$out_md"
  exit 0
fi
exec "{}" "$@"
"#,
            real_python
        ),
    )
    .unwrap();
    let mut python_permissions = fs::metadata(&fake_python).unwrap().permissions();
    python_permissions.set_mode(0o755);
    fs::set_permissions(&fake_python, python_permissions).unwrap();

    let script = Path::new(env!("CARGO_MANIFEST_DIR")).join("scripts/translate-focused-package.sh");
    let output = Command::new(&script)
        .arg("--skip-resolver-bench")
        .arg("--no-batch-translate")
        .arg("--jobs")
        .arg("1")
        .arg("--primary")
        .arg(&primary)
        .arg("--out")
        .arg(&out)
        .env("NEW_JAVA2RUST_BIN", &fake_translator)
        .env(
            "PATH",
            format!(
                "{}:{}",
                fake_bin.display(),
                std::env::var("PATH").unwrap_or_default()
            ),
        )
        .output()
        .expect("run focused package wrapper");
    assert!(
        !output.status.success(),
        "focused wrapper should fail when the idiom sweep omits a required signal\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("missing idiom signal: stubbed_constants"),
        "stderr:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[cfg(unix)]
#[test]
fn focused_package_missing_analysis_profile_metric_fails_even_with_idiom_fallback() {
    let dir = unique_temp_dir("focused_missing_analysis_profile_metric");
    let primary = dir.join("java");
    let out = dir.join("out");
    fs::create_dir_all(&primary).unwrap();
    fs::write(
        primary.join("Clean.java"),
        r#"
public class Clean {
    public int value() {
        return 1;
    }
}
"#,
    )
    .unwrap();

    let fake_translator = dir.join("fake-translator.sh");
    fs::write(
        &fake_translator,
        r#"#!/usr/bin/env bash
set -euo pipefail
if [[ "${1:-}" == "translate-batch" ]]; then
  out_dir=""
  generated_dir=""
  phase_events=""
  worker_events=""
  primary=""
  while [[ "$#" -gt 0 ]]; do
    case "$1" in
      --out-dir)
        out_dir="$2"
        shift 2
        ;;
      --generated-dir)
        generated_dir="$2"
        shift 2
        ;;
      --phase-events)
        phase_events="$2"
        shift 2
        ;;
      --worker-events)
        worker_events="$2"
        shift 2
        ;;
      --primary)
        primary="$2"
        shift 2
        ;;
      *)
        shift
        ;;
    esac
  done
  mkdir -p "$generated_dir"
  printf 'source\trust_path\ttranslate_status\tdiagnostics\tinvalid_patterns\tunsupported_members\n' > "$out_dir/status.tsv"
  printf 'pub struct Clean;\n' > "$generated_dir/00001_Clean.rs"
  printf '%s/Clean.java\t%s/00001_Clean.rs\tok\t0\t0\t0\n' "$primary" "$generated_dir" >> "$out_dir/status.tsv"
  printf 'metric\tvalue\nprimary_files\t1\n' > "$out_dir/analysis-profile.tsv"
  printf 'batch_global_semantic\tok\t0\tfake\n' >> "$phase_events"
  printf 'batch_translate_end\t%s\t0\t%s\t0\tok\t%s\n' "$$" "$primary" "$generated_dir" >> "$worker_events"
  exit 0
fi
printf 'unexpected fake translator command: %s\n' "${1:-}" >&2
exit 1
"#,
    )
    .unwrap();
    let mut translator_permissions = fs::metadata(&fake_translator).unwrap().permissions();
    translator_permissions.set_mode(0o755);
    fs::set_permissions(&fake_translator, translator_permissions).unwrap();

    let real_python = Command::new("which")
        .arg("python3")
        .output()
        .expect("locate python3");
    assert!(
        real_python.status.success(),
        "python3 is required for focused wrapper test"
    );
    let real_python = String::from_utf8(real_python.stdout)
        .unwrap()
        .trim()
        .to_string();

    let fake_bin = dir.join("bin");
    fs::create_dir_all(&fake_bin).unwrap();
    let fake_python = fake_bin.join("python3");
    fs::write(
        &fake_python,
        format!(
            r#"#!/usr/bin/env bash
set -euo pipefail
if [[ "${{1:-}}" == *"/scripts/sweep-generated-idioms.py" ]]; then
  out_tsv=""
  out_md=""
  while [[ "$#" -gt 0 ]]; do
    case "$1" in
      --out-tsv)
        out_tsv="$2"
        shift 2
        ;;
      --out-md)
        out_md="$2"
        shift 2
        ;;
      *)
        shift
        ;;
    esac
  done
  printf 'signal\tcount\tfiles\n' > "$out_tsv"
  printf 'unsupported_statements\t0\t0\n' >> "$out_tsv"
  printf 'unsupported_members\t0\t0\n' >> "$out_tsv"
  printf 'unsupported_expressions\t0\t0\n' >> "$out_tsv"
  printf 'unmodelled_types\t0\t0\n' >> "$out_tsv"
  printf 'unmodelled_call_sites\t0\t0\n' >> "$out_tsv"
  printf 'noop_facade_methods\t0\t0\n' >> "$out_tsv"
  printf 'noop_facade_call_sites\t7\t1\n' >> "$out_tsv"
  printf 'unmodelled_upcast_conversions\t0\t0\n' >> "$out_tsv"
  printf 'stubbed_constants\t0\t0\n' >> "$out_tsv"
  printf '# fake sweep\n' > "$out_md"
  exit 0
fi
exec "{}" "$@"
"#,
            real_python
        ),
    )
    .unwrap();
    let mut python_permissions = fs::metadata(&fake_python).unwrap().permissions();
    python_permissions.set_mode(0o755);
    fs::set_permissions(&fake_python, python_permissions).unwrap();

    let script = Path::new(env!("CARGO_MANIFEST_DIR")).join("scripts/translate-focused-package.sh");
    let output = Command::new(&script)
        .arg("--skip-resolver-bench")
        .arg("--jobs")
        .arg("1")
        .arg("--primary")
        .arg(&primary)
        .arg("--out")
        .arg(&out)
        .env("NEW_JAVA2RUST_BIN", &fake_translator)
        .env(
            "PATH",
            format!(
                "{}:{}",
                fake_bin.display(),
                std::env::var("PATH").unwrap_or_default()
            ),
        )
        .output()
        .expect("run focused package wrapper");
    assert!(
        !output.status.success(),
        "focused wrapper should fail when analysis profile exists but omits a required metric\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        String::from_utf8_lossy(&output.stderr)
            .contains("missing analysis profile metric: noop_facade_call_sites"),
        "stderr:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn focused_package_wrapper_preserves_primary_source_types_covered_by_external_pack() {
    let dir = unique_temp_dir("focused_primary_pack_overlap");
    let primary = dir.join("java");
    let out = dir.join("out");
    fs::create_dir_all(&primary).unwrap();
    fs::write(
        primary.join("ModeledPrimary.java"),
        r#"
package modeled;

public class ModeledPrimary {
    public enum Kind {
        ONE,
        TWO
    }

    public int value() {
        return 1;
    }

    public ModeledPeer peer() {
        return new ModeledPeer();
    }
}
"#,
    )
    .unwrap();
    fs::write(
        primary.join("ModeledPeer.java"),
        r#"
package modeled;

public class ModeledPeer {
    public int two() {
        return 2;
    }

    public int kinds() {
        return ModeledPrimary.Kind.values().length;
    }
}
"#,
    )
    .unwrap();
    let manifest = dir.join("pack.txt");
    fs::write(
        &manifest,
        "external-pack modeled ModeledPrimary semantic\n\
         external-pack modeled ModeledPeer semantic\n\
         external-pack-object modeled ModeledPrimary erased-object\n\
         external-pack-object modeled ModeledPeer erased-object\n",
    )
    .unwrap();

    let script = Path::new(env!("CARGO_MANIFEST_DIR")).join("scripts/translate-focused-package.sh");
    let mut command = Command::new(&script);
    command
        .arg("--skip-resolver-bench")
        .arg("--jobs")
        .arg("2")
        .arg("--primary")
        .arg(&primary)
        .arg("--out")
        .arg(&out)
        .arg("--external-pack-manifest")
        .arg(&manifest);
    if let Ok(binary) = std::env::var("CARGO_BIN_EXE_new-java2rust") {
        command.env("NEW_JAVA2RUST_BIN", binary);
    }
    let output = command.output().expect("run focused package wrapper");
    assert!(
        output.status.success(),
        "focused wrapper failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let mut generated = String::new();
    for entry in fs::read_dir(out.join("generated-crate/src/generated")).unwrap() {
        let entry = entry.unwrap();
        if entry.path().extension().and_then(|ext| ext.to_str()) == Some("rs") {
            generated.push_str(&fs::read_to_string(entry.path()).unwrap());
        }
    }
    assert!(
        generated.contains("pub struct ModeledPrimary"),
        "{generated}"
    );
    assert!(
        generated.contains("pub enum ModeledPrimary_Kind"),
        "{generated}"
    );
    assert!(
        generated.contains("ModeledPrimary_Kind::values()"),
        "{generated}"
    );
    assert!(
        !generated.contains("pub struct ModeledPrimaryKind;"),
        "{generated}"
    );
    assert!(generated.contains("pub struct ModeledPeer"), "{generated}");
}

/// One unparseable file in a batch must not mark its clean siblings `errors`.
///
/// Batch mode pooled every *primary* file's parse diagnostics into a batch-wide
/// bag and then appended that bag to each file's own diagnostics. Every file
/// therefore carried every other file's parse errors, so a single bad file made
/// the whole batch read `errors`, and the bad file was counted twice -- its own
/// diagnostics reach it directly through `parsed.diagnostics` as well. The
/// column had gone from constant-`ok` (the original 0055 finding) to
/// effectively constant-`errors`, which is still not a measurement.
///
/// The bag is for context and summary sources, which genuinely are shared by
/// every file; the single-file path always kept that invariant.
///
/// See claude_audits/0055-batch-translation-status-is-not-measured.md.
#[test]
fn translate_batch_keeps_parse_diagnostics_with_the_file_that_caused_them() {
    let dir = unique_temp_dir("translate_batch_diagnostic_isolation");
    let source_dir = dir.join("java");
    let out_dir = dir.join("out");
    let generated_dir = out_dir.join("generated");
    fs::create_dir_all(&source_dir).unwrap();
    fs::create_dir_all(&generated_dir).unwrap();
    fs::write(
        source_dir.join("CleanOne.java"),
        "public class CleanOne { public static int run() { return 7; } }\n",
    )
    .unwrap();
    fs::write(
        source_dir.join("CleanTwo.java"),
        "public class CleanTwo { public static int run() { return 8; } }\n",
    )
    .unwrap();
    fs::write(
        source_dir.join("Unparseable.java"),
        "public class Unparseable { public static int run() { return ((( } }\n",
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_new-java2rust"))
        .arg("translate-batch")
        .arg("--primary")
        .arg(&source_dir)
        .arg("--out-dir")
        .arg(&out_dir)
        .arg("--generated-dir")
        .arg(&generated_dir)
        .output()
        .expect("run translate-batch");
    assert!(
        output.status.success(),
        "translate-batch failed\nstderr:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );

    let status = fs::read_to_string(out_dir.join("status.tsv")).unwrap();
    let mut seen = 0;
    for line in status.lines().skip(1) {
        let fields = line.split('\t').collect::<Vec<_>>();
        let (source, translate_status, diagnostics) = (fields[0], fields[2], fields[3]);
        seen += 1;
        if source.ends_with("Unparseable.java") {
            assert_eq!(translate_status, "errors", "{status}");
            // Exactly once -- not once from its own parse and again from the bag.
            assert_eq!(diagnostics, "1", "{status}");
        } else {
            assert_eq!(
                translate_status, "ok",
                "a clean file inherited another file's parse errors\n{status}"
            );
            assert_eq!(diagnostics, "0", "{status}");
        }
    }
    assert_eq!(seen, 3, "{status}");
}
