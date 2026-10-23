use crate::java_string_identity::JavaStringIdentity;
use std::sync::{Arc, Mutex};

#[test]
fn clones_share_identity_but_equal_fresh_payloads_do_not() {
    for value in ["", "text", "é😀"] {
        let left = JavaStringIdentity::from_payload(Arc::<str>::from(value));
        let fresh = JavaStringIdentity::from_payload(Arc::<str>::from(value));
        assert!(left.java_equals(&fresh));
        assert!(!left.java_identity_eq(&fresh));
        assert!(left.java_identity_eq(&left.clone()));
        assert_eq!(left.identity_ptr(), left.clone().identity_ptr());
    }
    assert_eq!(JavaStringIdentity::<str>::default().payload(), "");
}

#[test]
fn intern_literals_and_receivers_share_one_pool_without_copying_receiver_identity() {
    let pool = Mutex::new(std::collections::HashMap::new());
    let left = JavaStringIdentity::from_payload(Arc::<[u16]>::from(vec![0xd800, 0, 0xdfff]));
    let interned = left.intern_reference(&pool);
    assert!(left.java_identity_eq(&interned));
    let other = JavaStringIdentity::from_payload(Arc::<[u16]>::from(left.payload()));
    assert!(!other.java_identity_eq(&interned));
    assert!(other.intern_reference(&pool).java_identity_eq(&interned));
    assert!(JavaStringIdentity::intern(left.payload(), &pool).java_identity_eq(&interned));
    assert_eq!(pool.lock().unwrap().len(), 1);
}

#[test]
fn racing_intern_calls_retain_exactly_one_of_the_original_references() {
    let pool = Arc::new(Mutex::new(std::collections::HashMap::new()));
    let barrier = Arc::new(std::sync::Barrier::new(8));
    let threads: Vec<_> = (0..8)
        .map(|_| {
            let pool = pool.clone();
            let barrier = barrier.clone();
            std::thread::spawn(move || {
                let fresh =
                    JavaStringIdentity::from_payload(Arc::<[u16]>::from(vec![0xd800, 65, 0xdc00]));
                barrier.wait();
                let interned = fresh.intern_reference(&pool);
                (fresh, interned)
            })
        })
        .collect();
    let results: Vec<_> = threads.into_iter().map(|t| t.join().unwrap()).collect();
    let canonical = &results[0].1;
    assert!(
        results
            .iter()
            .all(|(_, value)| value.java_identity_eq(canonical))
    );
    assert_eq!(
        results
            .iter()
            .filter(|(fresh, _)| fresh.java_identity_eq(canonical))
            .count(),
        1
    );
    assert_eq!(pool.lock().unwrap().len(), 1);
}
