// Shared immutable Java reference identity. String carriers add interning;
// other immutable mapped objects share the same allocation/clone mechanism.

#[derive(Debug, PartialEq, Eq, Hash)]
pub struct JavaStringIdentity<P: ?Sized>(std::sync::Arc<P>);

// Preserve the emitted String runtime's public name while exposing the
// payload-independent contract to linked runtime providers.
pub type JavaImmutableReference<P> = JavaStringIdentity<P>;

impl<P: ?Sized> std::ops::Deref for JavaStringIdentity<P> {
    type Target = P;
    fn deref(&self) -> &P {
        self.payload()
    }
}

impl<P: ?Sized> Clone for JavaStringIdentity<P> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

impl<P: ?Sized> Default for JavaStringIdentity<P>
where
    std::sync::Arc<P>: Default,
{
    fn default() -> Self {
        Self(Default::default())
    }
}

impl<P: ?Sized> JavaStringIdentity<P> {
    pub fn from_payload(payload: std::sync::Arc<P>) -> Self {
        Self(payload)
    }
    pub fn payload(&self) -> &P {
        &self.0
    }
    pub fn java_identity_eq(&self, other: &Self) -> bool {
        std::sync::Arc::ptr_eq(&self.0, &other.0)
    }
    pub fn identity_ptr(&self) -> *const () {
        std::sync::Arc::as_ptr(&self.0) as *const ()
    }
    pub fn java_equals(&self, other: &Self) -> bool
    where
        P: PartialEq,
    {
        self.payload() == other.payload()
    }
}

impl<P: ?Sized + std::borrow::ToOwned + Eq + std::hash::Hash> JavaStringIdentity<P>
where
    P::Owned: Eq + std::hash::Hash,
    std::sync::Arc<P>: From<P::Owned>,
{
    // The concrete String carrier owns the process-wide pool. Keeping it an
    // explicit argument avoids one generic static accidentally mixing pools
    // for different payloads, or introducing a second object identity model.
    pub fn intern(
        value: &P,
        pool: &std::sync::Mutex<std::collections::HashMap<P::Owned, std::sync::Arc<P>>>,
    ) -> Self {
        Self::intern_with(value, pool, || std::sync::Arc::from(value.to_owned()))
    }

    /// String.intern must retain this receiver when it wins an empty pool
    /// entry. A literal factory has no receiver, but shares the same pool path.
    pub fn intern_reference(
        &self,
        pool: &std::sync::Mutex<std::collections::HashMap<P::Owned, std::sync::Arc<P>>>,
    ) -> Self {
        Self::intern_with(self.payload(), pool, || self.0.clone())
    }

    fn intern_with(
        value: &P,
        pool: &std::sync::Mutex<std::collections::HashMap<P::Owned, std::sync::Arc<P>>>,
        allocate: impl FnOnce() -> std::sync::Arc<P>,
    ) -> Self {
        let mut table = pool.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        if let Some(existing) = table.get(value) {
            return Self(existing.clone());
        }
        let owned = value.to_owned();
        let payload = allocate();
        table.insert(owned, payload.clone());
        Self(payload)
    }
}
