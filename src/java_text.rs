// Lossless Java text values. Object identity and interning belong to the
// existing String object layer, not this payload. UTF-8 interop is explicit.

#[repr(transparent)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct JavaCodeUnit(pub u16);

impl JavaCodeUnit {
    pub fn from_u16(value: u16) -> Self {
        Self(value)
    }
    pub fn narrow_i32(value: i32) -> Self {
        Self(value as u16)
    }
    pub fn as_u16(self) -> u16 {
        self.0
    }
    pub fn widen_i32(self) -> i32 {
        self.0 as i32
    }
}
impl From<JavaCodeUnit> for i32 {
    fn from(value: JavaCodeUnit) -> Self {
        value.widen_i32()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum JavaTextBounds {
    Index { index: i32, length: i32 },
    Range { begin: i32, end: i32, length: i32 },
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct JavaUtf16 {
    units: Vec<u16>,
}

/// Recognize Boolean.parseBoolean's keyword without a scalar text projection.
pub fn java_boolean_parse_text<T: JavaTextView>(value: Option<T>) -> bool {
    value.is_some_and(|value| {
        let text = value.java_text_view();
        text.units().len() == 4
            && text.units().iter().zip(b"true").all(|(&unit, &ascii)| {
                unit == ascii as u16 || unit == ascii.to_ascii_uppercase() as u16
            })
    })
}

/// Stateful UTF-8 transport encoder, separate from Java logical text.
/// A high surrogate can be completed by a later write; flushing bytes does
/// not end input. Java's UTF-8 encoder replaces malformed input with '?'.
#[derive(Clone, Debug, Default)]
pub struct JavaUtf8Encoder {
    pending_high: Option<u16>,
    closed: bool,
}

impl JavaUtf8Encoder {
    pub fn is_closed(&self) -> bool {
        self.closed
    }

    pub fn encode(&mut self, units: &[u16]) -> Vec<u8> {
        assert!(!self.closed, "encoding after end of input");
        let mut bytes = Vec::with_capacity(units.len());
        for &unit in units {
            if let Some(high) = self.pending_high.take() {
                if (0xdc00..=0xdfff).contains(&unit) {
                    let point = 0x10000 + ((high as u32 - 0xd800) << 10) + unit as u32 - 0xdc00;
                    let mut buffer = [0; 4];
                    bytes.extend_from_slice(
                        char::from_u32(point)
                            .expect("surrogate pair")
                            .encode_utf8(&mut buffer)
                            .as_bytes(),
                    );
                    continue;
                }
                bytes.push(b'?');
            }
            if (0xd800..=0xdbff).contains(&unit) {
                self.pending_high = Some(unit);
            } else if (0xdc00..=0xdfff).contains(&unit) {
                bytes.push(b'?');
            } else {
                let mut buffer = [0; 4];
                bytes.extend_from_slice(
                    char::from_u32(unit as u32)
                        .expect("BMP scalar")
                        .encode_utf8(&mut buffer)
                        .as_bytes(),
                );
            }
        }
        bytes
    }

    pub fn finish(&mut self) -> Vec<u8> {
        self.closed = true;
        if self.pending_high.take().is_some() {
            vec![b'?']
        } else {
            Vec::new()
        }
    }
}

/// System.out/err and explicit PrintStream aliases share one encoder per sink.
pub fn java_standard_text_encoder(stderr: bool) -> &'static std::sync::Mutex<JavaUtf8Encoder> {
    static OUT: std::sync::OnceLock<std::sync::Mutex<JavaUtf8Encoder>> = std::sync::OnceLock::new();
    static ERR: std::sync::OnceLock<std::sync::Mutex<JavaUtf8Encoder>> = std::sync::OnceLock::new();
    (if stderr { &ERR } else { &OUT }).get_or_init(Default::default)
}

pub fn java_standard_write_text<T: JavaTextView>(stderr: bool, text: T) -> std::io::Result<()> {
    use std::io::Write;
    let text = text.java_text_view();
    let mut encoder = java_standard_text_encoder(stderr)
        .lock()
        .expect("Java standard output encoder mutex poisoned");
    if encoder.is_closed() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::BrokenPipe,
            "Stream closed",
        ));
    }
    let bytes = encoder.encode(text.units());
    if stderr {
        std::io::stderr().write_all(&bytes)
    } else {
        std::io::stdout().write_all(&bytes)
    }
}

pub fn java_system_print_text(stderr: bool, mut text: JavaUtf16, newline: bool) {
    if newline {
        text.push_text(&JavaUtf16::from_scalar("\n"));
    }
    // Like PrintStream, Java System printing suppresses transport I/O errors.
    let _ = java_standard_write_text(stderr, text);
}

/// Read-only Java text, including isolated UTF-16 surrogates. Scalar native
/// strings cross this boundary explicitly; lossless carriers borrow payloads.
pub trait JavaTextView {
    fn java_text_view(&self) -> std::borrow::Cow<'_, JavaUtf16>;

    fn java_text_length(&self) -> i32 {
        self.java_text_view().len()
    }
}

impl JavaTextView for str {
    fn java_text_view(&self) -> std::borrow::Cow<'_, JavaUtf16> {
        std::borrow::Cow::Owned(JavaUtf16::from_scalar(self))
    }

    fn java_text_length(&self) -> i32 {
        i32::try_from(self.encode_utf16().count()).expect("Java text allocation exceeds int length")
    }
}

impl JavaTextView for String {
    fn java_text_view(&self) -> std::borrow::Cow<'_, JavaUtf16> {
        self.as_str().java_text_view()
    }
    fn java_text_length(&self) -> i32 {
        self.as_str().java_text_length()
    }
}

// Explicit native scalar-array interop. Source Java char[] uses JavaCodeUnit,
// not Rust char; this adapter keeps native runtime callers on the same view.
impl JavaTextView for [char] {
    fn java_text_view(&self) -> std::borrow::Cow<'_, JavaUtf16> {
        std::borrow::Cow::Owned(self.iter().copied().collect())
    }
}
impl JavaTextView for Vec<char> {
    fn java_text_view(&self) -> std::borrow::Cow<'_, JavaUtf16> {
        self.as_slice().java_text_view()
    }
}

impl<T: JavaTextView + ?Sized> JavaTextView for &T {
    fn java_text_view(&self) -> std::borrow::Cow<'_, JavaUtf16> {
        (**self).java_text_view()
    }
    fn java_text_length(&self) -> i32 {
        (**self).java_text_length()
    }
}

impl<T: JavaTextView + ?Sized> JavaTextView for &mut T {
    fn java_text_view(&self) -> std::borrow::Cow<'_, JavaUtf16> {
        (**self).java_text_view()
    }
    fn java_text_length(&self) -> i32 {
        (**self).java_text_length()
    }
}

impl JavaTextView for JavaCodeUnit {
    fn java_text_view(&self) -> std::borrow::Cow<'_, JavaUtf16> {
        std::borrow::Cow::Owned(JavaUtf16::from_units(vec![self.as_u16()]))
    }
}

impl JavaTextView for [JavaCodeUnit] {
    fn java_text_view(&self) -> std::borrow::Cow<'_, JavaUtf16> {
        std::borrow::Cow::Owned(JavaUtf16::from_units(
            self.iter().map(|unit| unit.as_u16()).collect(),
        ))
    }
}

impl JavaTextView for Vec<JavaCodeUnit> {
    fn java_text_view(&self) -> std::borrow::Cow<'_, JavaUtf16> {
        self.as_slice().java_text_view()
    }
}

impl JavaTextView for JavaUtf16 {
    fn java_text_view(&self) -> std::borrow::Cow<'_, JavaUtf16> {
        std::borrow::Cow::Borrowed(self)
    }
}

pub trait JavaStringSearchNeedle {
    fn java_string_search_needle(&self) -> Option<std::borrow::Cow<'_, JavaUtf16>>;
}

impl JavaStringSearchNeedle for str {
    fn java_string_search_needle(&self) -> Option<std::borrow::Cow<'_, JavaUtf16>> {
        Some(self.java_text_view())
    }
}

impl JavaStringSearchNeedle for String {
    fn java_string_search_needle(&self) -> Option<std::borrow::Cow<'_, JavaUtf16>> {
        Some(self.java_text_view())
    }
}

impl JavaStringSearchNeedle for JavaUtf16 {
    fn java_string_search_needle(&self) -> Option<std::borrow::Cow<'_, JavaUtf16>> {
        Some(self.java_text_view())
    }
}

impl JavaStringSearchNeedle for char {
    fn java_string_search_needle(&self) -> Option<std::borrow::Cow<'_, JavaUtf16>> {
        Some(std::borrow::Cow::Owned(JavaUtf16::from_scalar(
            &self.to_string(),
        )))
    }
}

impl JavaStringSearchNeedle for JavaCodeUnit {
    fn java_string_search_needle(&self) -> Option<std::borrow::Cow<'_, JavaUtf16>> {
        Some(std::borrow::Cow::Owned(JavaUtf16::from_units(vec![
            self.as_u16(),
        ])))
    }
}

impl JavaStringSearchNeedle for i32 {
    fn java_string_search_needle(&self) -> Option<std::borrow::Cow<'_, JavaUtf16>> {
        let text = if (0..=0xffff).contains(self) {
            JavaUtf16::from_units(vec![*self as u16])
        } else {
            let point = u32::try_from(*self).ok().and_then(char::from_u32)?;
            JavaUtf16::from_scalar(&point.to_string())
        };
        Some(std::borrow::Cow::Owned(text))
    }
}

impl<T: JavaStringSearchNeedle + ?Sized> JavaStringSearchNeedle for &T {
    fn java_string_search_needle(&self) -> Option<std::borrow::Cow<'_, JavaUtf16>> {
        (**self).java_string_search_needle()
    }
}

impl JavaUtf16 {
    /// String.trim removes UTF-16 units <= U+0020, not Unicode whitespace.
    pub fn java_trim_range(&self) -> (i32, i32) {
        let begin = self
            .units
            .iter()
            .position(|unit| *unit > 0x20)
            .unwrap_or(self.units.len());
        let end = self
            .units
            .iter()
            .rposition(|unit| *unit > 0x20)
            .map(|index| index + 1)
            .unwrap_or(begin);
        (begin as i32, end as i32)
    }

    pub fn push_text(&mut self, other: &Self) {
        self.units.extend_from_slice(other.units());
    }
    pub fn set_length(&mut self, length: i32) -> Result<(), JavaTextBounds> {
        if length < 0 {
            return Err(JavaTextBounds::Range {
                begin: 0,
                end: length,
                length: self.len(),
            });
        }
        self.units.resize(length as usize, 0);
        Ok(())
    }
    pub fn set_unit(&mut self, index: i32, unit: JavaCodeUnit) -> Result<(), JavaTextBounds> {
        let length = self.len();
        let at = usize::try_from(index)
            .ok()
            .and_then(|index| self.units.get_mut(index));
        match at {
            Some(at) => {
                *at = unit.as_u16();
                Ok(())
            }
            None => Err(JavaTextBounds::Index { index, length }),
        }
    }
    pub fn replace_range(
        &mut self,
        begin: i32,
        end: i32,
        value: &Self,
    ) -> Result<(), JavaTextBounds> {
        if begin < 0 || end < begin || end > self.len() {
            return Err(JavaTextBounds::Range {
                begin,
                end,
                length: self.len(),
            });
        }
        self.units
            .splice(begin as usize..end as usize, value.units.iter().copied());
        Ok(())
    }
    pub fn from_units(units: Vec<u16>) -> Self {
        Self { units }
    }
    pub fn from_scalar(value: &str) -> Self {
        Self::from_units(value.encode_utf16().collect())
    }
    pub fn units(&self) -> &[u16] {
        &self.units
    }
    pub fn len(&self) -> i32 {
        // Java's allocation boundary limits String/array lengths to int.
        i32::try_from(self.units.len()).expect("Java text allocation exceeds int length")
    }
    pub fn is_empty(&self) -> bool {
        self.units.is_empty()
    }
    pub fn char_at(&self, index: i32) -> Result<JavaCodeUnit, JavaTextBounds> {
        let unit = usize::try_from(index)
            .ok()
            .and_then(|index| self.units.get(index))
            .copied();
        unit.map(JavaCodeUnit::from_u16)
            .ok_or(JavaTextBounds::Index {
                index,
                length: self.len(),
            })
    }
    pub fn substring(&self, begin: i32, end: i32) -> Result<Self, JavaTextBounds> {
        if begin < 0 || end < begin || end > self.len() {
            return Err(JavaTextBounds::Range {
                begin,
                end,
                length: self.len(),
            });
        }
        Ok(Self::from_units(
            self.units[begin as usize..end as usize].to_vec(),
        ))
    }
    pub fn concat(&self, other: &Self) -> Self {
        Self::from_units([self.units(), other.units()].concat())
    }
    /// Literal, non-overlapping replacement over Java code units. None means
    /// no occurrence, allowing the object layer to retain the input identity.
    pub fn replace_literal(&self, target: &Self, replacement: &Self) -> Option<Self> {
        if target.is_empty() {
            let mut units = Vec::new();
            units.extend_from_slice(replacement.units());
            for &unit in self.units() {
                units.push(unit);
                units.extend_from_slice(replacement.units());
            }
            return Some(Self::from_units(units));
        }
        let first = self.index_of(target, 0);
        if first < 0 {
            return None;
        }
        let mut units = Vec::new();
        let mut start = 0;
        let mut occurrence = first;
        while occurrence >= 0 {
            let at = occurrence as usize;
            units.extend_from_slice(&self.units()[start..at]);
            units.extend_from_slice(replacement.units());
            start = at + target.units().len();
            occurrence = self.index_of(target, start as i32);
        }
        units.extend_from_slice(&self.units()[start..]);
        Some(Self::from_units(units))
    }
    pub fn java_hash(&self) -> i32 {
        self.units.iter().fold(0i32, |hash, unit| {
            hash.wrapping_mul(31).wrapping_add(*unit as i32)
        })
    }
    pub fn java_compare(&self, other: &Self) -> i32 {
        for (&left, &right) in self.units.iter().zip(other.units.iter()) {
            if left != right {
                return left as i32 - right as i32;
            }
        }
        self.len() - other.len()
    }
    pub fn index_of(&self, needle: &Self, from: i32) -> i32 {
        let from = from.max(0) as usize;
        if needle.is_empty() {
            return from.min(self.units.len()) as i32;
        }
        let Some(last) = self.units.len().checked_sub(needle.units.len()) else {
            return -1;
        };
        if from > last {
            return -1;
        }
        (from..=last)
            .find(|&at| self.units[at..at + needle.units.len()] == *needle.units)
            .map(|at| at as i32)
            .unwrap_or(-1)
    }
    pub fn last_index_of(&self, needle: &Self, from: i32) -> i32 {
        if from < 0 {
            return -1;
        }
        if needle.is_empty() {
            return from.min(self.len());
        }
        let Some(last) = self.units.len().checked_sub(needle.units.len()) else {
            return -1;
        };
        let from = (from as usize).min(last);
        (0..=from)
            .rev()
            .find(|&at| self.units[at..at + needle.units.len()] == *needle.units)
            .map(|at| at as i32)
            .unwrap_or(-1)
    }
    pub fn index_of_code_point(&self, point: i32, from: i32) -> i32 {
        let Some(needle) = point.java_string_search_needle() else {
            return -1;
        };
        self.index_of(&needle, from)
    }
    pub fn scalar_string(&self) -> Result<String, std::string::FromUtf16Error> {
        String::from_utf16(self.units())
    }
}

impl From<String> for JavaUtf16 {
    fn from(value: String) -> Self {
        Self::from_scalar(&value)
    }
}
impl From<&str> for JavaUtf16 {
    fn from(value: &str) -> Self {
        Self::from_scalar(value)
    }
}
impl FromIterator<char> for JavaUtf16 {
    fn from_iter<T: IntoIterator<Item = char>>(values: T) -> Self {
        let mut units = Vec::new();
        for value in values {
            units.extend_from_slice(value.encode_utf16(&mut [0; 2]));
        }
        Self::from_units(units)
    }
}
