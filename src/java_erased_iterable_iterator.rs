/// The typed and erased projections share the same canonical cursor state.
/// The typed protocol releases its state loan before invoking Java callbacks.
impl JavaErasedIterator {
    pub fn from_typed<T: 'static, E>(cursor: JavaIterator<T>, encode: E) -> Self
    where
        E: Fn(T) -> Option<Box<dyn std::any::Any>> + 'static,
    {
        let has_cursor = cursor.clone();
        let next_cursor = cursor.clone();
        Self::from_operations(
            move || has_cursor.has_next_result(),
            move || Ok(encode(next_cursor.clone().next_result()?)),
            move || cursor.clone().remove_result(),
        )
    }
}
