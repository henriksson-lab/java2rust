/// The sequential, unknown-size Spliterators iterator capability. Cloning the
/// Java reference preserves the cursor position; it never clones its elements.
pub struct JavaIteratorSpliterator<T> {
    iterator: JavaIterator<T>,
    identity: std::rc::Rc<()>,
    characteristics: i32,
}

impl<T> Clone for JavaIteratorSpliterator<T> {
    fn clone(&self) -> Self {
        Self {
            iterator: self.iterator.clone(),
            identity: self.identity.clone(),
            characteristics: self.characteristics,
        }
    }
}

impl<T> PartialEq for JavaIteratorSpliterator<T> {
    fn eq(&self, other: &Self) -> bool {
        std::rc::Rc::ptr_eq(&self.identity, &other.identity)
    }
}
impl<T> Eq for JavaIteratorSpliterator<T> {}
impl<T> std::hash::Hash for JavaIteratorSpliterator<T> {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        std::hash::Hash::hash(&(std::rc::Rc::as_ptr(&self.identity) as usize), state);
    }
}
impl<T> std::fmt::Debug for JavaIteratorSpliterator<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("JavaIteratorSpliterator")
            .field("characteristics", &self.characteristics)
            .finish()
    }
}

impl<T> JavaIteratorSpliterator<T> {
    pub fn unknown_size_result(
        iterator: Option<JavaIterator<T>>,
        characteristics: i32,
    ) -> Result<Self, JavaException> {
        let iterator = iterator.ok_or_else(|| {
            JavaException::new("NullPointerException", "null iterator".to_string())
        })?;
        Ok(Self::unknown_size(iterator, characteristics))
    }

    pub fn unknown_size(iterator: JavaIterator<T>, characteristics: i32) -> Self {
        Self {
            iterator,
            identity: std::rc::Rc::new(()),
            // The JDK unknown-size factory always removes SIZED and SUBSIZED.
            characteristics: characteristics & !(0x0040 | 0x4000),
        }
    }

    pub fn characteristics(&self) -> i32 {
        self.characteristics
    }
    pub fn estimate_size(&self) -> i64 {
        i64::MAX
    }

    /// Do not hold the native cursor cell while calling Java hasNext/next.
    /// A live producer may reenter this capability or an alias of its owner.
    fn next_result(&self) -> Result<Option<T>, JavaException> {
        if !self.iterator.has_next_result()? {
            return Ok(None);
        }
        self.iterator.clone().next_result().map(Some)
    }

    pub fn try_advance_result<F>(&self, mut action: F) -> Result<bool, JavaException>
    where
        F: FnMut(T) -> Result<(), JavaException>,
    {
        let Some(value) = self.next_result()? else {
            return Ok(false);
        };
        action(value)?;
        Ok(true)
    }

    pub fn for_each_remaining_result<F>(&self, mut action: F) -> Result<(), JavaException>
    where
        F: FnMut(T) -> Result<(), JavaException>,
    {
        while self.try_advance_result(&mut action)? {}
        Ok(())
    }
}

impl<T: 'static> JavaIteratorSpliterator<T> {
    pub fn into_results(self) -> impl Iterator<Item = Result<T, JavaException>> {
        let mut finished = false;
        std::iter::from_fn(move || {
            if finished {
                return None;
            }
            let item = self.next_result().transpose();
            finished = item.as_ref().is_none_or(|item| item.is_err());
            item
        })
    }
}
