// java.util.regex -> the `regex` crate.
//
// `JavaPattern` wraps a compiled `regex::Regex`; `JavaMatcher` is the stateful
// match cursor a `Pattern.matcher(input)` produces. Java's regex dialect is a
// superset of Rust's (lookahead/backreferences have no `regex`-crate analog), so
// compilation is best-effort: a pattern the crate rejects falls back to a regex
// that never matches, keeping generated code compiling AND running rather than
// panicking.
//
// State: `JavaMatcher` advances across `find()`/`matches()` calls. A Java field /
// `static final` of these types is reached via `&Self`, so the cursor and last
// match live in `Cell`/`RefCell` and every method takes `&self` (an `&mut self`
// matcher would E0596 at field/static call sites — see io_read.rs).

/// `java.util.regex.Pattern` — a compiled regular expression.
#[derive(Clone)]
pub struct JavaPattern {
    re: regex::Regex,
    /// The original Java pattern string (for `toString()` / re-compilation under
    /// whole-input `matches()` semantics).
    src: String,
}

impl JavaPattern {
    /// Java `Pattern.CASE_INSENSITIVE` flag bit.
    pub const CASE_INSENSITIVE: i32 = 0x02;

    fn build(regex: &str, case_insensitive: bool) -> JavaPattern {
        // Prepend `(?i)` for the case-insensitive flag.
        let pat = if case_insensitive { format!("(?i){regex}") } else { regex.to_string() };
        let re = regex::Regex::new(&pat).unwrap_or_else(|_| {
            // A pattern the `regex` crate can't handle (lookahead, backrefs, …):
            // fall back to one that matches nothing rather than panicking.
            regex::Regex::new(r"\b\B").unwrap()
        });
        JavaPattern { re, src: regex.to_string() }
    }

    /// `Pattern.compile(String regex)`.
    pub fn compile<S: ToString>(regex: S) -> JavaPattern {
        JavaPattern::build(&regex.to_string(), false)
    }

    /// `Pattern.compile(String regex, int flags)`.
    pub fn compile_2<S: ToString>(regex: S, flags: i32) -> JavaPattern {
        JavaPattern::build(&regex.to_string(), flags & Self::CASE_INSENSITIVE != 0)
    }

    /// `Pattern.matcher(CharSequence input)` -> a fresh matcher over `input`.
    pub fn matcher<S: ToString>(&self, input: S) -> JavaMatcher {
        JavaMatcher::new(self.clone(), input.to_string())
    }

    /// `Pattern.split(CharSequence input)` -> the substrings between matches.
    pub fn split<S: ToString>(&self, input: S) -> Vec<String> {
        let s = input.to_string();
        let mut parts: Vec<String> = self.re.split(&s).map(|p| p.to_string()).collect();
        // Java drops trailing empty strings (limit 0 default).
        while parts.len() > 1 && parts.last().map(|p| p.is_empty()).unwrap_or(false) {
            parts.pop();
        }
        parts
    }

    /// `Pattern.split(CharSequence input, int limit)`.
    pub fn split_2<S: ToString>(&self, input: S, limit: i32) -> Vec<String> {
        let s = input.to_string();
        if limit > 0 {
            self.re.splitn(&s, limit as usize).map(|p| p.to_string()).collect()
        } else if limit == 0 {
            self.split(s)
        } else {
            self.re.split(&s).map(|p| p.to_string()).collect()
        }
    }

    /// `Pattern.pattern()` / `toString()` -> the source regex string.
    pub fn pattern(&self) -> String {
        self.src.clone()
    }

    /// `Pattern.quote(String s)` (static) -> a literal-matching regex.
    pub fn quote<S: ToString>(s: S) -> String {
        regex::escape(&s.to_string())
    }

    /// `Pattern.matches(String regex, CharSequence input)` (static) -> whole-input
    /// match.
    pub fn matches_static<R: ToString, S: ToString>(regex: R, input: S) -> bool {
        JavaPattern::compile(regex).matcher(input).matches()
    }
}

impl std::fmt::Display for JavaPattern {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        f.write_str(&self.src)
    }
}
impl std::fmt::Debug for JavaPattern {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "JavaPattern({:?})", self.src)
    }
}
impl Default for JavaPattern {
    fn default() -> Self {
        JavaPattern::compile("")
    }
}
impl PartialEq for JavaPattern {
    fn eq(&self, other: &Self) -> bool {
        self.src == other.src
    }
}
impl Eq for JavaPattern {}
impl std::hash::Hash for JavaPattern {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.src.hash(state);
    }
}

/// `java.util.regex.Matcher` — a stateful match cursor over one input string.
#[derive(Clone)]
pub struct JavaMatcher {
    pattern: JavaPattern,
    input: String,
    /// Byte offset where the next `find()` starts.
    region_start: std::cell::Cell<usize>,
    /// Captured group byte-spans of the last successful match (group 0 = whole
    /// match); `None` between/before matches.
    last: std::rc::Rc<std::cell::RefCell<Option<Vec<Option<(usize, usize)>>>>>,
}

impl JavaMatcher {
    fn new(pattern: JavaPattern, input: String) -> Self {
        JavaMatcher {
            pattern,
            input,
            region_start: std::cell::Cell::new(0),
            last: std::rc::Rc::new(std::cell::RefCell::new(None)),
        }
    }

    fn record(&self, caps: &regex::Captures) {
        let spans: Vec<Option<(usize, usize)>> =
            caps.iter().map(|m| m.map(|mm| (mm.start(), mm.end()))).collect();
        *self.last.borrow_mut() = Some(spans);
    }

    /// `Matcher.find()` -> advance to the next match; returns whether one was
    /// found.
    pub fn find(&self) -> bool {
        let start = self.region_start.get().min(self.input.len());
        match self.pattern.re.captures_at(&self.input, start) {
            Some(caps) => {
                let whole = caps.get(0).unwrap();
                // Advance past this match (avoid an infinite loop on empty matches).
                let next = if whole.end() > whole.start() { whole.end() } else { whole.end() + 1 };
                self.region_start.set(next);
                self.record(&caps);
                true
            }
            None => {
                *self.last.borrow_mut() = None;
                false
            }
        }
    }

    /// `Matcher.matches()` -> does the WHOLE input match (anchored both ends)?
    pub fn matches(&self) -> bool {
        // Anchor the source pattern at both ends and try once.
        let anchored = format!("^(?:{})$", self.pattern.src);
        let re = match regex::Regex::new(&anchored) {
            Ok(r) => r,
            Err(_) => return false,
        };
        match re.captures(&self.input) {
            Some(caps) => {
                self.record(&caps);
                true
            }
            None => {
                *self.last.borrow_mut() = None;
                false
            }
        }
    }

    /// `Matcher.lookingAt()` -> does the input match starting at the beginning
    /// (not necessarily to the end)?
    pub fn looking_at(&self) -> bool {
        match self.pattern.re.captures_at(&self.input, 0) {
            Some(caps) if caps.get(0).map(|m| m.start() == 0).unwrap_or(false) => {
                self.record(&caps);
                true
            }
            _ => {
                *self.last.borrow_mut() = None;
                false
            }
        }
    }

    fn span(&self, group: usize) -> Option<(usize, usize)> {
        self.last.borrow().as_ref().and_then(|v| v.get(group).copied().flatten())
    }

    /// `Matcher.group()` -> the whole last match.
    pub fn group(&self) -> String {
        self.group_1(0)
    }

    /// `Matcher.group(int group)` -> the n-th captured group (empty string if the
    /// group did not participate — Java returns null, but the translator's String
    /// channel has no null, so empty is the safe lowering).
    pub fn group_1(&self, group: i32) -> String {
        match self.span(group.max(0) as usize) {
            Some((s, e)) => self.input[s..e].to_string(),
            None => String::new(),
        }
    }

    /// `Matcher.start()` -> start index of the last match.
    pub fn start(&self) -> i32 {
        self.span(0).map(|(s, _)| s as i32).unwrap_or(-1)
    }
    /// `Matcher.start(int group)`.
    pub fn start_1(&self, group: i32) -> i32 {
        self.span(group.max(0) as usize).map(|(s, _)| s as i32).unwrap_or(-1)
    }
    /// `Matcher.end()` -> end index of the last match.
    pub fn end(&self) -> i32 {
        self.span(0).map(|(_, e)| e as i32).unwrap_or(-1)
    }
    /// `Matcher.end(int group)`.
    pub fn end_1(&self, group: i32) -> i32 {
        self.span(group.max(0) as usize).map(|(_, e)| e as i32).unwrap_or(-1)
    }

    /// `Matcher.groupCount()` -> number of capturing groups in the pattern.
    pub fn group_count(&self) -> i32 {
        self.pattern.re.captures_len() as i32 - 1
    }

    /// `Matcher.replaceAll(String replacement)` -> replace every match.
    pub fn replace_all<S: ToString>(&self, replacement: S) -> String {
        self.pattern
            .re
            .replace_all(&self.input, Self::convert_repl(&replacement.to_string()).as_str())
            .into_owned()
    }

    /// `Matcher.replaceFirst(String replacement)` -> replace the first match.
    pub fn replace_first<S: ToString>(&self, replacement: S) -> String {
        self.pattern
            .re
            .replace(&self.input, Self::convert_repl(&replacement.to_string()).as_str())
            .into_owned()
    }

    /// Java replacement strings use `$1`/`${name}`; the `regex` crate uses the
    /// same `$1`/`${name}` syntax, so they coincide. Java `\$` escapes to a
    /// literal `$`; the crate uses `$$`. Convert here.
    fn convert_repl(repl: &str) -> String {
        repl.replace("\\$", "$$")
    }

    /// `Matcher.reset()` -> rewind the cursor to the start.
    pub fn reset(&self) -> &Self {
        self.region_start.set(0);
        *self.last.borrow_mut() = None;
        self
    }

    /// `Matcher.pattern()` -> the originating pattern.
    pub fn pattern(&self) -> JavaPattern {
        self.pattern.clone()
    }
}

impl std::fmt::Display for JavaMatcher {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        f.write_str(&self.input)
    }
}
impl std::fmt::Debug for JavaMatcher {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "JavaMatcher({:?})", self.input)
    }
}
impl Default for JavaMatcher {
    fn default() -> Self {
        JavaMatcher::new(JavaPattern::default(), String::new())
    }
}
impl PartialEq for JavaMatcher {
    fn eq(&self, other: &Self) -> bool {
        self.pattern == other.pattern && self.input == other.input
    }
}
impl Eq for JavaMatcher {}
impl std::hash::Hash for JavaMatcher {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.pattern.hash(state);
        self.input.hash(state);
    }
}

#[cfg(test)]
mod regex_tests {
    use super::*;

    #[test]
    fn compile_matcher_find_group() {
        let p = JavaPattern::compile(r"(\d+)-(\d+)");
        let m = p.matcher("ab 12-34 cd 56-78");
        assert!(m.find());
        assert_eq!(m.group(), "12-34");
        assert_eq!(m.group_1(1), "12");
        assert_eq!(m.group_1(2), "34");
        assert!(m.find());
        assert_eq!(m.group_1(1), "56");
        assert!(!m.find());
    }

    #[test]
    fn matches_whole_input() {
        let p = JavaPattern::compile(r"\w+/\w*");
        assert!(p.matcher("text/html").matches());
        assert!(!p.matcher("text/html extra").matches());
    }

    #[test]
    fn case_insensitive_flag() {
        let p = JavaPattern::compile_2("abc", JavaPattern::CASE_INSENSITIVE);
        assert!(p.matcher("xxABCxx").find());
    }

    #[test]
    fn replace_all_and_first() {
        let p = JavaPattern::compile("[\"']");
        assert_eq!(p.matcher("a\"b'c").replace_all(""), "abc");
        let q = JavaPattern::compile(r"^\+");
        assert_eq!(q.matcher("+42").replace_first(""), "42");
    }

    #[test]
    fn group_backref_replacement() {
        let p = JavaPattern::compile(r"(\w)(\w)");
        assert_eq!(p.matcher("ab").replace_all("$2$1"), "ba");
    }

    #[test]
    fn split_default_and_limit() {
        let p = JavaPattern::compile(",");
        assert_eq!(p.split("a,b,c"), vec!["a", "b", "c"]);
        assert_eq!(p.split("a,b,,"), vec!["a", "b"]); // trailing empties dropped
        assert_eq!(p.split_2("a,b,c", 2), vec!["a", "b,c"]);
    }

    #[test]
    fn start_end_group_count() {
        let p = JavaPattern::compile(r"(b)(c)");
        let m = p.matcher("abcd");
        assert!(m.find());
        assert_eq!(m.start(), 1);
        assert_eq!(m.end(), 3);
        assert_eq!(m.group_count(), 2);
    }

    #[test]
    fn bad_pattern_never_matches() {
        // Lookahead is rejected by the crate -> fall back to never-match (no panic).
        let p = JavaPattern::compile(r"(?=foo)bar");
        assert!(!p.matcher("foobar").find());
        assert!(!p.matcher("foobar").matches());
    }

    #[test]
    fn quote_escapes() {
        let q = JavaPattern::quote("a.b*c");
        let p = JavaPattern::compile(&q);
        assert!(p.matcher("a.b*c").matches());
        assert!(!p.matcher("axbyc").matches());
    }
}
