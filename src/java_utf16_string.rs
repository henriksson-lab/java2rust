// The enclosing host/emitted module supplies JavaUtf16, JavaCodeUnit,
// JavaTextBounds, JavaTextView and the shared JavaStringIdentity leaf.

#[derive(Clone, Debug)]
pub struct JavaUtf16String(JavaStringIdentity<JavaUtf16>);

impl JavaUtf16String {
    pub fn fresh(value: impl JavaTextView) -> Self {
        Self::from_payload(value.java_text_view().into_owned())
    }
    pub fn intern_literal(value: impl JavaTextView) -> Self {
        Self::fresh(value).intern()
    }
    pub fn empty_ref() -> &'static Self {
        static EMPTY: std::sync::OnceLock<JavaUtf16String> = std::sync::OnceLock::new();
        EMPTY.get_or_init(|| Self::intern_literal(""))
    }
    pub fn from_units(units: Vec<u16>) -> Self {
        Self::from_payload(JavaUtf16::from_units(units))
    }
    pub fn from_scalar(value: &str) -> Self {
        Self::from_payload(JavaUtf16::from_scalar(value))
    }
    pub fn from_payload(value: JavaUtf16) -> Self {
        Self(JavaStringIdentity::from_payload(std::sync::Arc::new(value)))
    }
    pub fn intern(&self) -> Self {
        static INTERNED: std::sync::OnceLock<
            std::sync::Mutex<std::collections::HashMap<JavaUtf16, std::sync::Arc<JavaUtf16>>>,
        > = std::sync::OnceLock::new();
        let pool = INTERNED.get_or_init(|| std::sync::Mutex::new(std::collections::HashMap::new()));
        Self(self.0.intern_reference(pool))
    }
    pub fn java_equals(&self, other: &Self) -> bool {
        self.0.java_equals(&other.0)
    }
    pub fn java_identity_eq(&self, other: &Self) -> bool {
        self.0.java_identity_eq(&other.0)
    }
    pub fn identity_ptr(&self) -> *const () {
        self.0.identity_ptr()
    }
    pub fn payload(&self) -> &JavaUtf16 {
        self.0.payload()
    }
    pub fn units(&self) -> &[u16] {
        self.payload().units()
    }
    pub fn len(&self) -> i32 {
        self.payload().len()
    }
    pub fn is_empty(&self) -> bool {
        self.payload().is_empty()
    }
    /// Java String.toString returns the same object, not Rust scalar text.
    pub fn to_string(&self) -> Self {
        self.clone()
    }

    pub fn trim(&self) -> Self {
        let (begin, end) = self.payload().java_trim_range();
        self.substring(begin, end).expect("validated trim range")
    }
    pub fn char_at(&self, index: i32) -> Result<JavaCodeUnit, JavaTextBounds> {
        self.payload().char_at(index)
    }
    pub fn substring(&self, begin: i32, end: i32) -> Result<Self, JavaTextBounds> {
        if begin == 0 && end == self.len() {
            return Ok(self.clone());
        }
        let result = self.payload().substring(begin, end)?;
        if begin == end {
            return Ok(Self::from_payload(result).intern());
        }
        Ok(Self::from_payload(result))
    }
    pub fn concat(&self, other: &Self) -> Self {
        if other.is_empty() {
            return self.clone();
        }
        Self::from_payload(self.payload().concat(other.payload()))
    }
    /// Java's runtime + / append must allocate a fresh result even when a
    /// segment is empty. It is not the same identity contract as String.concat.
    pub fn append(&self, other: &Self) -> Self {
        Self::from_payload(self.payload().concat(other.payload()))
    }
    pub fn java_hash(&self) -> i32 {
        self.payload().java_hash()
    }
    pub fn java_compare(&self, other: &Self) -> i32 {
        self.payload().java_compare(other.payload())
    }
    pub fn index_of(&self, needle: &Self, from: i32) -> i32 {
        self.payload().index_of(needle.payload(), from)
    }
    pub fn last_index_of(&self, needle: &Self, from: i32) -> i32 {
        self.payload().last_index_of(needle.payload(), from)
    }
    pub fn index_of_code_point(&self, point: i32, from: i32) -> i32 {
        self.payload().index_of_code_point(point, from)
    }
    pub fn scalar_string(&self) -> Result<String, std::string::FromUtf16Error> {
        self.payload().scalar_string()
    }
}

impl PartialEq for JavaUtf16String {
    fn eq(&self, other: &Self) -> bool {
        self.java_equals(other)
    }
}
impl Eq for JavaUtf16String {}
impl JavaStringSearchNeedle for JavaUtf16String {
    fn java_string_search_needle(&self) -> Option<std::borrow::Cow<'_, JavaUtf16>> {
        Some(self.java_text_view())
    }
}
impl JavaTextView for JavaUtf16String {
    fn java_text_view(&self) -> std::borrow::Cow<'_, JavaUtf16> {
        std::borrow::Cow::Borrowed(self.payload())
    }
}
impl Default for JavaUtf16String {
    fn default() -> Self {
        Self::intern_literal("")
    }
}
impl From<JavaUtf16String> for JavaUtf16 {
    fn from(value: JavaUtf16String) -> Self {
        value.java_text_view().into_owned()
    }
}
impl From<&JavaUtf16String> for JavaUtf16 {
    fn from(value: &JavaUtf16String) -> Self {
        value.java_text_view().into_owned()
    }
}
impl std::hash::Hash for JavaUtf16String {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        std::hash::Hash::hash(self.payload(), state)
    }
}
impl PartialOrd for JavaUtf16String {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for JavaUtf16String {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.java_compare(other).cmp(&0)
    }
}
