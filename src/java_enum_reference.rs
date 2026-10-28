/// Source enum capabilities beyond the Java generic Enum bound. These hooks
/// retain the declaration's actual singleton identity and virtual behavior.
pub trait JavaSourceEnum: JavaEnum + JavaReferenceIdentity {
    fn java_enum_declaring_class_name() -> &'static str;
    fn java_enum_runtime_class_name(&self) -> &'static str;
    fn java_enum_from_ordinal(ordinal: i32) -> Self;
    fn java_enum_reflection_values() -> Option<Vec<Self>>;
    fn java_enum_to_string_result(&self) -> Result<Option<JavaLogicalString>, JavaException>;
}

/// An enum constant can be projected from its ordinal because source enum
/// values already refer to canonical, declaration-owned payloads. No Java
/// object payload is copied into this metadata, and callbacks run without a
/// metadata/global registry loan.
#[derive(Clone)]
pub struct JavaEnumReference(std::sync::Arc<JavaEnumReferenceData>);

struct JavaEnumReferenceData {
    declaring_class: &'static str,
    runtime_class: &'static str,
    name: &'static str,
    ordinal: i32,
    identity: usize,
    original: fn(i32) -> Box<dyn std::any::Any>,
    display: fn(i32) -> Result<Option<JavaLogicalString>, JavaException>,
    constants: fn() -> Option<Vec<JavaEnumReference>>,
}

impl JavaEnumReference {
    pub fn from_source<E: JavaSourceEnum>(value: &E) -> Self {
        fn original<E: JavaSourceEnum>(ordinal: i32) -> Box<dyn std::any::Any> {
            Box::new(E::java_enum_from_ordinal(ordinal))
        }
        fn display<E: JavaSourceEnum>(
            ordinal: i32,
        ) -> Result<Option<JavaLogicalString>, JavaException> {
            E::java_enum_from_ordinal(ordinal).java_enum_to_string_result()
        }
        Self(std::sync::Arc::new(JavaEnumReferenceData {
            declaring_class: E::java_enum_declaring_class_name(),
            runtime_class: value.java_enum_runtime_class_name(),
            name: value.name(),
            ordinal: value.ordinal(),
            identity: value.java_reference_identity() as usize,
            original: original::<E>,
            display: display::<E>,
            constants: java_source_enum_reference_constants::<E>,
        }))
    }
    pub fn identity(&self) -> usize {
        self.0.identity
    }
    pub fn declaring_class_name(&self) -> &'static str {
        self.0.declaring_class
    }
    pub fn runtime_class_name(&self) -> &'static str {
        self.0.runtime_class
    }
    pub fn name(&self) -> JavaLogicalString {
        java_enum_name_string(self.0.name)
    }
    pub fn ordinal(&self) -> i32 {
        self.0.ordinal
    }
    pub fn declaring_class(&self) -> JavaClass<()> {
        let mut class = JavaClass::literal(self.declaring_class_name());
        class.__enum_constants = Some(self.0.constants);
        class
    }
    pub fn runtime_class(&self) -> JavaClass<()> {
        if self.runtime_class_name() == self.declaring_class_name() {
            return self.declaring_class();
        }
        JavaClass::literal(self.runtime_class_name())
    }
    pub fn original(&self) -> Box<dyn std::any::Any> {
        (self.0.original)(self.0.ordinal)
    }
    pub fn cast_result<T: 'static>(&self) -> Result<T, JavaException> {
        self.original()
            .downcast::<T>()
            .map(|value| *value)
            .map_err(|_| {
                JavaException::new(
                    "ClassCastException",
                    format!(
                        "Invalid cast of enum {}.{}",
                        self.0.declaring_class, self.0.name
                    ),
                )
            })
    }
    pub fn to_string_result(&self) -> Result<Option<JavaLogicalString>, JavaException> {
        (self.0.display)(self.0.ordinal)
    }
    pub fn compare_to_result(&self, other: &Self) -> Result<i32, JavaException> {
        if self.0.declaring_class != other.0.declaring_class {
            return Err(JavaException::new(
                "ClassCastException",
                "Different enum declarations",
            ));
        }
        Ok(self.ordinal() - other.ordinal())
    }
}
impl PartialEq for JavaEnumReference {
    fn eq(&self, other: &Self) -> bool {
        self.identity() == other.identity()
    }
}
impl Eq for JavaEnumReference {}
impl std::hash::Hash for JavaEnumReference {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        std::hash::Hash::hash(&self.identity(), state)
    }
}
impl std::fmt::Debug for JavaEnumReference {
    fn fmt(&self, out: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        out.debug_struct("JavaEnumReference")
            .field("class", &self.0.declaring_class)
            .field("name", &self.0.name)
            .finish()
    }
}
impl JavaReferenceIdentity for JavaEnumReference {
    fn java_reference_identity(&self) -> *const () {
        self.identity() as *const ()
    }
}

impl<T> JavaClass<T> {
    pub fn enum_array_class_name(&self) -> String {
        format!("[L{};", self.name)
    }
    /// Attach a factory without reading constants or initializing the enum.
    pub fn with_enum_metadata<E: JavaSourceEnum>(mut self) -> Self {
        self.__enum_constants = Some(java_source_enum_reference_constants::<E>);
        self
    }
    /// Class generic arguments describe a use-site view, not a new Class.
    pub fn retarget<U>(&self) -> JavaClass<U> {
        JavaClass {
            name: self.name.clone(),
            __enum_constants: self.__enum_constants,
            _marker: std::marker::PhantomData,
        }
    }
    pub fn enum_reference_constants_result(
        &self,
    ) -> Result<Option<Vec<JavaEnumReference>>, JavaException> {
        if let Some(factory) = self.__enum_constants {
            return Ok(factory());
        }
        if java_runtime_class_enum_kind(&self.name) == Some(false) {
            return Ok(None);
        }
        Err(JavaException::new(
            "UnsupportedOperationException",
            format!("Enum metadata factory is unavailable for {}", self.name),
        ))
    }
}

pub fn java_enum_original_result<T: 'static>(
    value: &JavaEnumReference,
) -> Result<T, JavaException> {
    value.cast_result()
}

pub fn java_enum_original_erased_result(
    value: &JavaEnumReference,
) -> Result<Box<dyn std::any::Any>, JavaException> {
    Ok(value.original())
}

pub fn java_class_enum_constants_result<T, C>(
    class: &JavaClass<C>,
    project: fn(&JavaEnumReference) -> Result<T, JavaException>,
) -> Result<Option<Vec<Option<T>>>, JavaException> {
    class
        .enum_reference_constants_result()?
        .map(|values| {
            values
                .iter()
                .map(|value| project(value).map(Some))
                .collect()
        })
        .transpose()
}

fn java_source_enum_reference_constants<E: JavaSourceEnum>() -> Option<Vec<JavaEnumReference>> {
    E::java_enum_reflection_values()
        .map(|values| values.iter().map(JavaEnumReference::from_source).collect())
}
