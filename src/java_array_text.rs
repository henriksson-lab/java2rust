// Logical leaf-array display shared by scalar and lossless String carriers.
pub trait JavaArraysToStringElement {
    fn java_arrays_to_string_element(&self) -> String;
    fn java_arrays_to_text_element(&self) -> JavaUtf16 {
        JavaUtf16::from_scalar(&self.java_arrays_to_string_element())
    }
}

macro_rules! impl_java_arrays_to_string_display {
    ($($ty:ty),* $(,)?) => {
        $(
            impl JavaArraysToStringElement for $ty {
                fn java_arrays_to_string_element(&self) -> String {
                    self.to_string()
                }
            }
        )*
    };
}

impl_java_arrays_to_string_display!(bool, i8, i16, i32, i64, char, f32, f64, String, &str);

impl JavaArraysToStringElement for JavaCodeUnit {
    fn java_arrays_to_string_element(&self) -> String {
        self.java_arrays_to_text_element()
            .scalar_string()
            .expect("native scalar array display cannot represent an isolated Java surrogate")
    }
    fn java_arrays_to_text_element(&self) -> JavaUtf16 {
        JavaUtf16::from_units(vec![self.as_u16()])
    }
}

impl<T: JavaArraysToStringElement> JavaArraysToStringElement for Option<T> {
    fn java_arrays_to_string_element(&self) -> String {
        self.as_ref()
            .map(JavaArraysToStringElement::java_arrays_to_string_element)
            .unwrap_or_else(|| "null".to_string())
    }
    fn java_arrays_to_text_element(&self) -> JavaUtf16 {
        self.as_ref()
            .map(JavaArraysToStringElement::java_arrays_to_text_element)
            .unwrap_or_else(|| JavaUtf16::from_scalar("null"))
    }
}

pub trait JavaArraysToStringInput<T> {
    fn with_java_arrays_to_string_slice<R>(&self, visit: impl FnOnce(Option<&[T]>) -> R) -> R;
}

impl<T> JavaArraysToStringInput<T> for [T] {
    fn with_java_arrays_to_string_slice<R>(&self, visit: impl FnOnce(Option<&[T]>) -> R) -> R {
        visit(Some(self))
    }
}

impl<T> JavaArraysToStringInput<T> for Vec<T> {
    fn with_java_arrays_to_string_slice<R>(&self, visit: impl FnOnce(Option<&[T]>) -> R) -> R {
        visit(Some(self.as_slice()))
    }
}

impl<T, U: JavaArraysToStringInput<T> + ?Sized> JavaArraysToStringInput<T> for &U {
    fn with_java_arrays_to_string_slice<R>(&self, visit: impl FnOnce(Option<&[T]>) -> R) -> R {
        (*self).with_java_arrays_to_string_slice(visit)
    }
}

impl<T, U: JavaArraysToStringInput<T>> JavaArraysToStringInput<T> for Option<U> {
    fn with_java_arrays_to_string_slice<R>(&self, visit: impl FnOnce(Option<&[T]>) -> R) -> R {
        match self.as_ref() {
            Some(value) => value.with_java_arrays_to_string_slice(visit),
            None => visit(None),
        }
    }
}

impl<T> JavaArraysToStringInput<T> for std::rc::Rc<std::cell::RefCell<Vec<T>>> {
    fn with_java_arrays_to_string_slice<R>(&self, visit: impl FnOnce(Option<&[T]>) -> R) -> R {
        let values = self.borrow();
        visit(Some(values.as_slice()))
    }
}

impl<T> JavaArraysToStringInput<T> for std::sync::Arc<std::sync::Mutex<Vec<T>>> {
    fn with_java_arrays_to_string_slice<R>(&self, visit: impl FnOnce(Option<&[T]>) -> R) -> R {
        let values = self.lock().expect("Java shared array mutex poisoned");
        visit(Some(values.as_slice()))
    }
}

// These element implementations are immutable text/scalar leaves. Source
// Object display needs a separate fallible cursor protocol, with no array loan
// held across Java callbacks; it must not be added to this leaf-only API.
pub fn java_arrays_to_text<T: JavaArraysToStringElement + Clone, V: JavaArrayReadInput<T>>(
    values: V,
) -> JavaUtf16 {
    let Some(length) = values.java_arrays_length() else {
        return JavaUtf16::from_scalar("null");
    };
    let mut text = JavaUtf16::from_scalar("[");
    for index in 0..length {
        if index != 0 {
            text.push_text(&JavaUtf16::from_scalar(", "));
        }
        let value = values.java_arrays_element(index);
        text.push_text(&value.java_arrays_to_text_element());
    }
    text.push_text(&JavaUtf16::from_scalar("]"));
    text
}

pub fn java_arrays_to_string<T: JavaArraysToStringElement, V: JavaArraysToStringInput<T>>(
    values: V,
) -> String {
    values.with_java_arrays_to_string_slice(|values| {
        let Some(values) = values else {
            return "null".to_string();
        };
        let body = values
            .iter()
            .map(JavaArraysToStringElement::java_arrays_to_string_element)
            .collect::<Vec<_>>()
            .join(", ");
        format!("[{body}]")
    })
}
