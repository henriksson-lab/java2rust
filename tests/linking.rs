//! Integration tests for the dependency-linking pipeline (`--link`).
//!
//! These lock the consumer-side behaviors that kick in when a previously-
//! translated dependency's symbol map is supplied: type-path resolution,
//! call-site shaping (exact Rust name, argument borrowing, nullable-return
//! unwrap), and caller signature/`let mut` upgrades for `&mut self` receivers.

use java2rust_rs::symbol_map::LinkIndex;
use java2rust_rs::{convert, convert_with_links};

/// A dependency map for `org.lib.Store`, as `gen-symbols` would emit it after an
/// LLM polished the translated crate: `lookup` -> `find` returning
/// `Option<String>`, `register` -> `add` taking `&mut self`.
const STORE_MAP: &str = r#"
{
  "types": {
    "org.lib.Store": {
      "rust_path": "store::Store",
      "kind": "struct",
      "fields": {},
      "methods": {
        "lookup": {
          "rust": "find", "rust_path": "store::Store::find",
          "receiver": "ref", "ret": "Option<String>", "ret_nullable": true,
          "params": [{ "type": "&String", "by_ref": true, "mutable": false, "nullable": false }]
        },
        "register": {
          "rust": "add", "rust_path": "store::Store::add",
          "receiver": "refmut", "ret": null, "ret_nullable": false,
          "params": [{ "type": "&String", "by_ref": true, "mutable": false, "nullable": false }]
        }
      }
    }
  }
}
"#;

fn store_link() -> LinkIndex {
    let mut link = LinkIndex::default();
    link.merge_json(STORE_MAP).expect("parse STORE_MAP");
    link
}

const CLIENT: &str = r#"
package org.app;
import org.lib.Store;
public class Client {
    public String run(Store s, String k, String n) {
        s.register(n);
        String v = s.lookup(k);
        return v;
    }
}
"#;

#[test]
fn without_link_uses_bare_names() {
    let out = convert(CLIENT);
    assert!(out.contains("s: &Store"), "bare borrowed type:\n{out}");
    assert!(out.contains("s.register("), "Java method name kept:\n{out}");
    assert!(out.contains("s.lookup("), "Java method name kept:\n{out}");
    assert!(!out.contains("store::Store"), "no dep path without link:\n{out}");
    assert!(!out.contains(".unwrap()"), "no nullable knowledge without link:\n{out}");
}

#[test]
fn with_link_resolves_type_paths() {
    let out = convert_with_links(CLIENT, &store_link());
    assert!(out.contains("store::Store"), "type resolved to dep path:\n{out}");
}

#[test]
fn with_link_uses_current_rust_method_names() {
    let out = convert_with_links(CLIENT, &store_link());
    assert!(out.contains("s.add("), "renamed register->add:\n{out}");
    assert!(out.contains("s.find("), "renamed lookup->find:\n{out}");
    assert!(!out.contains("s.register("), "old name gone:\n{out}");
    assert!(!out.contains("s.lookup("), "old name gone:\n{out}");
}

#[test]
fn with_link_unwraps_nullable_return() {
    let out = convert_with_links(CLIENT, &store_link());
    assert!(out.contains("s.find(&k).unwrap()"), "Option return unwrapped:\n{out}");
}

#[test]
fn with_link_upgrades_param_to_mut_borrow() {
    // `s.register(..)` is a refmut call, so the `Store` parameter must be `&mut`.
    let out = convert_with_links(CLIENT, &store_link());
    assert!(out.contains("s: &mut store::Store"), "param upgraded to &mut:\n{out}");
}

#[test]
fn linked_method_wins_over_builtin_collection_rewrite() {
    // A linked type with methods whose names collide with the built-in stdlib
    // rewrites (`put` -> `.insert`, `size`/`length` -> `.len()`) must emit the
    // linked names, not the heuristic rewrites.
    let map = r#"
    {
      "types": {
        "org.json.JSONObject": {
          "rust_path": "json::JSONObject", "kind": "struct", "fields": {},
          "methods": {
            "put": { "rust": "put", "rust_path": "json::JSONObject::put",
              "receiver": "ref", "ret": null, "ret_nullable": false,
              "params": [
                { "type": "&String", "by_ref": true, "mutable": false, "nullable": false },
                { "type": "bool", "by_ref": false, "mutable": false, "nullable": false }] },
            "length": { "rust": "length", "rust_path": "json::JSONObject::length",
              "receiver": "ref", "ret": "i32", "ret_nullable": false, "params": [] }
          }
        }
      }
    }"#;
    let mut link = LinkIndex::default();
    link.merge_json(map).unwrap();

    let java = r#"
package org.app;
import org.json.JSONObject;
public class R {
    public int f(JSONObject o, String k) {
        o.put(k, true);
        return o.length();
    }
}
"#;
    let out = convert_with_links(java, &link);
    assert!(out.contains("o.put(&k, true)"), "linked put, not .insert:\n{out}");
    assert!(out.contains("o.length()"), "linked length, not .len():\n{out}");
    assert!(!out.contains(".insert("), "no collection rewrite:\n{out}");
    assert!(!out.contains(".len()"), "no length->len rewrite:\n{out}");
}

const MAKER: &str = r#"
package org.app;
import org.lib.Store;
public class Maker {
    public void build() {
        Store s = new Store();
        s.register("hello");
    }
}
"#;

#[test]
fn with_link_marks_local_let_mut() {
    let out = convert_with_links(MAKER, &store_link());
    assert!(out.contains("let mut s"), "local needs let mut for refmut call:\n{out}");
    assert!(out.contains("store::Store::new()"), "constructor resolved:\n{out}");
}

#[test]
fn without_link_local_is_immutable() {
    let out = convert(MAKER);
    assert!(out.contains("let s"), "{out}");
    assert!(!out.contains("let mut s"), "no mut without link knowledge:\n{out}");
}

#[test]
fn unknown_types_fall_back_to_stdlib_mapping() {
    // A type not in the map must still get the built-in mapping (List -> Vec).
    let java = r#"
package org.app;
import java.util.List;
public class C { public List<String> xs; }
"#;
    let out = convert_with_links(java, &store_link());
    assert!(out.contains("Vec<"), "stdlib mapping still applies:\n{out}");
    assert!(!out.contains("store::"), "{out}");
}
