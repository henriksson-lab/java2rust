// Java exception state and logical message API. The producer selects the
// text alias and explicit formatter adapter; no scalar projection lives here.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JavaException {
    class_name: &'static str,
    message: Option<JavaExceptionText>,
    cause: Option<std::boxed::Box<JavaException>>,
    suppressed: Vec<JavaException>,
}

impl JavaException {
    pub fn new(class_name: &'static str, message: impl Into<JavaExceptionText>) -> JavaException {
        JavaException::from_constructor(class_name, Some(message.into()), None)
    }

    pub fn from_constructor(
        class_name: &'static str,
        message: Option<JavaExceptionText>,
        cause: Option<JavaException>,
    ) -> JavaException {
        JavaException {
            class_name,
            message,
            cause: cause.map(std::boxed::Box::new),
            suppressed: Vec::new(),
        }
    }

    pub fn with_runtime_class(mut self, class_name: &'static str) -> JavaException {
        self.class_name = class_name;
        self
    }

    pub fn with_source_class(
        mut self,
        platform_base: &'static str,
        source_class: &'static str,
    ) -> JavaException {
        if self.class_name == platform_base {
            self.class_name = source_class;
        }
        self
    }

    pub fn from_cause_constructor(
        class_name: &'static str,
        cause: Option<JavaException>,
    ) -> JavaException {
        let message = cause.as_ref().map(JavaException::to_string);
        JavaException::from_constructor(class_name, message, cause)
    }

    pub fn with_cause(
        class_name: &'static str,
        message: impl Into<JavaExceptionText>,
        cause: JavaException,
    ) -> JavaException {
        JavaException {
            class_name,
            message: Some(message.into()),
            cause: Some(std::boxed::Box::new(cause)),
            suppressed: Vec::new(),
        }
    }

    pub fn from_display(class_name: &'static str, err: impl std::fmt::Display) -> JavaException {
        JavaException::new(class_name, err.to_string())
    }

    pub fn class_name(&self) -> &'static str {
        self.class_name
    }

    pub fn message(&self) -> &JavaExceptionText {
        static EMPTY: std::sync::OnceLock<JavaExceptionText> = std::sync::OnceLock::new();
        self.message
            .as_ref()
            .unwrap_or_else(|| EMPTY.get_or_init(Default::default))
    }

    pub fn to_string(&self) -> JavaExceptionText {
        java_exception_text_value(self.class_name, self.get_localized_message())
    }

    pub fn is_instance(&self, class_names: &[&str]) -> bool {
        class_names.iter().any(|name| {
            self.class_name == *name || java_exception_is_subtype(self.class_name, name)
        })
    }

    pub fn cause_or_self_if_instance(mut self, class_names: &[&str]) -> JavaException {
        if self
            .cause
            .as_ref()
            .is_some_and(|cause| cause.is_instance(class_names))
        {
            *self.cause.take().expect("checked Java exception cause")
        } else {
            self
        }
    }

    pub fn get_message(&self) -> Option<JavaExceptionText> {
        self.message.clone()
    }

    pub fn get_localized_message(&self) -> Option<JavaExceptionText> {
        self.get_message()
    }

    pub fn cause(&self) -> Option<&JavaException> {
        self.cause.as_deref()
    }

    pub fn get_cause(&self) -> Option<JavaException> {
        self.cause.as_deref().cloned()
    }

    pub fn add_suppressed(&mut self, exception: JavaException) {
        self.suppressed.push(exception);
    }

    pub fn get_suppressed(&self) -> Vec<Option<JavaException>> {
        self.suppressed.iter().cloned().map(Some).collect()
    }

    pub fn get_class(&self) -> JavaClass<()> {
        JavaClass::<()>::for_name(self.class_name)
    }
}
