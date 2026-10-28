// Shared native allocations can have several typed projections of one Java
// object. Identity and the root owner's lifetime must survive outside payload
// loans; a projection's wrapper address is not a new Java identity.
// Payload access policy stays explicit in T: immutable operation tables need
// no loan, while mutable Java state uses RefCell<T> or Mutex<T>.
// Reference copies name Rust's Clone implementation explicitly: a generated
// Java interface can itself declare clone() for Rc/Arc-backed implementors.
// Neither associated Rc::clone nor method syntax is an unambiguous Rust copy
// in that scope; typed UFCS also preserves the concrete type before Any erasure.

struct JavaLocalNativeIdentity {
    pointer: usize,
    // Keep the original allocation alive, including when the view's payload
    // does not itself need the original type. Further projections retain this
    // same root rather than a chain of identity-only wrappers.
    _owner: std::rc::Rc<dyn std::any::Any>,
}

struct JavaLocalNativeAllocation<T> {
    value: T,
    identity: Option<JavaLocalNativeIdentity>,
}

pub struct JavaLocalNativeRef<T> {
    allocation: std::rc::Rc<JavaLocalNativeAllocation<T>>,
}

/// Weak ownership of a particular native view allocation, for callback-cycle
/// prevention. Java weak-reference reachability must follow the canonical
/// object's root owner, which can outlive any one typed operation view.
pub struct JavaLocalNativeWeak<T> {
    allocation: std::rc::Weak<JavaLocalNativeAllocation<T>>,
}

impl<T> Clone for JavaLocalNativeWeak<T> {
    fn clone(&self) -> Self {
        Self {
            allocation: <std::rc::Weak<JavaLocalNativeAllocation<T>> as std::clone::Clone>::clone(
                &self.allocation,
            ),
        }
    }
}

impl<T> JavaLocalNativeWeak<T> {
    pub fn upgrade(&self) -> Option<JavaLocalNativeRef<T>> {
        self.allocation
            .upgrade()
            .map(|allocation| JavaLocalNativeRef { allocation })
    }
}

impl<T> Clone for JavaLocalNativeRef<T> {
    fn clone(&self) -> Self {
        Self {
            allocation: <std::rc::Rc<JavaLocalNativeAllocation<T>> as std::clone::Clone>::clone(
                &self.allocation,
            ),
        }
    }
}

impl<T> JavaLocalNativeRef<T> {
    pub fn downgrade(&self) -> JavaLocalNativeWeak<T> {
        JavaLocalNativeWeak {
            allocation: std::rc::Rc::downgrade(&self.allocation),
        }
    }

    /// Bridge an existing native allocation without changing its Java identity.
    pub fn projected_from_rc<S: 'static>(source: &std::rc::Rc<S>, value: T) -> Self {
        Self {
            allocation: std::rc::Rc::new(JavaLocalNativeAllocation {
                value,
                identity: Some(JavaLocalNativeIdentity {
                    pointer: std::rc::Rc::as_ptr(source) as usize,
                    _owner: <std::rc::Rc<S> as std::clone::Clone>::clone(source),
                }),
            }),
        }
    }

    /// A local operation table may view a synchronized Java allocation. Keep
    /// its original token and owner; the operation table itself need not be Send.
    pub fn projected_from_arc<S: 'static>(source: &std::sync::Arc<S>, value: T) -> Self {
        Self {
            allocation: std::rc::Rc::new(JavaLocalNativeAllocation {
                value,
                identity: Some(JavaLocalNativeIdentity {
                    pointer: std::sync::Arc::as_ptr(source) as usize,
                    _owner: std::rc::Rc::new(<std::sync::Arc<S> as std::clone::Clone>::clone(
                        source,
                    )),
                }),
            }),
        }
    }

    pub fn new(value: T) -> Self {
        Self {
            allocation: std::rc::Rc::new(JavaLocalNativeAllocation {
                value,
                identity: None,
            }),
        }
    }

    /// Construct the allocation header of a typed view. The view payload must
    /// supply live checked operations; this constructor only establishes
    /// identity and lifetime, never an element conversion or a snapshot.
    pub fn projected<S: 'static>(source: &JavaLocalNativeRef<S>, value: T) -> Self {
        let owner = match &source.allocation.identity {
            Some(identity) => {
                <std::rc::Rc<dyn std::any::Any> as std::clone::Clone>::clone(&identity._owner)
            }
            None => <std::rc::Rc<JavaLocalNativeAllocation<S>> as std::clone::Clone>::clone(
                &source.allocation,
            ) as std::rc::Rc<dyn std::any::Any>,
        };
        Self {
            allocation: std::rc::Rc::new(JavaLocalNativeAllocation {
                value,
                identity: Some(JavaLocalNativeIdentity {
                    pointer: source.identity_ptr() as usize,
                    _owner: owner,
                }),
            }),
        }
    }

    pub fn identity_ptr(&self) -> *const () {
        match &self.allocation.identity {
            Some(identity) => identity.pointer as *const (),
            None => std::rc::Rc::as_ptr(&self.allocation).cast(),
        }
    }
}

impl<T> std::ops::Deref for JavaLocalNativeRef<T> {
    type Target = T;
    fn deref(&self) -> &Self::Target {
        &self.allocation.value
    }
}

impl<T> AsRef<T> for JavaLocalNativeRef<T> {
    fn as_ref(&self) -> &T {
        &self.allocation.value
    }
}

struct JavaSyncNativeIdentity {
    pointer: usize,
    _owner: std::sync::Arc<dyn std::any::Any + Send + Sync>,
}

struct JavaSyncNativeAllocation<T> {
    value: T,
    identity: Option<JavaSyncNativeIdentity>,
}

pub struct JavaSyncNativeRef<T> {
    allocation: std::sync::Arc<JavaSyncNativeAllocation<T>>,
}

impl<T> Clone for JavaSyncNativeRef<T> {
    fn clone(&self) -> Self {
        Self {
            allocation: <std::sync::Arc<JavaSyncNativeAllocation<T>> as std::clone::Clone>::clone(
                &self.allocation,
            ),
        }
    }
}

impl<T> JavaSyncNativeRef<T> {
    pub fn projected_from_arc<S: Send + Sync + 'static>(
        source: &std::sync::Arc<S>,
        value: T,
    ) -> Self {
        Self {
            allocation: std::sync::Arc::new(JavaSyncNativeAllocation {
                value,
                identity: Some(JavaSyncNativeIdentity {
                    pointer: std::sync::Arc::as_ptr(source) as usize,
                    _owner: <std::sync::Arc<S> as std::clone::Clone>::clone(source),
                }),
            }),
        }
    }

    pub fn new(value: T) -> Self {
        Self {
            allocation: std::sync::Arc::new(JavaSyncNativeAllocation {
                value,
                identity: None,
            }),
        }
    }

    pub fn projected<S: Send + Sync + 'static>(source: &JavaSyncNativeRef<S>, value: T) -> Self {
        let owner = match &source.allocation.identity {
            Some(identity) => {
                <std::sync::Arc<dyn std::any::Any + Send + Sync> as std::clone::Clone>::clone(
                    &identity._owner,
                )
            }
            None => <std::sync::Arc<JavaSyncNativeAllocation<S>> as std::clone::Clone>::clone(
                &source.allocation,
            ) as std::sync::Arc<dyn std::any::Any + Send + Sync>,
        };
        Self {
            allocation: std::sync::Arc::new(JavaSyncNativeAllocation {
                value,
                identity: Some(JavaSyncNativeIdentity {
                    pointer: source.identity_ptr() as usize,
                    _owner: owner,
                }),
            }),
        }
    }

    pub fn identity_ptr(&self) -> *const () {
        match &self.allocation.identity {
            Some(identity) => identity.pointer as *const (),
            None => std::sync::Arc::as_ptr(&self.allocation).cast(),
        }
    }
}

impl<T> std::ops::Deref for JavaSyncNativeRef<T> {
    type Target = T;
    fn deref(&self) -> &Self::Target {
        &self.allocation.value
    }
}

impl<T> AsRef<T> for JavaSyncNativeRef<T> {
    fn as_ref(&self) -> &T {
        &self.allocation.value
    }
}
