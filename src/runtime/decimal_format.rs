/// A number accepted by `format(..)` — covers the owned and borrowed scalar
/// forms the translator emits (`f64`/`f32`/`i32`/`i64` and `&T` of those, since
/// a read-borrowed numeric arg arrives as `&f64`). `i64` above 2^53 loses
/// precision; acceptable for formatting.
pub trait JavaNum {
    fn as_f64(&self) -> f64;
}
impl JavaNum for f64 {
    fn as_f64(&self) -> f64 {
        *self
    }
}
impl JavaNum for f32 {
    fn as_f64(&self) -> f64 {
        *self as f64
    }
}
impl JavaNum for i32 {
    fn as_f64(&self) -> f64 {
        *self as f64
    }
}
impl JavaNum for i64 {
    fn as_f64(&self) -> f64 {
        *self as f64
    }
}
impl<T: JavaNum + ?Sized> JavaNum for &T {
    fn as_f64(&self) -> f64 {
        (**self).as_f64()
    }
}

/// Java `java.text.DecimalFormat` / `NumberFormat` -> a real number formatter.
/// Config (max/min fraction digits, grouping, decimal-separator-always-shown)
/// lives in `Cell`/`RefCell` so every mutator can take `&self`: a Java field /
/// `static final` of this type lowers to a value reached through `Deref` as
/// `&Self`, so `&mut self` methods would fail with E0596 at those call sites.
#[derive(Clone, Debug)]
pub struct JavaDecimalFormat {
    pattern: std::cell::RefCell<String>,
    max_fraction_digits: std::cell::Cell<i32>,
    min_fraction_digits: std::cell::Cell<i32>,
    grouping_used: std::cell::Cell<bool>,
    decimal_separator_always_shown: std::cell::Cell<bool>,
}
impl Default for JavaDecimalFormat {
    fn default() -> Self {
        JavaDecimalFormat {
            pattern: std::cell::RefCell::new(String::new()),
            max_fraction_digits: std::cell::Cell::new(3),
            min_fraction_digits: std::cell::Cell::new(0),
            grouping_used: std::cell::Cell::new(false),
            decimal_separator_always_shown: std::cell::Cell::new(false),
        }
    }
}
// Java compares/maps formatters by configuration; `Cell`/`RefCell` aren't
// `Hash` and `RefCell` isn't `Eq`, so hand-roll over the current field values.
impl PartialEq for JavaDecimalFormat {
    fn eq(&self, o: &Self) -> bool {
        *self.pattern.borrow() == *o.pattern.borrow()
            && self.max_fraction_digits.get() == o.max_fraction_digits.get()
            && self.min_fraction_digits.get() == o.min_fraction_digits.get()
            && self.grouping_used.get() == o.grouping_used.get()
            && self.decimal_separator_always_shown.get() == o.decimal_separator_always_shown.get()
    }
}
impl Eq for JavaDecimalFormat {}
impl std::hash::Hash for JavaDecimalFormat {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.pattern.borrow().hash(state);
        self.max_fraction_digits.get().hash(state);
        self.min_fraction_digits.get().hash(state);
        self.grouping_used.get().hash(state);
        self.decimal_separator_always_shown.get().hash(state);
    }
}
impl JavaDecimalFormat {
    pub fn new() -> Self {
        JavaDecimalFormat::default()
    }
    pub fn new_1<P: ToString>(pattern: P) -> Self {
        let f = JavaDecimalFormat::default();
        f.apply_pattern(pattern);
        f
    }
    pub fn new_2<P: ToString, S>(pattern: P, _symbols: S) -> Self {
        JavaDecimalFormat::new_1(pattern)
    }
    /// Parse a Java decimal pattern, best-effort: the count of fraction-place
    /// chars (`#`/`0`) after the `.` sets max fraction digits, the count of `0`
    /// after the `.` sets min; a `,` before the `.` turns grouping on.
    pub fn apply_pattern<P: ToString>(&self, p: P) {
        let pat = p.to_string();
        *self.pattern.borrow_mut() = pat.clone();
        let (int_part, frac_part) = match pat.find('.') {
            Some(i) => (&pat[..i], &pat[i + 1..]),
            None => (pat.as_str(), ""),
        };
        self.grouping_used.set(int_part.contains(','));
        let max = frac_part.chars().filter(|&c| c == '#' || c == '0').count() as i32;
        let min = frac_part.chars().filter(|&c| c == '0').count() as i32;
        self.max_fraction_digits.set(max);
        self.min_fraction_digits.set(min);
        self.decimal_separator_always_shown.set(pat.ends_with('.'));
    }
    pub fn apply_localized_pattern<P: ToString>(&self, p: P) {
        self.apply_pattern(p);
    }
    pub fn to_pattern(&self) -> String {
        self.pattern.borrow().clone()
    }
    pub fn set_maximum_fraction_digits(&self, n: i32) {
        self.max_fraction_digits.set(n);
    }
    pub fn get_maximum_fraction_digits(&self) -> i32 {
        self.max_fraction_digits.get()
    }
    pub fn set_minimum_fraction_digits(&self, n: i32) {
        self.min_fraction_digits.set(n);
    }
    pub fn get_minimum_fraction_digits(&self) -> i32 {
        self.min_fraction_digits.get()
    }
    pub fn set_grouping_used(&self, b: bool) {
        self.grouping_used.set(b);
    }
    pub fn is_grouping_used(&self) -> bool {
        self.grouping_used.get()
    }
    pub fn set_decimal_separator_always_shown(&self, b: bool) {
        self.decimal_separator_always_shown.set(b);
    }
    pub fn is_decimal_separator_always_shown(&self) -> bool {
        self.decimal_separator_always_shown.get()
    }
    /// `i64` arguments lose precision above 2^53; acceptable for formatting.
    pub fn format<N: JavaNum>(&self, n: N) -> String {
        let x: f64 = n.as_f64();
        let max = self.max_fraction_digits.get().max(0) as usize;
        let min = self.min_fraction_digits.get().max(0) as usize;
        // Round to max digits, then trim trailing zeros down to the minimum.
        let mut s = format!("{:.*}", max, x);
        if max > min {
            if let Some(dot) = s.find('.') {
                let keep_min = dot + 1 + min;
                let mut end = s.len();
                while end > keep_min && s.as_bytes()[end - 1] == b'0' {
                    end -= 1;
                }
                // If everything after the dot was trimmed, drop the dot too.
                if end == dot + 1 {
                    end = dot;
                }
                s.truncate(end);
            }
        }
        if self.decimal_separator_always_shown.get() && !s.contains('.') {
            s.push('.');
        }
        if self.grouping_used.get() {
            s = Self::group(&s);
        }
        s
    }
    /// Insert `,` every three digits in the integer part of `s` (handles a
    /// leading `-` and a trailing fractional part).
    fn group(s: &str) -> String {
        let (sign, body) = match s.strip_prefix('-') {
            Some(rest) => ("-", rest),
            None => ("", s),
        };
        let (int_part, frac_part) = match body.find('.') {
            Some(i) => (&body[..i], &body[i..]),
            None => (body, ""),
        };
        let bytes = int_part.as_bytes();
        let mut grouped = String::new();
        let len = bytes.len();
        for (idx, &b) in bytes.iter().enumerate() {
            if idx > 0 && (len - idx) % 3 == 0 {
                grouped.push(',');
            }
            grouped.push(b as char);
        }
        format!("{}{}{}", sign, grouped, frac_part)
    }
}

/// Java `java.text.NumberFormat`. Same surface as `DecimalFormat`; the static
/// factories return a default instance.
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct JavaNumberFormat {
    inner: JavaDecimalFormat,
}
impl JavaNumberFormat {
    pub fn new() -> Self {
        JavaNumberFormat::default()
    }
    /// `getInstance()` — the `getInstance(Locale)` 1-arg overload is routed here
    /// (locale dropped) by a `static_rule` arm, since static calls aren't
    /// arity-suffixed and Rust can't have both a 0- and 1-arg `get_instance`.
    pub fn get_instance() -> JavaNumberFormat {
        JavaNumberFormat::default()
    }
    pub fn get_number_instance() -> JavaNumberFormat {
        JavaNumberFormat::default()
    }
    pub fn get_integer_instance() -> JavaNumberFormat {
        let f = JavaNumberFormat::default();
        f.set_maximum_fraction_digits(0);
        f
    }
    pub fn get_percent_instance() -> JavaNumberFormat {
        JavaNumberFormat::default()
    }
    pub fn format<N: JavaNum>(&self, n: N) -> String {
        self.inner.format(n)
    }
    pub fn set_maximum_fraction_digits(&self, n: i32) {
        self.inner.set_maximum_fraction_digits(n);
    }
    pub fn get_maximum_fraction_digits(&self) -> i32 {
        self.inner.get_maximum_fraction_digits()
    }
    pub fn set_minimum_fraction_digits(&self, n: i32) {
        self.inner.set_minimum_fraction_digits(n);
    }
    pub fn get_minimum_fraction_digits(&self) -> i32 {
        self.inner.get_minimum_fraction_digits()
    }
    pub fn set_grouping_used(&self, b: bool) {
        self.inner.set_grouping_used(b);
    }
    pub fn is_grouping_used(&self) -> bool {
        self.inner.is_grouping_used()
    }
}

/// Java `java.text.DecimalFormatSymbols`. Minimal; the decimal separator is
/// stored but the formatter above uses `.` (the C locale).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct JavaDecimalFormatSymbols {
    decimal_separator: std::cell::Cell<char>,
    grouping_separator: std::cell::Cell<char>,
}
impl std::hash::Hash for JavaDecimalFormatSymbols {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.decimal_separator.get().hash(state);
        self.grouping_separator.get().hash(state);
    }
}
impl Default for JavaDecimalFormatSymbols {
    fn default() -> Self {
        JavaDecimalFormatSymbols {
            decimal_separator: std::cell::Cell::new('.'),
            grouping_separator: std::cell::Cell::new(','),
        }
    }
}
impl JavaDecimalFormatSymbols {
    pub fn new() -> Self {
        JavaDecimalFormatSymbols::default()
    }
    pub fn new_1<L>(_locale: L) -> Self {
        JavaDecimalFormatSymbols::default()
    }
    pub fn get_instance() -> JavaDecimalFormatSymbols {
        JavaDecimalFormatSymbols::default()
    }
    pub fn set_decimal_separator(&self, c: char) {
        self.decimal_separator.set(c);
    }
    pub fn get_decimal_separator(&self) -> char {
        self.decimal_separator.get()
    }
    pub fn set_grouping_separator(&self, c: char) {
        self.grouping_separator.set(c);
    }
    pub fn get_grouping_separator(&self) -> char {
        self.grouping_separator.get()
    }
}

#[cfg(test)]
mod decimal_format_tests {
    use super::*;
    #[test]
    fn fraction_digits_from_pattern() {
        let f = JavaDecimalFormat::new_1("0.00");
        assert_eq!(f.format(3.14159), "3.14");
    }
    #[test]
    fn grouping() {
        let f = JavaDecimalFormat::new_1("#,##0");
        assert_eq!(f.format(1234567.0), "1,234,567");
    }
    #[test]
    fn grouping_negative_with_fraction() {
        let f = JavaDecimalFormat::new_1("#,##0.0");
        assert_eq!(f.format(-1234567.5), "-1,234,567.5");
    }
    #[test]
    fn default_max_three_fraction() {
        let f = JavaDecimalFormat::new();
        assert_eq!(f.format(1.23456), "1.235");
    }
    #[test]
    fn min_fraction_pads() {
        let f = JavaDecimalFormat::new_1("0.00");
        assert_eq!(f.format(2.0), "2.00");
    }
    #[test]
    fn trim_to_min_above_min() {
        let f = JavaDecimalFormat::new_1("0.0##");
        assert_eq!(f.format(2.5), "2.5");
        assert_eq!(f.format(2.125), "2.125");
    }
    #[test]
    fn set_max_fraction_digits() {
        let f = JavaDecimalFormat::new();
        f.set_maximum_fraction_digits(1);
        assert_eq!(f.format(3.16), "3.2");
    }
    #[test]
    fn number_format_instance() {
        let f = JavaNumberFormat::get_instance();
        f.set_maximum_fraction_digits(2);
        assert_eq!(f.format(3.14159), "3.14");
    }
    #[test]
    fn integer_instance() {
        let f = JavaNumberFormat::get_integer_instance();
        assert_eq!(f.format(3.9), "4");
    }
    #[test]
    fn symbols() {
        let s = JavaDecimalFormatSymbols::new();
        s.set_decimal_separator(',');
        assert_eq!(s.get_decimal_separator(), ',');
    }
}
