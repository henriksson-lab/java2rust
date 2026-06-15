//! Integration test for crate/module wiring (`--crate`).
//!
//! Builds a small multi-package project on disk, then checks:
//! 1. the project self-map resolves cross-file references to `crate::…` paths;
//! 2. `finish_crate` emits a `lib.rs` + `mod.rs` tree and a `Cargo.toml`.

use std::fs;
use std::path::PathBuf;

use java2rust_rs::crate_layout::{build_project_map, finish_crate};
use java2rust_rs::symbol_map::LinkIndex;
use java2rust_rs::convert_with_links;

fn tmp(sub: &str) -> PathBuf {
    let d = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(sub);
    let _ = fs::remove_dir_all(&d);
    fs::create_dir_all(&d).unwrap();
    d
}

#[test]
fn project_map_resolves_cross_file_refs_to_crate_paths() {
    let src = tmp("cw_src/com/ex/model");
    fs::write(
        src.join("Point.java"),
        "package com.ex.model;\npublic class Point { public int getX() { return 0; } }\n",
    )
    .unwrap();

    let root = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("cw_src");
    let mut link = LinkIndex::default();
    link.merge(build_project_map(&root));

    let consumer = r#"
package com.ex.util;
import com.ex.model.Point;
public class Maker {
    public Point make(Point p) { return p; }
}
"#;
    let out = convert_with_links(consumer, &link);
    // Cross-file type resolves to its crate module path (input basename is the
    // first segment: `cw_src`).
    assert!(
        out.contains("crate::cw_src::com::ex::model::point::Point"),
        "cross-file ref resolved to crate path:\n{out}"
    );
}

#[test]
fn static_calls_are_crate_qualified() {
    let src = tmp("cw_static/com/ex/util");
    fs::write(
        src.join("Helper.java"),
        "package com.ex.util;\npublic class Helper { public static int twice(int x) { return x; } }\n",
    )
    .unwrap();
    let root = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("cw_static");
    let mut link = LinkIndex::default();
    link.merge(build_project_map(&root));

    let consumer = r#"
package com.ex;
import com.ex.util.Helper;
public class App {
    public int run() { return Helper.twice(3); }
}
"#;
    let out = convert_with_links(consumer, &link);
    assert!(
        out.contains("crate::cw_static::com::ex::util::helper::Helper::twice(3)"),
        "static call crate-qualified:\n{out}"
    );
}

#[test]
fn finish_crate_emits_module_tree_and_cargo() {
    let root = tmp("cw_out/pkg/sub");
    fs::write(root.join("thing.rs"), "pub struct Thing {}\n").unwrap();
    let out_root = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("cw_out");

    finish_crate(&out_root).unwrap();

    let lib = fs::read_to_string(out_root.join("lib.rs")).unwrap();
    assert!(lib.contains("pub mod pkg;"), "lib.rs declares top module:\n{lib}");
    let cargo = fs::read_to_string(out_root.join("Cargo.toml")).unwrap();
    assert!(cargo.contains("[lib]") && cargo.contains("path = \"lib.rs\""), "Cargo.toml:\n{cargo}");
    let modrs = fs::read_to_string(out_root.join("pkg/mod.rs")).unwrap();
    assert!(modrs.contains("pub mod sub;"), "pkg/mod.rs declares child:\n{modrs}");
    let subrs = fs::read_to_string(out_root.join("pkg/sub/mod.rs")).unwrap();
    assert!(subrs.contains("pub mod thing;"), "sub/mod.rs declares file module:\n{subrs}");
}
