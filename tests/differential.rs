//! Differential tests: run the Java and run the translation, compare output.
//!
//! Compile errors only catch idioms that fail loudly. The defects that cost the
//! most — arithmetic that wraps the wrong way, a `StringBuilder` that silently
//! returns `""`, a `static` block that never runs, `Integer.MAX_VALUE` emitted
//! as `0` — all produce Rust that compiles cleanly and computes the wrong
//! answer. Nothing but running both sides finds those.
//!
//! Each fixture in `tests/differential/` is a self-contained class exposing
//! `public static String run()` and a `main` that prints it. Java's stdout is
//! the oracle; the translation must produce the same bytes.
//!
//! Requires `javac`, `java` and `rustc` on PATH. Missing prerequisites are
//! failures, because a skipped behavioural oracle is indistinguishable from a
//! passing one in the gate.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::process::Command;

use new_java2rust::translate_source;

const EXPECTED_DIFFERENTIAL_FIXTURES: usize = 270;
const EXPECTED_KNOWN_GAP_FIXTURES: usize = 17;

#[test]
fn translated_output_matches_java_output() {
    let translator = translator_binary().unwrap_or_else(|| {
        panic!("translator binary not built; run `cargo build --release` before differential tests")
    });
    for tool in ["javac", "java", "rustc"] {
        assert!(
            have_tool(tool),
            "required differential test tool `{tool}` is not available"
        );
    }

    let fixtures = fixture_paths();
    assert_eq!(
        fixtures.len(),
        EXPECTED_DIFFERENTIAL_FIXTURES,
        "differential fixture count changed; update EXPECTED_DIFFERENTIAL_FIXTURES intentionally"
    );

    let mut failures = Vec::new();
    for fixture in &fixtures {
        let class = fixture
            .file_stem()
            .and_then(|stem| stem.to_str())
            .expect("fixture has a class name");
        match compare_fixture(&translator, fixture, class) {
            Ok(()) => {}
            Err(report) => failures.push(format!("{class}: {report}")),
        }
    }

    assert!(
        failures.is_empty(),
        "translated output differs from Java for {} of {} fixtures:\n\n{}",
        failures.len(),
        fixtures.len(),
        failures.join("\n\n")
    );
}

#[test]
fn erased_display_fixtures_match_java_output() {
    let translator = translator_binary().unwrap_or_else(|| {
        panic!("translator binary not built; run `cargo build --release` before differential tests")
    });
    for tool in ["javac", "java", "rustc"] {
        assert!(
            have_tool(tool),
            "required differential test tool `{tool}` is not available"
        );
    }

    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/differential");
    let fixtures = [
        "ErasedAtomicBooleanDisplay",
        "ErasedAtomicIntegerDisplay",
        "ErasedAtomicLongDisplay",
        "ErasedArrayListDisplay",
        "ErasedBoxedPrimitiveDisplay",
        "ErasedBitSetDisplay",
        "ErasedClassDisplay",
        "ErasedFileDisplay",
        "ErasedInputStreamDisplay",
        "ErasedLinkedHashMapDisplay",
        "ErasedLinkedHashSetDisplay",
        "ErasedLocaleDisplay",
        "ErasedMapEntryDisplay",
        "ErasedNullableArrayListDisplay",
        "ErasedNullableBooleanArrayListDisplay",
        "ErasedNullableByteArrayListDisplay",
        "ErasedNullableCharacterArrayListDisplay",
        "ErasedNullableDoubleArrayListDisplay",
        "ErasedNullableFloatArrayListDisplay",
        "ErasedNullableIntegerArrayListDisplay",
        "ErasedNullableLongArrayListDisplay",
        "ErasedNullableShortArrayListDisplay",
        "ErasedPathDisplay",
        "ErasedPatternDisplay",
        "ErasedPlatformDisplay",
        "ErasedRandomDisplay",
        "ErasedSimpleImmutableEntryDisplay",
        "ErasedStringBuilderDisplay",
        "ErasedUriDisplay",
        "ErasedUrlDisplay",
    ];
    let mut failures = Vec::new();
    for class in fixtures {
        let fixture = root.join(format!("{class}.java"));
        match compare_fixture(&translator, &fixture, class) {
            Ok(()) => {}
            Err(report) => failures.push(format!("{class}: {report}")),
        }
    }

    assert!(
        failures.is_empty(),
        "erased display fixture output differs from Java:\n\n{}",
        failures.join("\n\n")
    );
}

#[test]
fn nullable_map_get_fixtures_match_java_output() {
    let translator = translator_binary().unwrap_or_else(|| {
        panic!("translator binary not built; run `cargo build --release` before differential tests")
    });
    for tool in ["javac", "java", "rustc"] {
        assert!(
            have_tool(tool),
            "required differential test tool `{tool}` is not available"
        );
    }

    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/differential");
    let fixtures = ["MapGetNull2", "MapPutNullValue"];
    let mut failures = Vec::new();
    for class in fixtures {
        let fixture = root.join(format!("{class}.java"));
        match compare_fixture(&translator, &fixture, class) {
            Ok(()) => {}
            Err(report) => failures.push(format!("{class}: {report}")),
        }
    }

    assert!(
        failures.is_empty(),
        "nullable Map.get fixture output differs from Java:\n\n{}",
        failures.join("\n\n")
    );
}

#[test]
fn null_representation_fixtures_match_java_output() {
    let translator = translator_binary().unwrap_or_else(|| {
        panic!("translator binary not built; run `cargo build --release` before differential tests")
    });
    for tool in ["javac", "java", "rustc"] {
        assert!(
            have_tool(tool),
            "required differential test tool `{tool}` is not available"
        );
    }

    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/differential");
    let fixtures = [
        "NullBasics",
        "NullFieldDefault",
        "NullObjectReturn",
        "NullCtorArg",
        "NullArgLiteral",
        "NullArgCatch",
        "NullCallArgCatch",
        "NullDeref",
        "NullDefaultReturn",
        "NullFieldCtorOverload",
        "NullInList",
        "NullInstanceof",
        "NullConcat",
        "NullLiteralConcat",
        "NullStringArray",
        "NullableArrayLengthCatch",
        "NullableFieldReceiverCatch",
    ];
    let mut failures = Vec::new();
    for class in fixtures {
        let fixture = root.join(format!("{class}.java"));
        match compare_fixture(&translator, &fixture, class) {
            Ok(()) => {}
            Err(report) => failures.push(format!("{class}: {report}")),
        }
    }

    assert!(
        failures.is_empty(),
        "null-representation fixture output differs from Java:\n\n{}",
        failures.join("\n\n")
    );
}

#[test]
fn instanceof_pattern_expression_fixtures_match_java_output() {
    let translator = translator_binary().unwrap_or_else(|| {
        panic!("translator binary not built; run `cargo build --release` before differential tests")
    });
    for tool in ["javac", "java", "rustc"] {
        assert!(
            have_tool(tool),
            "required differential test tool `{tool}` is not available"
        );
    }

    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/differential");
    let fixtures = ["InstanceofPatternExpression"];
    let mut failures = Vec::new();
    for class in fixtures {
        let fixture = root.join(format!("{class}.java"));
        match compare_fixture(&translator, &fixture, class) {
            Ok(()) => {}
            Err(report) => failures.push(format!("{class}: {report}")),
        }
    }

    assert!(
        failures.is_empty(),
        "instanceof-pattern expression fixture output differs from Java:\n\n{}",
        failures.join("\n\n")
    );
}

#[test]
fn record_canonical_constructor_fixtures_match_java_output() {
    let translator = translator_binary().unwrap_or_else(|| {
        panic!("translator binary not built; run `cargo build --release` before differential tests")
    });
    for tool in ["javac", "java", "rustc"] {
        assert!(
            have_tool(tool),
            "required differential test tool `{tool}` is not available"
        );
    }

    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/differential");
    let fixtures = ["RecordCanonicalConstructor"];
    let mut failures = Vec::new();
    for class in fixtures {
        let fixture = root.join(format!("{class}.java"));
        match compare_fixture(&translator, &fixture, class) {
            Ok(()) => {}
            Err(report) => failures.push(format!("{class}: {report}")),
        }
    }

    assert!(
        failures.is_empty(),
        "record canonical-constructor fixture output differs from Java:\n\n{}",
        failures.join("\n\n")
    );
}

#[test]
fn priority_queue_fixtures_match_java_output() {
    let translator = translator_binary().unwrap_or_else(|| {
        panic!("translator binary not built; run `cargo build --release` before differential tests")
    });
    for tool in ["javac", "java", "rustc"] {
        assert!(
            have_tool(tool),
            "required differential test tool `{tool}` is not available"
        );
    }

    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/differential");
    let fixtures = ["PriorityQueueHeadOrder", "PriorityQueueOfferHeadOrder"];
    let mut failures = Vec::new();
    for class in fixtures {
        let fixture = root.join(format!("{class}.java"));
        match compare_fixture(&translator, &fixture, class) {
            Ok(()) => {}
            Err(report) => failures.push(format!("{class}: {report}")),
        }
    }

    assert!(
        failures.is_empty(),
        "priority queue fixture output differs from Java:\n\n{}",
        failures.join("\n\n")
    );
}

#[test]
fn map_view_contract_fixtures_match_java_output() {
    let translator = translator_binary().unwrap_or_else(|| {
        panic!("translator binary not built; run `cargo build --release` before differential tests")
    });
    for tool in ["javac", "java", "rustc"] {
        assert!(
            have_tool(tool),
            "required differential test tool `{tool}` is not available"
        );
    }

    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/differential");
    let fixtures = [
        "CollectionAddAllReturn",
        "MapEntryDetachedContract",
        "MapEntryDisplay",
        "MapEntryFieldDisplay",
        "MapEntryNextDisplay",
        "MapEntrySetBulkContract",
        "MapEntrySetAddAllUnsupported",
        "MapEntrySetAddUnsupported",
        "MapEntrySetClear",
        "MapEntrySetContainsEntryLocal",
        "MapEntrySetContainsNext",
        "MapEntrySetContainsNextLocal",
        "MapEntrySetContainsParam",
        "MapEntrySetContainsSimpleEntry",
        "MapEntrySetEnhancedForSetValue",
        "MapEntrySetRemove",
        "MapEntrySetRemoveEntryLocal",
        "MapEntrySetRemoveNextLocal",
        "MapEntrySetRemoveParam",
        "MapEntrySetRemoveSimpleEntry",
        "MapEntrySetSetValue",
        "MapEntrySimpleImmutableEntry",
        "MapKeySetAddAllUnsupported",
        "MapKeySetAddUnsupported",
        "MapKeySetAliasRemove",
        "MapKeySetBulkContract",
        "MapKeySetClear",
        "MapKeySetRemove",
        "MapKeySetRemoveAll",
        "MapValuesAddUnsupported",
        "MapValuesAliasRemove",
        "MapValuesBulkContract",
        "MapValuesClear",
        "MapValuesContains",
        "MapValuesRemove",
        "MapValuesRemoveAll",
    ];
    let mut failures = Vec::new();
    for class in fixtures {
        let fixture = root.join(format!("{class}.java"));
        match compare_fixture(&translator, &fixture, class) {
            Ok(()) => {}
            Err(report) => failures.push(format!("{class}: {report}")),
        }
    }

    assert!(
        failures.is_empty(),
        "map-view contract fixture output differs from Java:\n\n{}",
        failures.join("\n\n")
    );
}

#[test]
fn borrowed_element_discharge_fixtures_match_java_output() {
    let translator = translator_binary().unwrap_or_else(|| {
        panic!("translator binary not built; run `cargo build --release` before differential tests")
    });
    for tool in ["javac", "java", "rustc"] {
        assert!(
            have_tool(tool),
            "required differential test tool `{tool}` is not available"
        );
    }

    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/differential");
    let fixtures = [
        "NestedMapValueRead",
        "EnumValuesElementIntoObject",
        "BorrowedElementIntoPrimitiveLocal",
    ];
    let mut failures = Vec::new();
    for class in fixtures {
        let fixture = root.join(format!("{class}.java"));
        match compare_fixture(&translator, &fixture, class) {
            Ok(()) => {}
            Err(report) => failures.push(format!("{class}: {report}")),
        }
    }

    assert!(
        failures.is_empty(),
        "borrowed-element discharge fixture output differs from Java:\n\n{}",
        failures.join("\n\n")
    );
}

#[test]
fn string_join_null_display_fixtures_match_java_output() {
    let translator = translator_binary().unwrap_or_else(|| {
        panic!("translator binary not built; run `cargo build --release` before differential tests")
    });
    for tool in ["javac", "java", "rustc"] {
        assert!(
            have_tool(tool),
            "required differential test tool `{tool}` is not available"
        );
    }

    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/differential");
    let fixtures = [
        "StringJoinNullDelimiter",
        "StringJoinNullElement",
        "StringJoinVarargsNullElement",
    ];
    let mut failures = Vec::new();
    for class in fixtures {
        let fixture = root.join(format!("{class}.java"));
        match compare_fixture(&translator, &fixture, class) {
            Ok(()) => {}
            Err(report) => failures.push(format!("{class}: {report}")),
        }
    }

    assert!(
        failures.is_empty(),
        "String.join null-display fixture output differs from Java:\n\n{}",
        failures.join("\n\n")
    );
}

#[test]
fn list_exception_fixtures_match_java_output() {
    let translator = translator_binary().unwrap_or_else(|| {
        panic!("translator binary not built; run `cargo build --release` before differential tests")
    });
    for tool in ["javac", "java", "rustc"] {
        assert!(
            have_tool(tool),
            "required differential test tool `{tool}` is not available"
        );
    }

    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/differential");
    let fixtures = [
        "ArrayCompoundWriteException",
        "ArrayIndexException",
        "ArrayNegativeSizeCatch",
        "ArrayWriteException",
        "ArraysBinarySearchRangeCatch",
        "ArraysCompareMismatchRangeCatch",
        "ArraysCopyOfCatch",
        "ArraysCopyOfRangeCatch",
        "ArraysRangeCatch",
        "ArraysStreamRangeCatch",
        "ArrayDequeFirstPopCatch",
        "ArrayDequeOffer",
        "BitSetClearFlipCatch",
        "BitSetCtorCatch",
        "BitSetGetCatch",
        "BitSetSetCatch",
        "CatchException",
        "CatchOrder",
        "CatchSupertype",
        "CollectionsSortComparatorThrowCatch",
        "CollectionsNCopiesCatch",
        "ExceptionHierarchy",
        "Exceptions",
        "ExceptionsCustom",
        "FloatingParseNull",
        "IntegerParseNull",
        "InputStreamReadBoundsCatch",
        "IntegralParseRadix",
        "IteratorNextCatch",
        "LinkedListQueueHeadCatch",
        "ListAddIndexCatch",
        "ListElementReceiverNullCatch",
        "ListGetCatch",
        "ListRemoveIndexCatch",
        "ListSetCatch",
        "ListSubListInvalidRangeCatch",
        "LongParseNull",
        "MapEntrySetAddAllUnsupported",
        "MapEntrySetAddUnsupported",
        "MapEntryIteratorNextCatch",
        "MapGetArithmeticNullCatch",
        "MapKeySetAddAllUnsupported",
        "MapKeySetAddUnsupported",
        "MapValuesAddUnsupported",
        "MathFloorDivModCatch",
        "MathToIntExactCatch",
        "NarrowParseNull",
        "NullArgCatch",
        "NullCallArgCatch",
        "NullableArrayLengthCatch",
        "NullableFieldReceiverCatch",
        "OptionalFactoryGetCatch",
        "OptionalOfNullCatch",
        "QueueElementCatch",
        "QueueRemoveCatch",
        "RandomNextIntCatch",
        "RaggedArrayNegativeSizeCatch",
        "StringBoundsException",
        "StringBuilderCapacityCatch",
        "StringBuilderCharAtCatch",
        "StringBuilderDeleteInsertCatch",
        "StringBuilderSetLengthCatch",
        "StringBuilderSubstringCatch",
        "StringByteRangeCtorCatch",
        "StringTokenizerCharAtBoundsCatch",
        "StringTokenizerNextCatch",
        "StringTokenizerNextElementCatch",
        "ThreadSleepCatch",
        "ThreadSleepNanosCatch",
        "TreeMapFirstLastKeyCatch",
        "UnmodifiableCollectionMutationCatch",
    ];
    let mut failures = Vec::new();
    for class in fixtures {
        let fixture = root.join(format!("{class}.java"));
        match compare_fixture(&translator, &fixture, class) {
            Ok(()) => {}
            Err(report) => failures.push(format!("{class}: {report}")),
        }
    }

    assert!(
        failures.is_empty(),
        "list exception fixture output differs from Java:\n\n{}",
        failures.join("\n\n")
    );
}

#[test]
fn finally_completion_fixtures_match_java_output() {
    let translator = translator_binary().unwrap_or_else(|| {
        panic!("translator binary not built; run `cargo build --release` before differential tests")
    });
    for tool in ["javac", "java", "rustc"] {
        assert!(
            have_tool(tool),
            "required differential test tool `{tool}` is not available"
        );
    }

    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/differential");
    let fixtures = [
        "FinallyReturnOverride",
        "CatchFinallyBreak",
        "CatchFinallyContinue",
        "CatchFinallyReturnOverride",
        "FinallyBreakOverridesReturn",
        "FinallyBreakOverridesCatchReturn",
        "FinallyContinueOverridesBreak",
        "FinallyNestedContinue",
        "FinallyBlockContinue",
        "FinallyDoWhileContinue",
        "FinallyLabeledContinue",
        "FinallyOnBreak",
        "FinallyOnContinue",
        "FinallyPaths",
        "FinallyThrowOverride",
        "FinallyWhileContinue",
        "CatchFinallyLabeledContinue",
        "TryCatchFinallyBodyBreak",
        "TryCatchReturnFinally",
        "TryWithResourcesFinally",
        "TryWithResourcesFinallyBodyBreak",
        "TryWithResourcesFinallyBodyContinue",
        "TryWithResourcesCatchFinally",
        "TryWithResourcesCatchFinallyBreak",
        "TryWithResourcesCatchFinallyBodyBreak",
        "TryWithResourcesCatchFinallyBodyContinue",
        "TryWithResourcesFinallyBreakOverridesCatchReturn",
        "TryWithResourcesFinallyContinueOverridesCatchReturn",
        "TryWithResourcesCloseThrowFinally",
        "TryWithResourcesMultipleCloseSuppressed",
        "TryWithResourcesPrimaryExceptionCatchFinally",
        "TryWithResourcesSuppressedPrimary",
        "TryWithResourcesCatchBreak",
        "TryWithResourcesCatchContinue",
        "TryWithResourcesBodyBreak",
        "TryWithResourcesBodyContinue",
    ];
    let mut failures = Vec::new();
    for class in fixtures {
        let fixture = root.join(format!("{class}.java"));
        match compare_fixture(&translator, &fixture, class) {
            Ok(()) => {}
            Err(report) => failures.push(format!("{class}: {report}")),
        }
    }

    assert!(
        failures.is_empty(),
        "finally completion fixture output differs from Java:\n\n{}",
        failures.join("\n\n")
    );
}

#[test]
fn known_gap_fixtures_are_valid_java_and_match_expected_failure_mode() {
    for tool in ["javac", "java", "rustc"] {
        assert!(
            have_tool(tool),
            "required known-gap differential test tool `{tool}` is not available"
        );
    }

    let fixtures = known_gap_fixtures();
    let expected_paths = fixtures
        .iter()
        .map(|fixture| fixture.display.to_string())
        .collect::<BTreeSet<_>>();
    let actual_paths = known_gap_fixture_paths()
        .into_iter()
        .map(|path| {
            path.strip_prefix(Path::new(env!("CARGO_MANIFEST_DIR")))
                .unwrap_or(&path)
                .to_string_lossy()
                .into_owned()
        })
        .collect::<BTreeSet<_>>();
    assert_eq!(
        fixtures.len(),
        EXPECTED_KNOWN_GAP_FIXTURES,
        "known-gap fixture count changed; update EXPECTED_KNOWN_GAP_FIXTURES intentionally"
    );
    assert_eq!(
        actual_paths, expected_paths,
        "known-gap fixture list does not match tests/differential/known-gaps/*.java"
    );

    let mut failures = Vec::new();
    for fixture in fixtures {
        if let Err(report) = check_known_gap(&fixture) {
            failures.push(format!("{}: {report}", fixture.display));
        }
    }

    assert!(
        failures.is_empty(),
        "known-gap fixture checks failed:\n\n{}",
        failures.join("\n\n")
    );
}

fn compare_fixture(translator: &Path, fixture: &Path, class: &str) -> Result<(), String> {
    let work = std::env::temp_dir().join(format!("new_java2rust_differential_{class}"));
    let _ = std::fs::remove_dir_all(&work);
    std::fs::create_dir_all(&work).map_err(|err| format!("temp dir: {err}"))?;

    let java_source = work.join(format!("{class}.java"));
    std::fs::copy(fixture, &java_source).map_err(|err| format!("copy fixture: {err}"))?;

    let expected = run_java(&work, &java_source, class)?;
    let actual = run_translated(translator, &work, &java_source, class)?;

    if expected == actual {
        Ok(())
    } else {
        Err(format!(
            "java produced {expected:?}, translation produced {actual:?}"
        ))
    }
}

struct KnownGapFixture {
    display: &'static str,
    class: &'static str,
    expected_java_output: &'static str,
    expectation: KnownGapExpectation,
}

enum KnownGapExpectation {
    LoudDiagnostic {
        diagnostic_code: &'static str,
        also_diagnostic_contains: &'static [(&'static str, &'static str)],
    },
    LoudCompileError {
        message: &'static str,
        diagnostic_code: Option<&'static str>,
        rust_contains: &'static [&'static str],
        rust_not_contains: &'static [&'static str],
    },
    RustcError {
        message: &'static str,
        rust_contains: &'static [&'static str],
    },
    LoudRuntimePanic {
        message: &'static str,
        diagnostic_code: &'static str,
    },
}

fn known_gap_fixtures() -> Vec<KnownGapFixture> {
    vec![
        KnownGapFixture {
            display: "tests/differential/known-gaps/Aliasing.java",
            class: "Aliasing",
            expected_java_output: "5|7|3|9|11|13|17|19",
            expectation: KnownGapExpectation::LoudDiagnostic {
                diagnostic_code: "observable_alias",
                also_diagnostic_contains: &[
                    (
                        "storage_strategy_disagreement",
                        "Aliasing.Node needs SharedLocal",
                    ),
                    (
                        "storage_strategy_disagreement",
                        "Aliasing.Counter needs SharedLocal",
                    ),
                ],
            },
        },
        KnownGapFixture {
            display: "tests/differential/known-gaps/InheritanceUpcast.java",
            class: "InheritanceUpcast",
            expected_java_output: "savings,105,savings:105",
            expectation: KnownGapExpectation::LoudCompileError {
                message: "unsupported Java class upcast: Savings to Account requires class hierarchy lowering",
                diagnostic_code: Some("unsupported_inheritance"),
                rust_contains: &[],
                rust_not_contains: &["let a: Account = Savings::new(100, 5).into();"],
            },
        },
        KnownGapFixture {
            display: "tests/differential/known-gaps/ExplicitInheritanceUpcast.java",
            class: "ExplicitInheritanceUpcast",
            expected_java_output: "savings",
            expectation: KnownGapExpectation::LoudCompileError {
                message: "unsupported Java class upcast: Savings to Account requires class hierarchy lowering",
                diagnostic_code: Some("unsupported_inheritance"),
                rust_contains: &[],
                rust_not_contains: &["Savings::new(100, 5).into()"],
            },
        },
        KnownGapFixture {
            display: "tests/differential/known-gaps/SuperMutatingCall.java",
            class: "SuperMutatingCall",
            expected_java_output: "11",
            expectation: KnownGapExpectation::LoudCompileError {
                message: "unsupported Java mutating super call: Base.bump requires class hierarchy lowering",
                diagnostic_code: Some("unsupported_inheritance"),
                rust_contains: &[],
                rust_not_contains: &["Base::bump(&"],
            },
        },
        KnownGapFixture {
            display: "tests/differential/known-gaps/NestedInheritanceUpcast.java",
            class: "NestedInheritanceUpcast",
            expected_java_output: "woof,woofwoof",
            expectation: KnownGapExpectation::LoudCompileError {
                message: "unsupported Java class upcast: NestedInheritanceUpcast.Dog to NestedInheritanceUpcast.Animal requires class hierarchy lowering",
                diagnostic_code: Some("unsupported_inheritance"),
                rust_contains: &[],
                rust_not_contains: &["NestedInheritanceUpcast_Dog::new().into()"],
            },
        },
        KnownGapFixture {
            display: "tests/differential/known-gaps/PolymorphicListUpcast.java",
            class: "PolymorphicListUpcast",
            expected_java_output: "savings,105",
            expectation: KnownGapExpectation::LoudCompileError {
                message: "unsupported Java class upcast: Savings to Account requires class hierarchy lowering",
                diagnostic_code: Some("unsupported_inheritance"),
                rust_contains: &[],
                rust_not_contains: &["accounts.add(Savings::new"],
            },
        },
        KnownGapFixture {
            display: "tests/differential/known-gaps/InnerCapture.java",
            class: "InnerCapture",
            expected_java_output: "21",
            expectation: KnownGapExpectation::LoudRuntimePanic {
                message: "unsupported Java inner-class outer capture construction for InnerCapture.Adder; pass an outer InnerCapture handle",
                diagnostic_code: "inner_class_construction_panics",
            },
        },
        KnownGapFixture {
            display: "tests/differential/known-gaps/QualifiedInnerCapture.java",
            class: "QualifiedInnerCapture",
            expected_java_output: "21",
            expectation: KnownGapExpectation::LoudRuntimePanic {
                message: "unsupported Java inner-class outer capture construction for QualifiedInnerCapture.Inner; pass an outer QualifiedInnerCapture handle",
                diagnostic_code: "inner_class_construction_panics",
            },
        },
        KnownGapFixture {
            display: "tests/differential/known-gaps/FinallySwitchBreak.java",
            class: "FinallySwitchBreak",
            expected_java_output: "zfofdf",
            expectation: KnownGapExpectation::RustcError {
                message: "E0267",
                rust_contains: &["|| -> Result", "break;"],
            },
        },
        KnownGapFixture {
            display: "tests/differential/known-gaps/InstanceofSupertype.java",
            class: "InstanceofSupertype",
            expected_java_output: "true,true,false",
            expectation: KnownGapExpectation::LoudCompileError {
                message: "unsupported Java instanceof check: target type A has source subtypes and requires class hierarchy lowering",
                diagnostic_code: Some("unsupported_instanceof_source_hierarchy_runtime_carrier"),
                rust_contains: &[],
                rust_not_contains: &[".is::<A>()"],
            },
        },
        KnownGapFixture {
            display: "tests/differential/known-gaps/ErasedObjectSupertypeCast.java",
            class: "ErasedObjectSupertypeCast",
            expected_java_output: "B",
            expectation: KnownGapExpectation::LoudCompileError {
                message: "unsupported Java cast: target type A has source subtypes and requires class hierarchy lowering",
                diagnostic_code: Some("unsupported_cast_source_hierarchy_runtime_carrier"),
                rust_contains: &[],
                rust_not_contains: &["o.downcast_ref::<A>()", "o.downcast::<A>()"],
            },
        },
        KnownGapFixture {
            display: "tests/differential/known-gaps/PatternInstanceofSupertype.java",
            class: "PatternInstanceofSupertype",
            expected_java_output: "A,A,other",
            expectation: KnownGapExpectation::LoudCompileError {
                message: "unsupported Java instanceof check: target type A has source subtypes and requires class hierarchy lowering",
                diagnostic_code: Some("unsupported_instanceof_source_hierarchy_runtime_carrier"),
                rust_contains: &[],
                rust_not_contains: &["downcast_ref::<A>()"],
            },
        },
        KnownGapFixture {
            display: "tests/differential/known-gaps/PatternInstanceofSupertypeTernary.java",
            class: "PatternInstanceofSupertypeTernary",
            expected_java_output: "A,A,other",
            expectation: KnownGapExpectation::LoudCompileError {
                message: "unsupported Java instanceof check: target type A has source subtypes and requires class hierarchy lowering",
                diagnostic_code: Some("unsupported_instanceof_source_hierarchy_runtime_carrier"),
                rust_contains: &[],
                rust_not_contains: &["match o.downcast_ref::<A>()"],
            },
        },
        KnownGapFixture {
            display: "tests/differential/known-gaps/StringEqualsVsEq.java",
            class: "StringEqualsVsEq",
            expected_java_output: "true,false,true,true,false",
            expectation: KnownGapExpectation::LoudCompileError {
                message: "unsupported Java String reference identity comparison requires string identity representation",
                diagnostic_code: Some("java_string_identity"),
                rust_contains: &[],
                rust_not_contains: &[],
            },
        },
        KnownGapFixture {
            display: "tests/differential/known-gaps/ArrayBlockingQueueAdd.java",
            class: "ArrayBlockingQueueAdd",
            expected_java_output: "true,a,IllegalStateException",
            expectation: KnownGapExpectation::LoudCompileError {
                message: "Java ArrayBlockingQueue.add(value) requires bounded-capacity queue support",
                diagnostic_code: None,
                rust_contains: &[],
                rust_not_contains: &["queue.push("],
            },
        },
        KnownGapFixture {
            display: "tests/differential/known-gaps/ArrayBlockingQueueOffer.java",
            class: "ArrayBlockingQueueOffer",
            expected_java_output: "true,false,a",
            expectation: KnownGapExpectation::LoudCompileError {
                message: "Java ArrayBlockingQueue.offer(value) requires bounded-capacity queue support",
                diagnostic_code: None,
                rust_contains: &[],
                rust_not_contains: &["queue.push("],
            },
        },
        KnownGapFixture {
            display: "tests/differential/known-gaps/PriorityQueueComparatorConstructor.java",
            class: "PriorityQueueComparatorConstructor",
            expected_java_output: "3,2,1",
            expectation: KnownGapExpectation::LoudCompileError {
                message: "Java PriorityQueue(Comparator) requires comparator-backed priority-queue support",
                diagnostic_code: None,
                rust_contains: &[],
                rust_not_contains: &["return comparator;"],
            },
        },
    ]
}

fn check_known_gap(fixture: &KnownGapFixture) -> Result<(), String> {
    let source_path = Path::new(env!("CARGO_MANIFEST_DIR")).join(fixture.display);
    let source = std::fs::read_to_string(&source_path).map_err(|err| err.to_string())?;

    let work = std::env::temp_dir().join(format!("new_java2rust_known_gap_{}", fixture.class));
    let _ = std::fs::remove_dir_all(&work);
    std::fs::create_dir_all(&work).map_err(|err| format!("temp dir: {err}"))?;
    let java_output = run_java(&work, &source_path, fixture.class);
    let java_output = match java_output {
        Ok(output) => output,
        Err(err) => {
            let _ = std::fs::remove_dir_all(&work);
            return Err(err);
        }
    };

    if java_output != fixture.expected_java_output {
        let _ = std::fs::remove_dir_all(&work);
        return Err(format!(
            "Java oracle changed: expected {:?}, got {:?}",
            fixture.expected_java_output, java_output
        ));
    }

    let translation = translate_source(&source);
    match fixture.expectation {
        KnownGapExpectation::LoudDiagnostic {
            diagnostic_code,
            also_diagnostic_contains,
        } => {
            let diagnostic = translation
                .diagnostics
                .iter()
                .find(|diagnostic| diagnostic.code == diagnostic_code)
                .ok_or_else(|| {
                    format!(
                        "translation did not report diagnostic code {diagnostic_code:?}: {:?}",
                        translation.diagnostics.iter().collect::<Vec<_>>()
                    )
                })?;
            if diagnostic.severity != new_java2rust::diagnostics::Severity::Error {
                let _ = std::fs::remove_dir_all(&work);
                return Err(format!(
                    "diagnostic code {diagnostic_code:?} was not an error: {:?}",
                    diagnostic.severity
                ));
            }
            for (code, message) in also_diagnostic_contains {
                if !translation.diagnostics.iter().any(|diagnostic| {
                    diagnostic.code == *code && diagnostic.message.contains(message)
                }) {
                    let _ = std::fs::remove_dir_all(&work);
                    return Err(format!(
                        "translation did not report diagnostic {code:?} containing {message:?}: {:?}",
                        translation.diagnostics.iter().collect::<Vec<_>>()
                    ));
                }
            }
        }
        KnownGapExpectation::LoudCompileError {
            message,
            diagnostic_code,
            rust_contains,
            rust_not_contains,
        } => {
            let expected = format!("compile_error!(\"{message}\")");
            if !translation.rust.contains(&expected) {
                let _ = std::fs::remove_dir_all(&work);
                return Err(format!(
                    "translation did not fail loudly with {expected:?}\n{}",
                    translation.rust
                ));
            }
            if let Some(diagnostic_code) = diagnostic_code
                && !translation.diagnostics.has_code(diagnostic_code)
            {
                let _ = std::fs::remove_dir_all(&work);
                return Err(format!(
                    "translation did not report diagnostic code {diagnostic_code:?}: {:?}",
                    translation.diagnostics.iter().collect::<Vec<_>>()
                ));
            }
            check_rust_fragments(&translation.rust, rust_contains, rust_not_contains).map_err(
                |err| {
                    let _ = std::fs::remove_dir_all(&work);
                    err
                },
            )?;
        }
        KnownGapExpectation::RustcError {
            message,
            rust_contains,
        } => {
            check_rust_fragments(&translation.rust, rust_contains, &[]).map_err(|err| {
                let _ = std::fs::remove_dir_all(&work);
                err
            })?;
            let run = run_translated_rust_source(&work, fixture.class, &translation.rust);
            let Err(err) = run else {
                let _ = std::fs::remove_dir_all(&work);
                return Err("known rustc-loud translation unexpectedly compiled".to_string());
            };
            if !err.contains(message) {
                let _ = std::fs::remove_dir_all(&work);
                return Err(format!(
                    "translation did not fail at rustc with {message:?}: {err}"
                ));
            }
        }
        KnownGapExpectation::LoudRuntimePanic {
            message,
            diagnostic_code,
        } => {
            if !translation.diagnostics.has_code(diagnostic_code) {
                let _ = std::fs::remove_dir_all(&work);
                return Err(format!(
                    "translation did not report diagnostic code {diagnostic_code:?}: {:?}",
                    translation.diagnostics.iter().collect::<Vec<_>>()
                ));
            }
            let run = run_translated_rust_source(&work, fixture.class, &translation.rust);
            let Err(err) = run else {
                let _ = std::fs::remove_dir_all(&work);
                return Err("known runtime-loud translation unexpectedly ran".to_string());
            };
            if !err.contains(message) {
                let _ = std::fs::remove_dir_all(&work);
                return Err(format!(
                    "translation did not fail at runtime with {message:?}: {err}"
                ));
            }
        }
    }
    let _ = std::fs::remove_dir_all(&work);
    Ok(())
}

fn check_rust_fragments(rust: &str, required: &[&str], forbidden: &[&str]) -> Result<(), String> {
    for expected in required {
        if !rust.contains(expected) {
            return Err(format!(
                "translation did not contain expected generated Rust fragment \
                 {expected:?}\n{rust}"
            ));
        }
    }
    for unexpected in forbidden {
        if rust.contains(unexpected) {
            return Err(format!(
                "translation still contains forbidden generated Rust fragment \
                 {unexpected:?}\n{rust}"
            ));
        }
    }
    Ok(())
}

fn known_gap_fixture_paths() -> Vec<PathBuf> {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/differential/known-gaps");
    let mut paths = std::fs::read_dir(dir)
        .map(|entries| {
            entries
                .filter_map(Result::ok)
                .map(|entry| entry.path())
                .filter(|path| path.extension().is_some_and(|ext| ext == "java"))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    paths.sort();
    paths
}

fn run_translated_rust_source(work: &Path, class: &str, rust: &str) -> Result<String, String> {
    let binary = compile_translated_rust_source(work, class, rust)?;
    let run = Command::new(&binary)
        .output()
        .map_err(|err| format!("run: {err}"))?;
    if !run.status.success() {
        return Err(format!(
            "translated program failed: {}",
            String::from_utf8_lossy(&run.stderr).trim_end()
        ));
    }
    Ok(String::from_utf8_lossy(&run.stdout).trim_end().to_string())
}

fn compile_translated_rust_source(work: &Path, class: &str, rust: &str) -> Result<PathBuf, String> {
    let mut rust = rust.to_string();
    rust.push_str(&harness_main(&rust, class));
    let rust_path = work.join(format!("{class}.rs"));
    std::fs::write(&rust_path, rust).map_err(|err| format!("write rust: {err}"))?;

    let binary = work.join(class);
    let compile = Command::new("rustc")
        .args(["--edition", "2021", "--crate-name"])
        .arg(class.to_lowercase())
        .arg(&rust_path)
        .arg("-o")
        .arg(&binary)
        .output()
        .map_err(|err| format!("rustc: {err}"))?;
    if !compile.status.success() {
        return Err(format!(
            "generated Rust did not compile: {}",
            String::from_utf8_lossy(&compile.stderr)
        ));
    }
    Ok(binary)
}

fn run_java(work: &Path, source: &Path, class: &str) -> Result<String, String> {
    let compile = Command::new("javac")
        .arg("-d")
        .arg(work)
        .arg(source)
        .output()
        .map_err(|err| format!("javac: {err}"))?;
    if !compile.status.success() {
        return Err(format!(
            "javac failed: {}",
            String::from_utf8_lossy(&compile.stderr)
        ));
    }
    let run = Command::new("java")
        .arg("-cp")
        .arg(work)
        .arg(class)
        .output()
        .map_err(|err| format!("java: {err}"))?;
    if !run.status.success() {
        return Err(format!(
            "java failed: {}",
            String::from_utf8_lossy(&run.stderr)
        ));
    }
    Ok(String::from_utf8_lossy(&run.stdout).trim_end().to_string())
}

fn run_translated(
    translator: &Path,
    work: &Path,
    source: &Path,
    class: &str,
) -> Result<String, String> {
    let translated = Command::new(translator)
        .arg(source)
        .output()
        .map_err(|err| format!("translator: {err}"))?;
    if !translated.status.success() {
        return Err(format!(
            "translation failed: {}",
            String::from_utf8_lossy(&translated.stderr)
        ));
    }
    let mut rust = String::from_utf8_lossy(&translated.stdout).into_owned();
    rust.push_str(&harness_main(&rust, class));

    let rust_path = work.join(format!("{class}.rs"));
    std::fs::write(&rust_path, &rust).map_err(|err| format!("write rust: {err}"))?;

    let binary = work.join(class);
    let compile = Command::new("rustc")
        .args(["--edition", "2021", "--crate-name"])
        .arg(class.to_lowercase())
        .arg(&rust_path)
        .arg("-o")
        .arg(&binary)
        .output()
        .map_err(|err| format!("rustc: {err}"))?;
    if !compile.status.success() {
        let stderr = String::from_utf8_lossy(&compile.stderr);
        let errors = stderr
            .lines()
            .filter(|line| line.starts_with("error"))
            .take(5)
            .collect::<Vec<_>>()
            .join("; ");
        return Err(format!("generated Rust did not compile: {errors}"));
    }
    let run = Command::new(&binary)
        .output()
        .map_err(|err| format!("run: {err}"))?;
    if !run.status.success() {
        return Err(format!(
            "translated program failed: {}",
            String::from_utf8_lossy(&run.stderr).trim_end()
        ));
    }
    Ok(String::from_utf8_lossy(&run.stdout).trim_end().to_string())
}

/// `run()` is lowered as fallible whenever the analysis says it can throw, so
/// the harness has to accept both shapes.
fn harness_main(rust: &str, class: &str) -> String {
    let fallible = rust.contains(&format!("pub fn run() -> Result<"));
    if fallible {
        format!(
            "\nfn main() {{\n    match {class}::run() {{\n        Ok(value) => println!(\"{{value}}\"),\n        Err(err) => println!(\"EXCEPTION:{{}}\", err.class_name),\n    }}\n}}\n"
        )
    } else {
        format!("\nfn main() {{\n    println!(\"{{}}\", {class}::run());\n}}\n")
    }
}

fn fixture_paths() -> Vec<PathBuf> {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/differential");
    let mut paths = std::fs::read_dir(dir)
        .map(|entries| {
            entries
                .filter_map(Result::ok)
                .map(|entry| entry.path())
                .filter(|path| path.extension().is_some_and(|ext| ext == "java"))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    paths.sort();
    paths
}

fn translator_binary() -> Option<PathBuf> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    ["release", "debug"]
        .iter()
        .map(|profile| root.join("target").join(profile).join("new-java2rust"))
        .find(|path| path.exists())
}

fn have_tool(tool: &str) -> bool {
    Command::new(tool)
        .arg("--version")
        .output()
        .is_ok_and(|output| output.status.success())
}
