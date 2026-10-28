/// The Java Enum base view keeps the original constant; T is a use-site
/// generic marker and does not own or replace the source enum payload.
pub struct Enum<T = Box<dyn std::any::Any>> {
    reference: JavaEnumReference,
    marker: std::marker::PhantomData<fn() -> T>,
}
impl<T> Enum<T> {
    pub fn from_enum_reference_result(value: &JavaEnumReference) -> Result<Self, JavaException> {
        Ok(Self::from_reference(value.clone()))
    }
    pub fn from_reference(reference: JavaEnumReference) -> Self {
        Self {
            reference,
            marker: std::marker::PhantomData,
        }
    }
    pub fn from_source<E: JavaSourceEnum>(value: &E) -> Self {
        Self::from_reference(JavaEnumReference::from_source(value))
    }
    pub fn retarget<U>(&self) -> Enum<U> {
        Enum::from_reference(self.reference.clone())
    }
    pub fn name(&self) -> JavaLogicalString {
        self.reference.name()
    }
    pub fn ordinal(&self) -> i32 {
        self.reference.ordinal()
    }
    pub fn hash_code(&self) -> i32 {
        self.reference.identity() as i32
    }
    pub fn get_declaring_class(&self) -> JavaClass<()> {
        self.reference.declaring_class()
    }
    pub fn get_class(&self) -> JavaClass<()> {
        self.reference.runtime_class()
    }
    pub fn cast_result<E: 'static>(&self) -> Result<E, JavaException> {
        self.reference.cast_result()
    }
    pub fn original(&self) -> Box<dyn std::any::Any> {
        self.reference.original()
    }
    pub fn enum_reference(&self) -> JavaEnumReference {
        self.reference.clone()
    }
    pub fn to_string(&self) -> Result<Option<JavaLogicalString>, JavaException> {
        self.reference.to_string_result()
    }
    pub fn compare_to_result<U>(&self, other: &Enum<U>) -> Result<i32, JavaException> {
        self.reference.compare_to_result(&other.reference)
    }
}
impl<T> Clone for Enum<T> {
    fn clone(&self) -> Self {
        Self::from_reference(self.reference.clone())
    }
}
impl<T> Default for Enum<T> {
    fn default() -> Self {
        panic!("an uninitialized Java enum reference requires nullable storage")
    }
}
impl<T, U> PartialEq<Enum<U>> for Enum<T> {
    fn eq(&self, other: &Enum<U>) -> bool {
        self.reference == other.reference
    }
}
impl<T> Eq for Enum<T> {}
impl<T> std::hash::Hash for Enum<T> {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        std::hash::Hash::hash(&self.reference, state)
    }
}
impl<T> std::fmt::Debug for Enum<T> {
    fn fmt(&self, out: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.reference.fmt(out)
    }
}
impl<T> std::fmt::Display for Enum<T> {
    fn fmt(&self, out: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self
            .to_string()
            .unwrap_or_else(|error| std::panic::panic_any(error))
        {
            Some(value) => java_native_format_text(out, &value),
            None => out.write_str("null"),
        }
    }
}
impl<T> JavaReferenceIdentity for Enum<T> {
    fn java_reference_identity(&self) -> *const () {
        self.reference.java_reference_identity()
    }
}
impl<T> JavaMapKey for Enum<T> {
    fn java_map_key_eq(&self, other: &Self) -> bool {
        self == other
    }
    fn java_map_key_identity_eq(&self, other: &Self) -> bool {
        self == other
    }
}
impl<T> JavaAppendValue for Enum<T> {
    fn into_java_append_string(self) -> JavaUtf16 {
        self.to_string()
            .unwrap_or_else(|error| std::panic::panic_any(error))
            .map(|value| value.into_java_append_string())
            .unwrap_or_else(|| JavaUtf16::from_scalar("null"))
    }
}
