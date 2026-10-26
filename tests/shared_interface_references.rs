#[path = "common/mod.rs"]
mod common;

#[test]
fn interface_getters_copy_references_without_allocations_or_guards_across_reentry() {
    common::verify_java_fixture_zero_contract(
        "shared_interface_reference",
        "SharedInterfaceReferenceProbe",
        "tests/fixtures/SharedInterfaceReferenceProbe.java",
        include_str!("fixtures/SharedInterfaceReferenceProbe.java"),
        r#"
trait Exit { fn value(self) -> i32; }
impl Exit for i32 { fn value(self) -> i32 { self } }
impl<E: std::fmt::Debug> Exit for Result<i32,E> { fn value(self) -> i32 { self.unwrap() } }
#[test] fn identity_mutation_casts_and_reentry() { assert_eq!(SharedInterfaceReferenceProbe::run().value(), 0); }
struct CopyAllocator;
std::thread_local! {
    static ENABLED: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    static ALLOCATIONS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}
unsafe impl std::alloc::GlobalAlloc for CopyAllocator {
    unsafe fn alloc(&self, layout: std::alloc::Layout) -> *mut u8 {
        let p = unsafe { std::alloc::GlobalAlloc::alloc(&std::alloc::System, layout) };
        if !p.is_null() && ENABLED.try_with(|v| v.get()).unwrap_or(false) {
            let _ = ALLOCATIONS.try_with(|v| v.set(v.get()+1));
        }
        p
    }
    unsafe fn dealloc(&self, p: *mut u8, layout: std::alloc::Layout) { unsafe { std::alloc::GlobalAlloc::dealloc(&std::alloc::System, p, layout); } }
}
#[global_allocator] static ALLOCATOR: CopyAllocator = CopyAllocator;
#[test] fn repeated_getters_allocate_nothing() {
    struct NotClone;
    let _copy: fn(&__JavaReferenceRefCounter<NotClone>) -> __JavaReferenceRefCounter<NotClone> = std::clone::Clone::clone;
    let holder = SharedInterfaceReferenceProbe::holder().unwrap();
    assert_eq!(SharedInterfaceReferenceProbe::read_copies(&holder, 1).value(), 0);
    ALLOCATIONS.with(|v| v.set(0));
    ENABLED.with(|v| v.set(true));
    let result = SharedInterfaceReferenceProbe::read_copies(&holder, 1000);
    ENABLED.with(|v| v.set(false));
    assert_eq!(result.value(), 0);
    assert_eq!(ALLOCATIONS.with(|v| v.get()), 0);
}
"#,
    );
}

#[test]
fn static_interface_references_preserve_sync_and_reentrant_dispatch() {
    common::verify_java_fixture_zero_contract(
        "shared_sync_interface_reference",
        "SharedInterfaceSyncReferenceProbe",
        "tests/fixtures/SharedInterfaceSyncReferenceProbe.java",
        include_str!("fixtures/SharedInterfaceSyncReferenceProbe.java"),
        r#"
trait Exit { fn value(self) -> i32; }
impl Exit for i32 { fn value(self) -> i32 { self } }
impl<E: std::fmt::Debug> Exit for Result<i32,E> { fn value(self) -> i32 { self.unwrap() } }
#[test] fn sync_and_reentry() {
    fn send_sync<T: Send + Sync>() {}
    send_sync::<__JavaReferenceSyncReferenceCounter>();
    assert_eq!(SharedInterfaceSyncReferenceProbe::run().value(), 0);
}
"#,
    );
}

#[test]
fn interface_reference_constructors_survive_production_crate_assembly() {
    common::cargo_run_translated_batch_sources(
        "shared_interface_reference_assembly",
        &[new_java2rust::BatchSource::new(
            "SharedInterfaceSyncReferenceProbe.java",
            include_str!("fixtures/SharedInterfaceSyncReferenceProbe.java"),
        )],
        r#"
trait Exit { fn value(self) -> i32; }
impl Exit for i32 { fn value(self) -> i32 { self } }
impl<E: std::fmt::Debug> Exit for Result<i32,E> { fn value(self) -> i32 { self.unwrap() } }
fn main() { assert_eq!(translated::SharedInterfaceSyncReferenceProbe::run().value(), 0); }
"#,
    );
}
