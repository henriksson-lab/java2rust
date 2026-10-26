#[path = "common/mod.rs"]
mod common;

#[test]
fn java_bridge_boundaries_distinguish_array_class_casts_reads_and_array_stores() {
    // Entry-gate oracle for the shared generic view contracts. Reuse this
    // fixture in the translated gate when adapter admission is implemented.
    common::verify_java_sources_zero_contract(
        "GenericBridgeContractProbe",
        &[(
            "tests/fixtures/GenericBridgeContractProbe.java",
            include_str!("fixtures/GenericBridgeContractProbe.java"),
        )],
    );
}
