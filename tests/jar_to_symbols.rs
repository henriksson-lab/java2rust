//! Integration test for the `jar-to-symbols` binary.
//!
//! Runs it against a committed fixture JAR (`tests/fixtures/demo.jar`, built from
//! an annotated `org.demo.Store` that references the not-included
//! `org.external.Widget`) and checks: signatures, static-ness, nullability from
//! `@Nullable` annotations, private-member skipping, and the missing-types
//! warning. Then feeds the resulting map through `convert_with_links` to confirm
//! it drives precise translation.

use std::process::Command;

use java2rust_rs::symbol_map::LinkIndex;
use java2rust_rs::convert_with_links;

fn run_jar_to_symbols() -> (String, String) {
    let jar = format!("{}/tests/fixtures/demo.jar", env!("CARGO_MANIFEST_DIR"));
    let out = Command::new(env!("CARGO_BIN_EXE_jar-to-symbols"))
        .arg(&jar)
        .output()
        .expect("run jar-to-symbols");
    (
        String::from_utf8_lossy(&out.stdout).into_owned(),
        String::from_utf8_lossy(&out.stderr).into_owned(),
    )
}

#[test]
fn extracts_signatures_and_nullability() {
    let (json, _) = run_jar_to_symbols();
    // Type path from the jar package.
    assert!(json.contains("\"org.demo.Store\""), "{json}");
    assert!(json.contains("\"rust_path\": \"org::demo::Store\""), "{json}");
    // Nullability lifted from the @Nullable annotation in the bytecode.
    assert!(json.contains("\"lookup\""), "{json}");
    assert!(json.contains("\"ret_nullable\": true"), "method @Nullable -> ret_nullable:\n{json}");
    // Static method has no receiver; a constructor maps to `new`.
    assert!(json.contains("\"create\""), "{json}");
    assert!(json.contains("\"new\""), "{json}");
    // Annotation interface and private members are excluded.
    assert!(!json.contains("org.demo.Nullable"), "annotation type excluded:\n{json}");
    assert!(!json.contains("\"secret\""), "private method excluded:\n{json}");
}

#[test]
fn warns_about_uncovered_types() {
    let (_, stderr) = run_jar_to_symbols();
    assert!(stderr.contains("WARNING"), "missing-type warning present:\n{stderr}");
    assert!(stderr.contains("org.external"), "names the uncovered package:\n{stderr}");
    // JDK types referenced (String, List) must NOT be reported as missing.
    assert!(!stderr.contains("java.lang"), "JDK not flagged:\n{stderr}");
    assert!(!stderr.contains("java.util"), "JDK not flagged:\n{stderr}");
}

#[test]
fn map_drives_precise_translation() {
    let (json, _) = run_jar_to_symbols();
    let mut link = LinkIndex::default();
    link.merge_json(&json).expect("parse jar map");

    let client = r#"
package org.app;
import org.demo.Store;
public class Client {
    public String run(Store s, String k) {
        s.register(k, 2);
        String v = s.lookup(k);
        return v;
    }
}
"#;
    let out = convert_with_links(client, &link);
    assert!(out.contains("org::demo::Store"), "type path linked:\n{out}");
    // @Nullable return -> .unwrap() at the read.
    assert!(out.contains("s.lookup(&k).unwrap()"), "nullable return unwrapped:\n{out}");
    // by_ref String param, by-value int param.
    assert!(out.contains("s.register(&k, 2)"), "param borrowing:\n{out}");
}
