#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct BindingId {
    pub kind: BindingKind,
    pub name: String,
    pub occurrence: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum BindingKind {
    Param,
    Local,
    CatchParam,
    LoopVariable,
    Field,
    StaticField,
}

impl BindingId {
    pub fn new(kind: BindingKind, name: impl Into<String>, occurrence: usize) -> Self {
        Self {
            kind,
            name: name.into(),
            occurrence,
        }
    }

    pub fn compatibility_local(name: impl Into<String>) -> Self {
        Self::new(BindingKind::Local, name, 0)
    }

    pub fn name(&self) -> &str {
        &self.name
    }
}
