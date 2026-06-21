/// `java.util.concurrent.atomic.AtomicInteger` over `std::sync::atomic::AtomicI32`.
/// All methods take `&self` (the std atomics already do), so a `static final`
/// counter (lowered to a `const`/field) works without `&mut`.
#[derive(Debug, Default)]
pub struct JavaAtomicInteger {
    v: std::sync::atomic::AtomicI32,
}
impl Clone for JavaAtomicInteger {
    fn clone(&self) -> Self {
        JavaAtomicInteger { v: std::sync::atomic::AtomicI32::new(self.get()) }
    }
}
// Java code compares/keys atomics; `AtomicI32` isn't `PartialEq`/`Eq`/`Hash`, so
// key on the current value.
value_eq_hash!(JavaAtomicInteger, get());
impl JavaAtomicInteger {
    pub fn new() -> Self {
        JavaAtomicInteger { v: std::sync::atomic::AtomicI32::new(0) }
    }
    pub fn new_1(initial: i32) -> Self {
        JavaAtomicInteger { v: std::sync::atomic::AtomicI32::new(initial) }
    }
    pub fn get(&self) -> i32 {
        self.v.load(std::sync::atomic::Ordering::SeqCst)
    }
    pub fn set(&self, n: i32) {
        self.v.store(n, std::sync::atomic::Ordering::SeqCst)
    }
    pub fn get_and_set(&self, n: i32) -> i32 {
        self.v.swap(n, std::sync::atomic::Ordering::SeqCst)
    }
    pub fn get_and_add(&self, d: i32) -> i32 {
        self.v.fetch_add(d, std::sync::atomic::Ordering::SeqCst)
    }
    pub fn add_and_get(&self, d: i32) -> i32 {
        self.v.fetch_add(d, std::sync::atomic::Ordering::SeqCst) + d
    }
    pub fn get_and_increment(&self) -> i32 {
        self.get_and_add(1)
    }
    pub fn increment_and_get(&self) -> i32 {
        self.add_and_get(1)
    }
    pub fn get_and_decrement(&self) -> i32 {
        self.get_and_add(-1)
    }
    pub fn decrement_and_get(&self) -> i32 {
        self.add_and_get(-1)
    }
    pub fn compare_and_set(&self, expect: i32, update: i32) -> bool {
        self.v
            .compare_exchange(
                expect,
                update,
                std::sync::atomic::Ordering::SeqCst,
                std::sync::atomic::Ordering::SeqCst,
            )
            .is_ok()
    }
    pub fn int_value(&self) -> i32 {
        self.get()
    }
    pub fn long_value(&self) -> i64 {
        self.get() as i64
    }
    pub fn double_value(&self) -> f64 {
        self.get() as f64
    }
    pub fn float_value(&self) -> f32 {
        self.get() as f32
    }
}
value_display!(JavaAtomicInteger, get());

/// `java.util.concurrent.atomic.AtomicLong` over `AtomicI64`.
#[derive(Debug, Default)]
pub struct JavaAtomicLong {
    v: std::sync::atomic::AtomicI64,
}
impl Clone for JavaAtomicLong {
    fn clone(&self) -> Self {
        JavaAtomicLong { v: std::sync::atomic::AtomicI64::new(self.get()) }
    }
}
value_eq_hash!(JavaAtomicLong, get());
impl JavaAtomicLong {
    pub fn new() -> Self {
        JavaAtomicLong { v: std::sync::atomic::AtomicI64::new(0) }
    }
    pub fn new_1(initial: i64) -> Self {
        JavaAtomicLong { v: std::sync::atomic::AtomicI64::new(initial) }
    }
    pub fn get(&self) -> i64 {
        self.v.load(std::sync::atomic::Ordering::SeqCst)
    }
    pub fn set(&self, n: i64) {
        self.v.store(n, std::sync::atomic::Ordering::SeqCst)
    }
    pub fn get_and_set(&self, n: i64) -> i64 {
        self.v.swap(n, std::sync::atomic::Ordering::SeqCst)
    }
    pub fn get_and_add(&self, d: i64) -> i64 {
        self.v.fetch_add(d, std::sync::atomic::Ordering::SeqCst)
    }
    pub fn add_and_get(&self, d: i64) -> i64 {
        self.v.fetch_add(d, std::sync::atomic::Ordering::SeqCst) + d
    }
    pub fn get_and_increment(&self) -> i64 {
        self.get_and_add(1)
    }
    pub fn increment_and_get(&self) -> i64 {
        self.add_and_get(1)
    }
    pub fn get_and_decrement(&self) -> i64 {
        self.get_and_add(-1)
    }
    pub fn decrement_and_get(&self) -> i64 {
        self.add_and_get(-1)
    }
    pub fn compare_and_set(&self, expect: i64, update: i64) -> bool {
        self.v
            .compare_exchange(
                expect,
                update,
                std::sync::atomic::Ordering::SeqCst,
                std::sync::atomic::Ordering::SeqCst,
            )
            .is_ok()
    }
    pub fn int_value(&self) -> i32 {
        self.get() as i32
    }
    pub fn long_value(&self) -> i64 {
        self.get()
    }
    pub fn double_value(&self) -> f64 {
        self.get() as f64
    }
}
value_display!(JavaAtomicLong, get());

/// `java.util.concurrent.atomic.AtomicBoolean` over `AtomicBool`.
#[derive(Debug, Default)]
pub struct JavaAtomicBoolean {
    v: std::sync::atomic::AtomicBool,
}
impl Clone for JavaAtomicBoolean {
    fn clone(&self) -> Self {
        JavaAtomicBoolean { v: std::sync::atomic::AtomicBool::new(self.get()) }
    }
}
value_eq_hash!(JavaAtomicBoolean, get());
impl JavaAtomicBoolean {
    pub fn new() -> Self {
        JavaAtomicBoolean { v: std::sync::atomic::AtomicBool::new(false) }
    }
    pub fn new_1(initial: bool) -> Self {
        JavaAtomicBoolean { v: std::sync::atomic::AtomicBool::new(initial) }
    }
    pub fn get(&self) -> bool {
        self.v.load(std::sync::atomic::Ordering::SeqCst)
    }
    pub fn set(&self, b: bool) {
        self.v.store(b, std::sync::atomic::Ordering::SeqCst)
    }
    pub fn get_and_set(&self, b: bool) -> bool {
        self.v.swap(b, std::sync::atomic::Ordering::SeqCst)
    }
    pub fn compare_and_set(&self, expect: bool, update: bool) -> bool {
        self.v
            .compare_exchange(
                expect,
                update,
                std::sync::atomic::Ordering::SeqCst,
                std::sync::atomic::Ordering::SeqCst,
            )
            .is_ok()
    }
}
value_display!(JavaAtomicBoolean, get());

#[cfg(test)]
mod atomic_tests {
    use super::*;
    #[test]
    fn atomic_integer_ops() {
        let a = JavaAtomicInteger::new_1(5);
        assert_eq!(a.get(), 5);
        assert_eq!(a.increment_and_get(), 6);
        assert_eq!(a.get_and_increment(), 6);
        assert_eq!(a.get(), 7);
        assert_eq!(a.add_and_get(3), 10);
        assert!(a.compare_and_set(10, 0));
        assert_eq!(a.get(), 0);
    }
    #[test]
    fn atomic_long_and_bool() {
        let l = JavaAtomicLong::new();
        assert_eq!(l.increment_and_get(), 1);
        let b = JavaAtomicBoolean::new_1(true);
        assert!(b.get_and_set(false));
        assert!(!b.get());
    }
}
