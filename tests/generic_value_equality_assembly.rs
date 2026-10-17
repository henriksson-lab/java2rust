#[path = "common/mod.rs"]
mod common;

fn java_oracle(sources: &[new_java2rust::BatchSource]) -> Option<i32> {
    use std::{
        fs,
        process::Command,
        time::{SystemTime, UNIX_EPOCH},
    };
    for tool in ["javac", "java"] {
        if Command::new(tool).arg("-version").output().is_err() {
            eprintln!("Java equality oracle skipped: {tool} unavailable");
            return None;
        }
    }
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let directory = std::env::temp_dir().join(format!("new_java2rust_equality_oracle_{unique}"));
    let classes = directory.join("classes");
    fs::create_dir_all(&classes).unwrap();
    let mut paths = Vec::new();
    for source in sources {
        let path = directory.join(source.path());
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, source.source()).unwrap();
        paths.push(path);
    }
    let probe = directory.join("EqualityOracleProbe.java");
    fs::write(&probe, "public class EqualityOracleProbe { public static void main(String[] args) { System.out.println(example.client.Use.run()); } }").unwrap();
    paths.push(probe);
    let compiled = Command::new("javac")
        .arg("-d")
        .arg(&classes)
        .args(&paths)
        .output()
        .unwrap();
    assert!(
        compiled.status.success(),
        "{}",
        String::from_utf8_lossy(&compiled.stderr)
    );
    let executed = Command::new("java")
        .arg("-cp")
        .arg(&classes)
        .arg("EqualityOracleProbe")
        .output()
        .unwrap();
    assert!(
        executed.status.success(),
        "{}",
        String::from_utf8_lossy(&executed.stderr)
    );
    let value = String::from_utf8(executed.stdout)
        .unwrap()
        .trim()
        .parse()
        .unwrap();
    fs::remove_dir_all(&directory).unwrap();
    Some(value)
}

#[test]
fn generic_equality_and_null_frames_execute_across_production_modules() {
    use new_java2rust::BatchSource;
    let sources = [
        BatchSource::new(
            "example/api/EqualityOps.java",
            r#"
package example.api;
public class EqualityOps {
    public static <T> boolean same(T left, T right) { return left.equals(right); }
    public static <T> boolean different(T left, T right) { return !left.equals(right); }
}
"#,
        ),
        BatchSource::new(
            "example/api/Equality.java",
            r#"
package example.api;
public interface Equality<Unused, T> {
    default boolean same(T left, T right) { return left.equals(right); }
}
"#,
        ),
        BatchSource::new(
            "example/impl/Words.java",
            "package example.impl; public class Words implements example.api.Equality<Integer, String> {}",
        ),
        BatchSource::new(
            "example/client/Use.java",
            r#"
package example.client;
import example.api.Equality;
import example.api.EqualityOps;
import example.impl.Words;
public class Use {
    public static int run() {
        int result = 0;
        if (EqualityOps.same("same", "same")) result += 1;
        if (!EqualityOps.same("same", null)) result += 2;
        if (EqualityOps.different("left", "right")) result += 4;
        Equality<Integer, String> words = new Words();
        if (words.same("value", "value")) result += 8;
        return result;
    }
}
"#,
        ),
    ];
    let expected = java_oracle(&sources).unwrap_or(15);
    assert_eq!(expected, 15);
    common::cargo_run_translated_batch_sources(
        "generic_value_equality_assembly",
        &sources,
        &format!("fn main() {{ assert_eq!(translated::Use::run(), {expected}); }}"),
    );
}
