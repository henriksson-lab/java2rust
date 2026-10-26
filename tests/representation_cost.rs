use new_java2rust::representation_cost::{CostWeights, analyze_ir, analyze_rust};

fn count(source: &str, name: &str) -> u64 {
    analyze_rust(source)
        .unwrap()
        .metrics()
        .get(name)
        .copied()
        .unwrap_or(0)
}

#[test]
fn synchronization_and_atomic_ownership_have_explicit_high_costs() {
    let weights = CostWeights::default();
    let simple = analyze_rust("fn f(value: &i32) -> i32 { *value }").unwrap();
    let rc = analyze_rust("fn f(value: std::rc::Rc<i32>) { let copy = value.clone(); }").unwrap();
    let arc = analyze_rust("fn f(value: std::sync::Arc<std::sync::Mutex<i32>>) { let copy = value.clone(); let guard = copy.lock().unwrap(); }").unwrap();
    assert!(simple.score(&weights) < rc.score(&weights));
    assert!(rc.score(&weights) < arc.score(&weights));
    assert_eq!(arc.metrics()["type.arc"], 1);
    assert_eq!(arc.metrics()["type.mutex"], 1);
    assert_eq!(arc.metrics()["op.arc_clone"], 1);
    assert_eq!(arc.metrics()["op.lock"], 1);
    let expected = ["type.arc", "type.mutex", "op.arc_clone", "op.lock"]
        .iter()
        .map(|metric| u128::from(weights.0[*metric]))
        .sum::<u128>();
    assert_eq!(arc.score(&weights), expected);
}

#[test]
fn aliases_wrappers_and_qualified_clone_keep_the_backend_visible() {
    let source = "use std::sync::{Arc as Shared, Mutex}; type Handle<T> = Shared<Mutex<T>>; struct Ref(Handle<i32>); fn f(value: Handle<i32>) { let x = <std::sync::Arc<_> as std::clone::Clone>::clone(&value); }";
    assert_eq!(count(source, "type.arc"), 2);
    assert_eq!(count(source, "type.mutex"), 2);
    assert_eq!(count(source, "op.arc_clone"), 1);
    assert_eq!(
        count(
            "struct Arc; impl Arc { fn new() -> Arc { Arc } } fn f() { Arc::new(); }",
            "op.arc_allocate"
        ),
        0
    );
}

#[test]
fn comments_literals_macros_and_loop_sites_are_handled_deliberately() {
    let source = r##"
// Arc::new(Mutex::new(1)).lock();
fn f() {
    let message = r#"std::sync::Arc::new(1)"#;
    for i in 0..10 {
        let values = vec![std::sync::Arc::new(std::sync::Mutex::new(i))];
    }
}
"##;
    let report = analyze_rust(source).unwrap();
    assert_eq!(report.metrics()["op.arc_allocate"], 1);
    assert_eq!(report.metrics()["op.mutex_create"], 1);
    assert_eq!(report.metrics()["op.vec_allocate"], 1);
    assert!(
        report
            .sites
            .iter()
            .filter(|site| site.metric == "op.arc_allocate")
            .all(|site| site.loop_depth == 1)
    );
    assert!(!report.coverage.contains_key("opaque_macro_invocations"));
}

#[test]
fn helper_movement_does_not_erase_cost_and_raw_ir_is_audited() {
    let inline = analyze_rust("fn f(){ std::sync::Arc::new(1); }").unwrap();
    let outlined =
        analyze_rust("fn helper(){ std::sync::Arc::new(1); } fn f(){ helper(); }").unwrap();
    assert_eq!(
        inline.score(&CostWeights::default()),
        outlined.score(&CostWeights::default())
    );
    let ir = new_java2rust::rust_ir::RustFile {
        items: vec![new_java2rust::rust_ir::RustItem::Raw(
            "fn helper(){std::sync::Mutex::new(1);}".into(),
        )],
    };
    assert_eq!(analyze_ir(&ir).unwrap().metrics()["op.mutex_create"], 1);
}

#[test]
fn weights_diffs_and_coverage_are_reported_without_claiming_correctness() {
    let previous = analyze_rust("fn f(value:i32) {}").unwrap();
    let current = analyze_rust("fn f<T:Clone>(value:T){value.clone(); unknown!(std::sync::Arc::new(1)); compile_error!(\"x\");}").unwrap();
    assert_eq!(current.metrics()["op.unresolved_clone"], 1);
    assert!(current.coverage["opaque_macro_invocations"] > 0);
    assert_eq!(current.coverage["compile_error_markers"], 1);
    let weights = CostWeights::default()
        .with_overrides("op.unresolved_clone=1000\n# comment")
        .unwrap();
    let rendered = current.render(&weights, Some(&previous), true);
    assert!(rendered.contains("correctness=not-evaluated"));
    assert!(rendered.contains("delta=1000"));
    assert!(rendered.contains("# site\t"));
    assert!(
        CostWeights::default()
            .with_overrides("type.unknown=10")
            .is_err()
    );
    assert!(
        CostWeights::default()
            .with_overrides("type.arc=-1")
            .is_err()
    );
    assert!(analyze_rust("fn broken( {").unwrap().coverage["parse_errors"] > 0);
}

#[test]
fn generic_substitution_and_deref_lock_receivers_are_visible() {
    let source = "type Identity<T> = T; struct Wrapper<T>(T); fn f(x: Identity<std::sync::Arc<std::sync::RwLock<i32>>>, y: Wrapper<std::rc::Rc<std::cell::RefCell<i32>>>) { x.read(); x.write(); y.borrow_mut(); }";
    let report = analyze_rust(&source).unwrap();
    assert!(!report.coverage.contains_key("parse_errors"));
    assert_eq!(report.metrics()["type.arc"], 1);
    assert_eq!(report.metrics()["type.rwlock"], 1);
    assert_eq!(report.metrics()["type.refcell"], 1);
    assert_eq!(report.metrics()["op.read_lock"], 1);
    assert_eq!(report.metrics()["op.write_lock"], 1);
    assert_eq!(report.metrics()["op.borrow_mut"], 1);
}

#[test]
fn cli_reports_directory_baseline_overrides_and_rejects_bad_arguments() {
    use std::{fs, process::Command};
    let root = std::env::temp_dir().join(format!("java2rust-cost-test-{}", std::process::id()));
    fs::create_dir_all(root.join("src/target")).unwrap();
    fs::write(
        root.join("src/lib.rs"),
        "fn f(x: std::sync::Arc<i32>) { x.clone(); }",
    )
    .unwrap();
    fs::write(
        root.join("src/target/ignored.rs"),
        "fn f(){std::sync::Mutex::new(1);}",
    )
    .unwrap();
    fs::write(root.join("old.rs"), "fn f(x: &i32) {}").unwrap();
    fs::write(root.join("weights.txt"), "type.arc=100\nop.arc_clone=50").unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_new-java2rust"))
        .arg("representation-cost")
        .arg(root.join("src"))
        .arg("--baseline")
        .arg(root.join("old.rs"))
        .arg("--weights")
        .arg(root.join("weights.txt"))
        .arg("--sites")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let text = String::from_utf8(output.stdout).unwrap();
    assert!(text.contains("# total=150\n"));
    assert!(text.contains("baseline_total=1; delta=149"));
    assert!(text.contains("lib.rs::crate::f"));
    assert!(!text.contains("op.mutex_create"));
    for args in [
        vec!["representation-cost", "--sites"],
        vec!["representation-cost", "-", "--baseline", "-"],
        vec!["representation-cost", "-", "--sites", "--sites"],
    ] {
        assert_eq!(
            Command::new(env!("CARGO_BIN_EXE_new-java2rust"))
                .args(args)
                .output()
                .unwrap()
                .status
                .code(),
            Some(2)
        );
    }
    fs::remove_dir_all(root).unwrap();
}
