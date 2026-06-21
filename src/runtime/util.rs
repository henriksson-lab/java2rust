/// Helpers backing the `java.util.Arrays` / `java.util.Collections` declarative
/// rewrite rules (`src/stdlib.rs`). Each is a free function so the rule template
/// stays a one-liner. Bounds are deliberately permissive to match how the
/// translator passes arguments (owned `Vec`s and borrowed slices both deref to
/// `&[T]`).

/// `Arrays.copyOf(arr, n)` — a length-`n` copy, truncated or `Default`-padded
/// (Java zero/null-fills the tail).
pub fn java_array_copy_of<T: Clone + Default>(src: &[T], new_len: i64) -> Vec<T> {
    let n = new_len.max(0) as usize;
    let mut v: Vec<T> = Vec::with_capacity(n);
    v.extend(src.iter().take(n).cloned());
    while v.len() < n {
        v.push(T::default());
    }
    v
}

/// `Arrays.binarySearch(arr, key)` — JDK semantics: index of a match, else
/// `-(insertion_point) - 1`. Requires a sorted slice (the caller's contract).
pub fn java_binary_search<T: PartialOrd>(arr: &[T], key: &T) -> i32 {
    let mut lo: i64 = 0;
    let mut hi: i64 = arr.len() as i64 - 1;
    while lo <= hi {
        let mid = ((lo + hi) >> 1) as usize;
        if &arr[mid] < key {
            lo = mid as i64 + 1;
        } else if &arr[mid] > key {
            hi = mid as i64 - 1;
        } else {
            return mid as i32;
        }
    }
    (-(lo + 1)) as i32
}
