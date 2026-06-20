/// Java `java.util.StringTokenizer` -> eager tokenization into a queue.
/// Delimiters are a *set of characters* (Java semantics: any char in the set
/// separates). Empty tokens are skipped unless `return_delims` is set, in which
/// case the delimiter characters are themselves returned as single-char tokens.
#[derive(Clone, Default, Debug)]
pub struct JavaStringTokenizer {
    tokens: std::collections::VecDeque<String>,
}

impl JavaStringTokenizer {
    /// 1-arg ctor: default delimiters are whitespace.
    pub fn new<S: ToString>(s: S) -> Self {
        Self::new_3(s, " \t\n\r\x0C", false)
    }
    /// 2-arg ctor: each character in `delim` is a delimiter.
    pub fn new_2<S: ToString, D: ToString>(s: S, delim: D) -> Self {
        Self::new_3(s, delim, false)
    }
    /// 3-arg ctor: when `return_delims` is true the delimiter characters are also
    /// returned as one-character tokens.
    pub fn new_3<S: ToString, D: ToString>(s: S, delim: D, return_delims: bool) -> Self {
        let s = s.to_string();
        let delims: std::collections::HashSet<char> = delim.to_string().chars().collect();
        let mut tokens = std::collections::VecDeque::new();
        let mut cur = String::new();
        for c in s.chars() {
            if delims.contains(&c) {
                if !cur.is_empty() {
                    tokens.push_back(std::mem::take(&mut cur));
                }
                if return_delims {
                    tokens.push_back(c.to_string());
                }
            } else {
                cur.push(c);
            }
        }
        if !cur.is_empty() {
            tokens.push_back(cur);
        }
        JavaStringTokenizer { tokens }
    }
    pub fn has_more_tokens(&self) -> bool {
        !self.tokens.is_empty()
    }
    pub fn next_token(&mut self) -> String {
        self.tokens.pop_front().unwrap_or_default()
    }
    pub fn count_tokens(&self) -> i32 {
        self.tokens.len() as i32
    }
    /// Alias of `has_more_tokens` (`Enumeration` interface).
    pub fn has_more_elements(&self) -> bool {
        self.has_more_tokens()
    }
    /// Alias of `next_token` (`Enumeration` interface).
    pub fn next_element(&mut self) -> String {
        self.next_token()
    }
}

#[cfg(test)]
mod string_tokenizer_tests {
    use super::*;
    #[test]
    fn whitespace_default() {
        let mut t = JavaStringTokenizer::new("a b  c");
        assert_eq!(t.count_tokens(), 3);
        assert_eq!(t.next_token(), "a");
        assert_eq!(t.next_token(), "b");
        assert_eq!(t.next_token(), "c");
        assert!(!t.has_more_tokens());
    }
    #[test]
    fn custom_delims_and_return_delims() {
        assert_eq!(JavaStringTokenizer::new_2("a,b;c", ",;").count_tokens(), 3);
        assert_eq!(JavaStringTokenizer::new_3("a,b", ",", true).count_tokens(), 3);
    }
}
