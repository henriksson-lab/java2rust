#[path = "../src/java_native_reference.rs"]
mod reference;
use reference::{JavaLocalNativeRef, JavaSyncNativeRef};

#[test]
fn projected_headers_bridge_existing_allocations_and_weak_views_do_not_leak() {
    let root = std::rc::Rc::new(std::cell::RefCell::new(vec![1]));
    let first = JavaLocalNativeRef::projected_from_rc(&root, root.clone());
    let second = JavaLocalNativeRef::projected_from_rc(&root, ());
    let weak_root = std::rc::Rc::downgrade(&root);
    let weak_view = first.downgrade();
    assert_eq!(first.identity_ptr(), std::rc::Rc::as_ptr(&root).cast());
    assert_eq!(first.identity_ptr(), second.identity_ptr());
    let owners_before = std::rc::Rc::strong_count(&root);
    let copies = (0..1000).map(|_| first.clone()).collect::<Vec<_>>();
    assert_eq!(std::rc::Rc::strong_count(&root), owners_before);
    drop(copies);
    let alias = weak_view.upgrade().unwrap();
    alias.borrow_mut().push(2);
    assert_eq!(root.borrow().as_slice(), [1, 2]);
    drop((root, first, alias));
    assert!(weak_view.upgrade().is_none());
    assert!(weak_root.upgrade().is_some());
    drop(second);
    assert!(weak_root.upgrade().is_none());
    let sync = std::sync::Arc::new(std::sync::Mutex::new(4));
    let view = JavaSyncNativeRef::projected_from_arc(&sync, ());
    assert_eq!(view.identity_ptr(), std::sync::Arc::as_ptr(&sync).cast());
}

#[test]
fn local_projection_keeps_identity_while_payloads_are_borrowed() {
    struct NotClone(i32);
    let source = JavaLocalNativeRef::new(std::cell::RefCell::new(NotClone(7)));
    let copy = source.clone();
    let view = JavaLocalNativeRef::projected(&source, std::cell::RefCell::new(copy.clone()));
    let source_loan = source.borrow_mut();
    let view_loan = view.borrow_mut();
    assert_eq!(source.identity_ptr(), copy.identity_ptr());
    assert_eq!(source.identity_ptr(), view.identity_ptr());
    assert_eq!(source_loan.0, 7);
    assert_eq!(view_loan.identity_ptr(), source.identity_ptr());
    drop((source_loan, view_loan));
    view.borrow().borrow_mut().0 = 19;
    assert_eq!(source.borrow().0, 19);
}

#[test]
fn identity_owner_retains_root_without_retaining_intermediate_views() {
    struct Watched(std::rc::Rc<std::cell::Cell<usize>>);
    impl Drop for Watched {
        fn drop(&mut self) {
            self.0.set(self.0.get() + 1);
        }
    }
    let root_drops = std::rc::Rc::new(std::cell::Cell::new(0));
    let view_drops = std::rc::Rc::new(std::cell::Cell::new(0));
    let root = JavaLocalNativeRef::new(Watched(root_drops.clone()));
    let view = JavaLocalNativeRef::projected(&root, Watched(view_drops.clone()));
    let final_view = JavaLocalNativeRef::projected(&view, ());
    assert_eq!(root.identity_ptr(), final_view.identity_ptr());
    drop((root, view));
    assert_eq!(view_drops.get(), 1);
    assert_eq!(root_drops.get(), 0);
    drop(final_view);
    assert_eq!(root_drops.get(), 1);
}

#[test]
fn sync_projection_identity_needs_no_lock_and_aliases_cross_threads() {
    struct NotClone(Vec<i32>);
    let source = JavaSyncNativeRef::new(std::sync::Mutex::new(NotClone(vec![1])));
    let view = JavaSyncNativeRef::projected(&source, std::sync::Mutex::new(source.clone()));
    let source_lock = source.lock().unwrap();
    let view_lock = view.lock().unwrap();
    assert_eq!(source.identity_ptr(), view.identity_ptr());
    let cloned_while_locked = view.clone();
    assert_eq!(source.identity_ptr(), cloned_while_locked.identity_ptr());
    drop((source_lock, view_lock));
    std::thread::spawn(move || {
        let owner = cloned_while_locked.lock().unwrap().clone();
        owner.lock().unwrap().0.push(2);
    })
    .join()
    .unwrap();
    assert_eq!(source.lock().unwrap().0, [1, 2]);
}

#[test]
fn distinct_allocations_and_optional_reference_copies_keep_their_identity() {
    let first = JavaLocalNativeRef::new(std::cell::RefCell::new(vec![1]));
    let second = JavaLocalNativeRef::new(std::cell::RefCell::new(vec![1]));
    assert_ne!(first.identity_ptr(), second.identity_ptr());
    let nullable = Some(first);
    let copied = nullable.clone();
    assert_eq!(
        nullable.as_ref().unwrap().identity_ptr(),
        copied.as_ref().unwrap().identity_ptr()
    );
    copied.as_ref().unwrap().borrow_mut().push(2);
    assert_eq!(nullable.as_ref().unwrap().borrow().as_slice(), [1, 2]);
}
