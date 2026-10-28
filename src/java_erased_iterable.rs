/// A local operation projection of one original Java Iterable reference.
/// Factories are called at iterator(), never at erasure or a type test.
#[derive(Clone)]
pub struct JavaErasedIterableView {
    identity: usize,
    factory: std::rc::Rc<dyn Fn() -> Result<JavaErasedIterator, JavaException>>,
}
impl std::fmt::Debug for JavaErasedIterableView {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("JavaErasedIterableView")
            .field("identity", &self.identity)
            .finish()
    }
}
impl JavaReferenceIdentity for JavaErasedIterableView {
    fn java_reference_identity(&self) -> *const () {
        self.identity as *const ()
    }
}
impl JavaErasedIterableView {
    pub fn from_factory<F>(identity: *const (), factory: F) -> Self
    where
        F: Fn() -> Result<JavaErasedIterator, JavaException> + 'static,
    {
        Self {
            identity: identity as usize,
            factory: std::rc::Rc::new(factory),
        }
    }
    pub fn iterator_result(&self) -> Result<JavaErasedIterator, JavaException> {
        (self.factory)()
    }
}

/// Cursor aliases share operations and cursor state owned by the producer.
/// No RefCell or lock guard is introduced around a Java operation invocation.
#[derive(Clone)]
pub struct JavaErasedIterator {
    has_next: std::rc::Rc<dyn Fn() -> Result<bool, JavaException>>,
    next: std::rc::Rc<dyn Fn() -> Result<Option<Box<dyn std::any::Any>>, JavaException>>,
    remove: std::rc::Rc<dyn Fn() -> Result<(), JavaException>>,
}
impl JavaErasedIterator {
    pub fn from_operations<H, N, R>(has_next: H, next: N, remove: R) -> Self
    where
        H: Fn() -> Result<bool, JavaException> + 'static,
        N: Fn() -> Result<Option<Box<dyn std::any::Any>>, JavaException> + 'static,
        R: Fn() -> Result<(), JavaException> + 'static,
    {
        Self {
            has_next: std::rc::Rc::new(has_next),
            next: std::rc::Rc::new(next),
            remove: std::rc::Rc::new(remove),
        }
    }
    pub fn has_next_result(&self) -> Result<bool, JavaException> {
        (self.has_next)()
    }
    pub fn next_result(&self) -> Result<Option<Box<dyn std::any::Any>>, JavaException> {
        (self.next)()
    }
    pub fn remove_result(&self) -> Result<(), JavaException> {
        (self.remove)()
    }
}

#[derive(Clone, Debug)]
pub enum JavaErasedIterableProjection {
    Unknown,
    Absent,
    Present(JavaErasedIterableView),
}
impl JavaErasedIterableProjection {
    pub fn type_test(&self) -> Option<bool> {
        match self {
            Self::Unknown => None,
            Self::Absent => Some(false),
            Self::Present(_) => Some(true),
        }
    }
    pub fn try_cast(&self) -> Option<Result<JavaErasedIterableView, JavaException>> {
        match self {
            Self::Unknown => None,
            Self::Absent => Some(Err(JavaException::new(
                "ClassCastException",
                "java.lang.Iterable".to_string(),
            ))),
            Self::Present(view) => Some(Ok(view.clone())),
        }
    }
}

pub fn java_erased_iterable_type_test_result(
    value: Option<&dyn std::any::Any>,
) -> Result<bool, JavaException> {
    let Some(value) = value else {
        return Ok(false);
    };
    if let Some(value) = value.downcast_ref::<Box<dyn std::any::Any>>() {
        return java_erased_iterable_type_test_result(Some(value.as_ref()));
    }
    if let Some(value) = value.downcast_ref::<JavaErasedValue>() {
        if let Some(result) = value.iterable_projection().type_test() {
            return Ok(result);
        }
    }
    if let Some(class) = java_erased_any_class_known(value) {
        return java_erased_iterable_class_type_test(&class.get_name());
    }
    Err(JavaException::new(
        "UnsupportedOperationException",
        "Iterable capability is not registered".to_string(),
    ))
}

pub fn java_erased_iterable_cast_result(
    value: Option<&dyn std::any::Any>,
) -> Result<Option<JavaErasedIterableView>, JavaException> {
    let Some(value) = value else {
        return Ok(None);
    };
    if let Some(value) = value.downcast_ref::<Box<dyn std::any::Any>>() {
        return java_erased_iterable_cast_result(Some(value.as_ref()));
    }
    if let Some(value) = value.downcast_ref::<JavaErasedValue>() {
        if let Some(result) = value.iterable_projection().try_cast() {
            return result.map(Some);
        }
    }
    if let Some(class) = java_erased_any_class_known(value) {
        if !java_erased_iterable_class_type_test(&class.get_name())? {
            return Err(JavaException::new(
                "ClassCastException",
                "java.lang.Iterable".to_string(),
            ));
        }
    }
    Err(JavaException::new(
        "UnsupportedOperationException",
        "Iterable operations are not registered".to_string(),
    ))
}

fn java_erased_iterable_class_type_test(class: &str) -> Result<bool, JavaException> {
    // These concrete JDK carriers are proved non-Iterable. This is capability
    // evidence, not a claim that their entire reflection ancestry is modeled.
    if class.starts_with('[')
        || matches!(
            class,
            "java.lang.Object"
                | "java.lang.String"
                | "java.lang.Boolean"
                | "java.lang.Byte"
                | "java.lang.Short"
                | "java.lang.Integer"
                | "java.lang.Long"
                | "java.lang.Float"
                | "java.lang.Double"
                | "java.lang.Character"
                | "java.lang.Void"
        )
    {
        return Ok(false);
    }
    java_runtime_class_is_assignable("java.lang.Iterable", class)
}
