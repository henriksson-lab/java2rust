// Shared trait-boilerplate macros for the runtime carriers. Included once, before
// the carrier fragments, in BOTH the shipped `JAVA_RUNTIME` concat and the
// `java_runtime_compiles` compile-check (header.rs can't host them — it's excluded
// from the compile-check). Each expands to exactly the impls the carriers used to
// hand-roll; `#[allow(unused_macros)]` because a given crate may not map every
// carrier (e.g. the atomics are compile-checked but not shipped).

/// `PartialEq`/`Eq`/`Hash` keyed on a value accessor (`self.<acc>`), e.g.
/// `value_eq_hash!(JavaCRC32, crc.get());` -> compare/hash `self.crc.get()`.
#[allow(unused_macros)]
macro_rules! value_eq_hash {
    ($t:ty, $($acc:tt)+) => {
        impl PartialEq for $t {
            fn eq(&self, other: &Self) -> bool {
                self.$($acc)+ == other.$($acc)+
            }
        }
        impl Eq for $t {}
        impl std::hash::Hash for $t {
            fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
                self.$($acc)+.hash(state);
            }
        }
    };
}

/// `Display` of a value accessor: `value_display!(JavaAtomicInteger, get());`.
#[allow(unused_macros)]
macro_rules! value_display {
    ($t:ty, $($acc:tt)+) => {
        impl std::fmt::Display for $t {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(f, "{}", self.$($acc)+)
            }
        }
    };
}

/// `PartialEq`/`Eq`/`Hash` by `Rc` pointer identity on a field (the shared-cursor
/// IO carriers): `rc_identity_eq_hash!(JavaReader, inner);`.
#[allow(unused_macros)]
macro_rules! rc_identity_eq_hash {
    ($t:ty, $field:ident) => {
        impl PartialEq for $t {
            fn eq(&self, other: &Self) -> bool {
                Rc::ptr_eq(&self.$field, &other.$field)
            }
        }
        impl Eq for $t {}
        impl std::hash::Hash for $t {
            fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
                (Rc::as_ptr(&self.$field) as *const () as usize).hash(state);
            }
        }
    };
}

/// Trivial always-equal `PartialEq`/`Eq` + no-op `Hash` for a carrier that is
/// never a meaningful key (`noop_eq_hash!(JavaInflater);`).
#[allow(unused_macros)]
macro_rules! noop_eq_hash {
    ($t:ty) => {
        impl PartialEq for $t {
            fn eq(&self, _other: &Self) -> bool {
                true
            }
        }
        impl Eq for $t {}
        impl std::hash::Hash for $t {
            fn hash<H: std::hash::Hasher>(&self, _state: &mut H) {}
        }
    };
}

/// No-op `Display` (`noop_display!(JavaWriter);` -> writes nothing).
#[allow(unused_macros)]
macro_rules! noop_display {
    ($t:ty) => {
        impl std::fmt::Display for $t {
            fn fmt(&self, _f: &mut std::fmt::Formatter) -> std::fmt::Result {
                Ok(())
            }
        }
    };
}
