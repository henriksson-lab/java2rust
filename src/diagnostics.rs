#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Span {
    start: usize,
    end: usize,
}

impl Span {
    pub fn new(start: usize, end: usize) -> Self {
        Self { start, end }
    }

    pub fn start(&self) -> usize {
        self.start
    }

    pub fn end(&self) -> usize {
        self.end
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    Error,
    Warning,
    Unsupported,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagnostic {
    severity: Severity,
    code: &'static str,
    message: String,
    span: Option<Span>,
}

impl Diagnostic {
    pub fn severity(&self) -> Severity {
        self.severity
    }

    pub fn code(&self) -> &'static str {
        self.code
    }

    pub fn message(&self) -> &str {
        &self.message
    }

    pub fn span(&self) -> Option<Span> {
        self.span
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DiagnosticBag {
    diagnostics: Vec<Diagnostic>,
}

impl DiagnosticBag {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn push(
        &mut self,
        severity: Severity,
        code: &'static str,
        message: impl Into<String>,
        span: Option<Span>,
    ) {
        self.diagnostics.push(Diagnostic {
            severity,
            code,
            message: message.into(),
            span,
        });
    }

    pub fn unsupported(&mut self, code: &'static str, message: impl Into<String>) {
        self.push(Severity::Unsupported, code, message, None);
    }

    pub fn warning(&mut self, code: &'static str, message: impl Into<String>) {
        self.push(Severity::Warning, code, message, None);
    }

    pub fn error(&mut self, code: &'static str, message: impl Into<String>) {
        self.push(Severity::Error, code, message, None);
    }

    pub fn extend(&mut self, other: DiagnosticBag) {
        self.diagnostics.extend(other.diagnostics);
    }

    pub fn has_errors(&self) -> bool {
        self.diagnostics
            .iter()
            .any(|d| matches!(d.severity, Severity::Error))
    }

    pub fn has_code(&self, code: &str) -> bool {
        self.diagnostics.iter().any(|d| d.code == code)
    }

    pub fn is_empty(&self) -> bool {
        self.diagnostics.is_empty()
    }

    pub fn iter(&self) -> impl Iterator<Item = &Diagnostic> {
        self.diagnostics.iter()
    }

    pub fn len(&self) -> usize {
        self.diagnostics.len()
    }
}
