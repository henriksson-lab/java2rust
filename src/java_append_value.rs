// One logical text capability shared by source-only concatenation and native
// builders. Object callbacks are staged by lowering, not Rust Display here.
pub trait JavaAppendValue {
    fn into_java_append_string(self) -> JavaUtf16;
}

impl JavaAppendValue for JavaUtf16 {
    fn into_java_append_string(self) -> JavaUtf16 {
        self
    }
}

impl JavaAppendValue for &JavaUtf16 {
    fn into_java_append_string(self) -> JavaUtf16 {
        self.clone()
    }
}

impl JavaAppendValue for JavaCodeUnit {
    fn into_java_append_string(self) -> JavaUtf16 {
        JavaUtf16::from_units(vec![self.as_u16()])
    }
}
impl JavaAppendValue for &JavaCodeUnit {
    fn into_java_append_string(self) -> JavaUtf16 {
        (*self).into_java_append_string()
    }
}
impl JavaAppendValue for Vec<JavaCodeUnit> {
    fn into_java_append_string(self) -> JavaUtf16 {
        self.java_text_view().into_owned()
    }
}
impl JavaAppendValue for &Vec<JavaCodeUnit> {
    fn into_java_append_string(self) -> JavaUtf16 {
        self.java_text_view().into_owned()
    }
}
impl JavaAppendValue for &[JavaCodeUnit] {
    fn into_java_append_string(self) -> JavaUtf16 {
        self.java_text_view().into_owned()
    }
}

impl JavaAppendValue for bool {
    fn into_java_append_string(self) -> JavaUtf16 {
        ({ self.to_string() }).into()
    }
}

impl JavaAppendValue for char {
    fn into_java_append_string(self) -> JavaUtf16 {
        ({ self.to_string() }).into()
    }
}

impl JavaAppendValue for i8 {
    fn into_java_append_string(self) -> JavaUtf16 {
        ({ self.to_string() }).into()
    }
}

impl JavaAppendValue for i16 {
    fn into_java_append_string(self) -> JavaUtf16 {
        ({ self.to_string() }).into()
    }
}

impl JavaAppendValue for i32 {
    fn into_java_append_string(self) -> JavaUtf16 {
        ({ self.to_string() }).into()
    }
}

impl JavaAppendValue for i64 {
    fn into_java_append_string(self) -> JavaUtf16 {
        ({ self.to_string() }).into()
    }
}

impl JavaAppendValue for isize {
    fn into_java_append_string(self) -> JavaUtf16 {
        ({ self.to_string() }).into()
    }
}

impl JavaAppendValue for u8 {
    fn into_java_append_string(self) -> JavaUtf16 {
        ({ self.to_string() }).into()
    }
}

impl JavaAppendValue for u16 {
    fn into_java_append_string(self) -> JavaUtf16 {
        ({ self.to_string() }).into()
    }
}

impl JavaAppendValue for u32 {
    fn into_java_append_string(self) -> JavaUtf16 {
        ({ self.to_string() }).into()
    }
}

impl JavaAppendValue for u64 {
    fn into_java_append_string(self) -> JavaUtf16 {
        ({ self.to_string() }).into()
    }
}

impl JavaAppendValue for usize {
    fn into_java_append_string(self) -> JavaUtf16 {
        ({ self.to_string() }).into()
    }
}

impl JavaAppendValue for f32 {
    fn into_java_append_string(self) -> JavaUtf16 {
        ({ java_float_to_string(self) }).into()
    }
}

impl JavaAppendValue for f64 {
    fn into_java_append_string(self) -> JavaUtf16 {
        ({ java_double_to_string(self) }).into()
    }
}

impl JavaAppendValue for &bool {
    fn into_java_append_string(self) -> JavaUtf16 {
        ({ (*self).into_java_append_string() }).into()
    }
}
impl JavaAppendValue for &char {
    fn into_java_append_string(self) -> JavaUtf16 {
        ({ (*self).into_java_append_string() }).into()
    }
}
impl JavaAppendValue for &i8 {
    fn into_java_append_string(self) -> JavaUtf16 {
        ({ (*self).into_java_append_string() }).into()
    }
}
impl JavaAppendValue for &i16 {
    fn into_java_append_string(self) -> JavaUtf16 {
        ({ (*self).into_java_append_string() }).into()
    }
}
impl JavaAppendValue for &i32 {
    fn into_java_append_string(self) -> JavaUtf16 {
        ({ (*self).into_java_append_string() }).into()
    }
}
impl JavaAppendValue for &i64 {
    fn into_java_append_string(self) -> JavaUtf16 {
        ({ (*self).into_java_append_string() }).into()
    }
}
impl JavaAppendValue for &isize {
    fn into_java_append_string(self) -> JavaUtf16 {
        ({ (*self).into_java_append_string() }).into()
    }
}
impl JavaAppendValue for &u8 {
    fn into_java_append_string(self) -> JavaUtf16 {
        ({ (*self).into_java_append_string() }).into()
    }
}
impl JavaAppendValue for &u16 {
    fn into_java_append_string(self) -> JavaUtf16 {
        ({ (*self).into_java_append_string() }).into()
    }
}
impl JavaAppendValue for &u32 {
    fn into_java_append_string(self) -> JavaUtf16 {
        ({ (*self).into_java_append_string() }).into()
    }
}
impl JavaAppendValue for &u64 {
    fn into_java_append_string(self) -> JavaUtf16 {
        ({ (*self).into_java_append_string() }).into()
    }
}
impl JavaAppendValue for &usize {
    fn into_java_append_string(self) -> JavaUtf16 {
        ({ (*self).into_java_append_string() }).into()
    }
}
impl JavaAppendValue for &f32 {
    fn into_java_append_string(self) -> JavaUtf16 {
        ({ (*self).into_java_append_string() }).into()
    }
}
impl JavaAppendValue for &f64 {
    fn into_java_append_string(self) -> JavaUtf16 {
        ({ (*self).into_java_append_string() }).into()
    }
}

impl<T: JavaAppendValue> JavaAppendValue for Option<T> {
    fn into_java_append_string(self) -> JavaUtf16 {
        ({
            self.map(JavaAppendValue::into_java_append_string)
                .unwrap_or_else(|| JavaUtf16::from_scalar("null"))
        })
        .into()
    }
}

impl<T: JavaAppendValue + Clone> JavaAppendValue for &Option<T> {
    fn into_java_append_string(self) -> JavaUtf16 {
        ({ self.clone().into_java_append_string() }).into()
    }
}

impl<T: ?Sized> JavaAppendValue for &&T
where
    for<'a> &'a T: JavaAppendValue,
{
    fn into_java_append_string(self) -> JavaUtf16 {
        (*self).into_java_append_string()
    }
}

impl JavaAppendValue for String {
    fn into_java_append_string(self) -> JavaUtf16 {
        ({ self }).into()
    }
}

impl JavaAppendValue for &String {
    fn into_java_append_string(self) -> JavaUtf16 {
        ({ self.clone() }).into()
    }
}

impl JavaAppendValue for &str {
    fn into_java_append_string(self) -> JavaUtf16 {
        ({ self.to_string() }).into()
    }
}

impl JavaAppendValue for Vec<char> {
    fn into_java_append_string(self) -> JavaUtf16 {
        ({ self.into_iter().collect::<JavaUtf16>() }).into()
    }
}

impl JavaAppendValue for &Vec<char> {
    fn into_java_append_string(self) -> JavaUtf16 {
        self.iter().copied().collect()
    }
}

impl JavaAppendValue for &[char] {
    fn into_java_append_string(self) -> JavaUtf16 {
        self.iter().copied().collect()
    }
}
