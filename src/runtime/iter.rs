/// A Java external iterator over a snapshot of a collection.
pub struct JavaIter<T> {
    items: Vec<T>,
    pos: usize,
}

impl<T: Clone> JavaIter<T> {
    pub fn new<I: IntoIterator<Item = T>>(src: I) -> Self {
        JavaIter { items: src.into_iter().collect(), pos: 0 }
    }
    pub fn has_next(&self) -> bool {
        self.pos < self.items.len()
    }
    pub fn next(&mut self) -> Option<T> {
        let v = self.items.get(self.pos).cloned();
        if v.is_some() {
            self.pos += 1;
        }
        v
    }
    pub fn has_previous(&self) -> bool {
        self.pos > 0
    }
    pub fn previous(&mut self) -> Option<T> {
        if self.pos > 0 {
            self.pos -= 1;
            self.items.get(self.pos).cloned()
        } else {
            None
        }
    }
}

