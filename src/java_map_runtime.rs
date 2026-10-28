/// Copy one stored entry at a time, releasing the source loan before Java
/// code consumes it. Copiers are compiler-selected reference operations.
pub fn java_native_map_entry_iterator_result<K: 'static, V: 'static, H>(
    source: &H,
    copy_key: fn(&K) -> K,
    copy_value: fn(&V) -> V,
) -> Result<JavaIterator<(K, V)>, JavaException>
where
    H: JavaSharedAllocationAccess<Value = JavaSemanticHashMap<K, V>> + 'static,
{
    let (limit, expected) = source.java_read(|map| (map.len(), map.modification_count()));
    let index = std::rc::Rc::new(std::cell::Cell::new(0usize));
    let has_index = index.clone();
    let has_source = source.clone();
    let next_source = source.clone();
    Ok(JavaIterator::new_live(
        move || {
            has_source.java_read(|map| {
                if map.modification_count() != expected {
                    return Err(JavaException::new(
                        "ConcurrentModificationException",
                        "Map allocation changed".to_string(),
                    ));
                }
                Ok(has_index.get() < limit)
            })
        },
        move || {
            next_source.java_read(|map| {
                if map.modification_count() != expected {
                    return Err(JavaException::new(
                        "ConcurrentModificationException",
                        "Map allocation changed".to_string(),
                    ));
                }
                let (key, value) = map.entry_at(index.get()).ok_or_else(|| {
                    JavaException::new(
                        "NoSuchElementException",
                        "Map iterator exhausted".to_string(),
                    )
                })?;
                let entry = (copy_key(key), copy_value(value));
                index.set(index.get() + 1);
                Ok(entry)
            })
        },
        || {
            Err(JavaException::new(
                "UnsupportedOperationException",
                "Read-only entry cursor".to_string(),
            ))
        },
    ))
}

/// Admitted only for key contracts whose equality/ordering cannot invoke Java
/// callbacks. Source and destination can alias: each read ends before a write.
pub fn java_native_pure_key_map_put_all_result<K: JavaMapKey + 'static, V: 'static, D, S>(
    destination: &D,
    source: &S,
    copy_key: fn(&K) -> K,
    copy_value: fn(&V) -> V,
) -> Result<(), JavaException>
where
    D: JavaSharedAllocationAccess<Value = JavaSemanticHashMap<K, V>> + 'static,
    S: JavaSharedAllocationAccess<Value = JavaSemanticHashMap<K, V>> + 'static,
{
    let mut cursor = java_native_map_entry_iterator_result(source, copy_key, copy_value)?;
    while cursor.has_next_result()? {
        let (key, value) = cursor.next_result()?;
        destination.java_write(|map| {
            map.insert(key, value);
        });
    }
    Ok(())
}
