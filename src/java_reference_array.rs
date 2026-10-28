/// Operations over one original Java reference-array allocation. This local
/// view can retain either an Rc or an Arc owner; it does not change that owner's
/// domain or claim that an erased element is Send.
#[derive(Clone)]
pub struct JavaReferenceArrayView {
    operations: std::rc::Rc<JavaReferenceArrayOperations>,
}

struct JavaReferenceArrayOperations {
    identity: usize,
    class: &'static str,
    length: Box<dyn Fn() -> usize>,
    get: Box<dyn Fn(i32) -> Result<Option<Box<dyn std::any::Any>>, JavaException>>,
    set: Box<dyn Fn(i32, Option<Box<dyn std::any::Any>>) -> Result<(), JavaException>>,
}

impl std::fmt::Debug for JavaReferenceArrayView {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("JavaReferenceArrayView")
            .field("class", &self.operations.class)
            .field("identity", &self.operations.identity)
            .finish()
    }
}

impl JavaReferenceIdentity for JavaReferenceArrayView {
    fn java_reference_identity(&self) -> *const () {
        self.operations.identity as *const ()
    }
}

fn java_reference_array_index(index: i32, length: usize) -> Result<usize, JavaException> {
    if index < 0 || index as usize >= length {
        Err(JavaException::new(
            "ArrayIndexOutOfBoundsException",
            index.to_string(),
        ))
    } else {
        Ok(index as usize)
    }
}

fn java_reference_array_class(identity: usize) -> Result<Option<&'static str>, JavaException> {
    let Some(class) = java_array_runtime_class(identity) else {
        return Ok(None);
    };
    let Some(component) = java_runtime_class_component(class) else {
        return Err(JavaException::new(
            "ClassCastException",
            "not an array".to_string(),
        ));
    };
    if java_runtime_class_primitive_code(&component).is_some() {
        return Err(JavaException::new(
            "ClassCastException",
            "primitive array is not Object[]".to_string(),
        ));
    }
    Ok(Some(class))
}

fn java_reference_array_check_store(
    class: &str,
    value: Option<&dyn std::any::Any>,
) -> Result<(), JavaException> {
    if let Some(value) = value {
        let component =
            java_runtime_class_component(class).expect("reference array class evidence");
        // Every non-null Java reference is an Object. Its ancestry need not
        // be registered to prove this positive store check.
        if component == "java.lang.Object" {
            return Ok(());
        }
        let actual = java_erased_any_class(value).get_name();
        if !java_runtime_class_is_assignable(&component, &actual)? {
            return Err(JavaException::new("ArrayStoreException", actual));
        }
    }
    Ok(())
}

impl JavaReferenceArrayView {
    pub fn runtime_class(&self) -> &'static str {
        self.operations.class
    }
    pub fn length(&self) -> i32 {
        (self.operations.length)() as i32
    }
    pub fn get_result(&self, index: i32) -> Result<Option<Box<dyn std::any::Any>>, JavaException> {
        (self.operations.get)(index)
    }
    pub fn set_result(
        &self,
        index: i32,
        value: Option<Box<dyn std::any::Any>>,
    ) -> Result<(), JavaException> {
        (self.operations.set)(index, value)
    }

    /// copy is the compiler's Java reference-copy operation, never a Java
    /// callback. It alone runs under the short payload loan. Codecs run after
    /// that loan has ended and may use ordinary checked carrier conversions.
    /// None means missing class evidence, not a failed Java type test.
    pub fn from_local_array<T: 'static, E, D>(
        owner: std::rc::Rc<std::cell::RefCell<Vec<T>>>,
        copy: fn(&T) -> T,
        encode: E,
        decode: D,
    ) -> Result<Option<Self>, JavaException>
    where
        E: Fn(T) -> Option<Box<dyn std::any::Any>> + 'static,
        D: Fn(Option<Box<dyn std::any::Any>>) -> Result<T, JavaException> + 'static,
    {
        let identity = std::rc::Rc::as_ptr(&owner) as usize;
        let Some(class) = java_reference_array_class(identity)? else {
            return Ok(None);
        };
        let read = owner.clone();
        let write = owner.clone();
        Ok(Some(Self {
            operations: std::rc::Rc::new(JavaReferenceArrayOperations {
                identity,
                class,
                length: Box::new(move || owner.borrow().len()),
                get: Box::new(move |index| {
                    let value = {
                        let values = read.borrow();
                        let index = java_reference_array_index(index, values.len())?;
                        copy(&values[index])
                    };
                    Ok(encode(value))
                }),
                set: Box::new(move |index, value| {
                    java_reference_array_index(index, write.borrow().len())?;
                    java_reference_array_check_store(class, value.as_deref())?;
                    let value = decode(value)?;
                    let mut values = write.borrow_mut();
                    let index = java_reference_array_index(index, values.len())?;
                    values[index] = value;
                    Ok(())
                }),
            }),
        }))
    }

    /// Project a synchronized owner into local operations without copying its
    /// payload, converting it into Rc storage, or retaining a mutex guard.
    pub fn from_shared_array<T: 'static, E, D>(
        owner: std::sync::Arc<std::sync::Mutex<Vec<T>>>,
        copy: fn(&T) -> T,
        encode: E,
        decode: D,
    ) -> Result<Option<Self>, JavaException>
    where
        E: Fn(T) -> Option<Box<dyn std::any::Any>> + 'static,
        D: Fn(Option<Box<dyn std::any::Any>>) -> Result<T, JavaException> + 'static,
    {
        let identity = std::sync::Arc::as_ptr(&owner) as usize;
        let Some(class) = java_reference_array_class(identity)? else {
            return Ok(None);
        };
        let read = owner.clone();
        let write = owner.clone();
        Ok(Some(Self {
            operations: std::rc::Rc::new(JavaReferenceArrayOperations {
                identity,
                class,
                length: Box::new(move || owner.lock().unwrap().len()),
                get: Box::new(move |index| {
                    let value = {
                        let values = read.lock().unwrap();
                        let index = java_reference_array_index(index, values.len())?;
                        copy(&values[index])
                    };
                    Ok(encode(value))
                }),
                set: Box::new(move |index, value| {
                    java_reference_array_index(index, write.lock().unwrap().len())?;
                    java_reference_array_check_store(class, value.as_deref())?;
                    let value = decode(value)?;
                    let mut values = write.lock().unwrap();
                    let index = java_reference_array_index(index, values.len())?;
                    values[index] = value;
                    Ok(())
                }),
            }),
        }))
    }
}

#[derive(Clone, Debug)]
pub enum JavaReferenceArrayProjection {
    Unknown,
    Absent,
    Present(JavaReferenceArrayView),
}

impl JavaReferenceArrayProjection {
    pub fn type_test(&self) -> Option<bool> {
        match self {
            Self::Unknown => None,
            Self::Absent => Some(false),
            Self::Present(_) => Some(true),
        }
    }
    pub fn try_cast(&self) -> Option<Result<JavaReferenceArrayView, JavaException>> {
        match self {
            Self::Unknown => None,
            Self::Absent => Some(Err(JavaException::new(
                "ClassCastException",
                "Object[]".to_string(),
            ))),
            Self::Present(view) => Some(Ok(view.clone())),
        }
    }
}

/// A Java cast of null succeeds. Null checking belongs to the subsequent
/// array operation, after evaluation of its index (and simple-store RHS).
pub fn java_reference_array_cast_result(
    value: Option<&dyn std::any::Any>,
) -> Result<Option<JavaReferenceArrayView>, JavaException> {
    let Some(value) = value else {
        return Ok(None);
    };
    if let Some(value) = value.downcast_ref::<Box<dyn std::any::Any>>() {
        return java_reference_array_cast_result(Some(value.as_ref()));
    }
    if let Some(value) = value.downcast_ref::<JavaErasedValue>() {
        if let Some(result) = value.reference_array_projection().try_cast() {
            return result.map(Some);
        }
    }
    if let Some(class) = java_erased_any_class_known(value) {
        let component = java_runtime_class_component(&class.get_name());
        if component.is_none()
            || component
                .as_ref()
                .is_some_and(|component| java_runtime_class_primitive_code(component).is_some())
        {
            return Err(JavaException::new(
                "ClassCastException",
                "Object[]".to_string(),
            ));
        }
    }
    Err(JavaException::new(
        "UnsupportedOperationException",
        "reference-array operations are not registered".to_string(),
    ))
}

pub fn java_reference_array_non_null(
    value: Option<JavaReferenceArrayView>,
) -> Result<JavaReferenceArrayView, JavaException> {
    value.ok_or_else(|| JavaException::new("NullPointerException", "Java null array".to_string()))
}

pub fn java_reference_array_copy_value(
    value: &Option<Box<dyn std::any::Any>>,
) -> Option<Box<dyn std::any::Any>> {
    java_reference_array_copy_erased(value)
}

fn java_reference_array_payload(value: &dyn std::any::Any) -> &dyn std::any::Any {
    if let Some(value) = value.downcast_ref::<JavaErasedValue>() {
        java_reference_array_payload(value.as_any())
    } else if let Some(value) = value.downcast_ref::<Box<dyn std::any::Any>>() {
        java_reference_array_payload(value.as_ref())
    } else {
        value
    }
}

fn java_reference_array_encode_typed<T: std::any::Any>(
    value: Option<T>,
) -> Option<Box<dyn std::any::Any>> {
    value.map(|value| Box::new(value) as Box<dyn std::any::Any>)
}

fn java_reference_array_decode_typed<T: Clone + std::any::Any>(
    value: Option<Box<dyn std::any::Any>>,
) -> Result<Option<T>, JavaException> {
    value
        .map(|value| {
            java_reference_array_payload(value.as_ref())
                .downcast_ref::<T>()
                .cloned()
                .ok_or_else(|| {
                    JavaException::new(
                        "ArrayStoreException",
                        "invalid reference-array value carrier".to_string(),
                    )
                })
        })
        .transpose()
}

fn java_reference_array_copy_erased(
    value: &Option<Box<dyn std::any::Any>>,
) -> Option<Box<dyn std::any::Any>> {
    java_clone_optional_erased_object_ref(value)
}

fn java_reference_array_projection(
    view: Result<Option<JavaReferenceArrayView>, JavaException>,
) -> JavaReferenceArrayProjection {
    match view {
        Ok(Some(view)) => JavaReferenceArrayProjection::Present(view),
        Ok(None) => JavaReferenceArrayProjection::Unknown,
        Err(error) => panic!("invalid compiler reference-array capability: {error:?}"),
    }
}

pub fn java_erase_primitive_array_reference<T: JavaReferenceIdentity + std::any::Any>(
    value: T,
) -> Box<dyn std::any::Any> {
    Box::new(
        JavaErasedValue::from_native_reference(value)
            .with_reference_array_projection(JavaReferenceArrayProjection::Absent),
    )
}

/// Typed codec for final reference components. Covariant/source hierarchy
/// components require their normal Java upcast/downcast adapter instead.
pub fn java_erase_local_final_reference_array<T: Clone + std::any::Any>(
    owner: std::rc::Rc<std::cell::RefCell<Vec<Option<T>>>>,
) -> Box<dyn std::any::Any> {
    let projection = java_reference_array_projection(JavaReferenceArrayView::from_local_array(
        owner.clone(),
        Clone::clone,
        java_reference_array_encode_typed::<T>,
        java_reference_array_decode_typed::<T>,
    ));
    Box::new(
        JavaErasedValue::from_native_reference(owner).with_reference_array_projection(projection),
    )
}

pub fn java_erase_shared_final_reference_array<T: Clone + std::any::Any>(
    owner: std::sync::Arc<std::sync::Mutex<Vec<Option<T>>>>,
) -> Box<dyn std::any::Any> {
    let projection = java_reference_array_projection(JavaReferenceArrayView::from_shared_array(
        owner.clone(),
        Clone::clone,
        java_reference_array_encode_typed::<T>,
        java_reference_array_decode_typed::<T>,
    ));
    Box::new(
        JavaErasedValue::from_native_reference(owner).with_reference_array_projection(projection),
    )
}

pub fn java_erase_local_object_array(
    owner: std::rc::Rc<std::cell::RefCell<Vec<Option<Box<dyn std::any::Any>>>>>,
) -> Box<dyn std::any::Any> {
    let projection = java_reference_array_projection(JavaReferenceArrayView::from_local_array(
        owner.clone(),
        java_reference_array_copy_erased,
        |value| value,
        Ok,
    ));
    Box::new(
        JavaErasedValue::from_native_reference(owner).with_reference_array_projection(projection),
    )
}

pub fn java_erase_shared_object_array(
    owner: std::sync::Arc<std::sync::Mutex<Vec<Option<Box<dyn std::any::Any>>>>>,
) -> Box<dyn std::any::Any> {
    let projection = java_reference_array_projection(JavaReferenceArrayView::from_shared_array(
        owner.clone(),
        java_reference_array_copy_erased,
        |value| value,
        Ok,
    ));
    Box::new(
        JavaErasedValue::from_native_reference(owner).with_reference_array_projection(projection),
    )
}

pub fn java_reference_array_type_test_result(
    value: Option<&dyn std::any::Any>,
) -> Result<bool, JavaException> {
    let Some(value) = value else {
        return Ok(false);
    };
    if let Some(value) = value.downcast_ref::<Box<dyn std::any::Any>>() {
        return java_reference_array_type_test_result(Some(value.as_ref()));
    }
    if let Some(value) = value.downcast_ref::<JavaErasedValue>() {
        if let Some(result) = value.reference_array_projection().type_test() {
            return Ok(result);
        }
    }
    if let Some(class) = java_erased_any_class_known(value) {
        return Ok(java_runtime_class_component(&class.get_name())
            .is_some_and(|component| java_runtime_class_primitive_code(&component).is_none()));
    }
    Err(JavaException::new(
        "UnsupportedOperationException",
        "reference-array type evidence is not registered".to_string(),
    ))
}

/// Erasing an operation carrier retains its original array identity/class and
/// the same checked operations; the view's implementation allocation is private.
pub fn java_erase_reference_array_view(view: JavaReferenceArrayView) -> Box<dyn std::any::Any> {
    Box::new(JavaErasedValue::from_native_reference(view.clone())
        .with_reference_array_projection(JavaReferenceArrayProjection::Present(view)))
}
