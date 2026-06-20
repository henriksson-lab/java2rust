/// Java `java.util.BitSet` -> a `Vec<u64>` word-array backed bit vector. Methods
/// mirror `java.util.BitSet` semantics: indices are Java `int` (`i32`), the set
/// auto-grows on `set`/`flip`, and `length()`/`cardinality()`/`nextSetBit` follow
/// the JDK contract. Negative indices are treated as out-of-range no-ops/false
/// (the JDK throws; the translated code never relies on the exception).
#[derive(Clone, Default, Debug, PartialEq, Eq, Hash)]
pub struct JavaBitSet {
    words: Vec<u64>,
}
/// Bit-index argument. Java `BitSet` indices are `int`, but the JDK widens a
/// `char`/`short`/`byte` index to `int` implicitly, and the translator passes the
/// source expression's Rust type through unchanged — so accept the common index
/// types and normalize to `i32`.
pub trait BitIndex: Copy {
    fn bit_index(self) -> i32;
}
impl BitIndex for i32 {
    fn bit_index(self) -> i32 {
        self
    }
}
impl BitIndex for i64 {
    fn bit_index(self) -> i32 {
        self as i32
    }
}
impl BitIndex for usize {
    fn bit_index(self) -> i32 {
        self as i32
    }
}
impl BitIndex for u32 {
    fn bit_index(self) -> i32 {
        self as i32
    }
}
impl BitIndex for i16 {
    fn bit_index(self) -> i32 {
        self as i32
    }
}
impl BitIndex for i8 {
    fn bit_index(self) -> i32 {
        self as i32
    }
}
impl BitIndex for char {
    fn bit_index(self) -> i32 {
        self as i32
    }
}
impl JavaBitSet {
    /// `new BitSet()` — an empty set. The 1-arg `new BitSet(nbits)` reserves
    /// capacity for `nbits` bits (a hint only; the set still grows on demand).
    pub fn new() -> Self {
        JavaBitSet { words: Vec::new() }
    }
    pub fn new_2<I: BitIndex>(nbits: I) -> Self {
        let nbits = nbits.bit_index();
        let words = if nbits > 0 { (nbits as usize).div_ceil(64) } else { 0 };
        JavaBitSet { words: vec![0u64; words] }
    }
    fn ensure(&mut self, word: usize) {
        if word >= self.words.len() {
            self.words.resize(word + 1, 0);
        }
    }
    /// `get(i)` — is bit `i` set?
    pub fn get<I: BitIndex>(&self, i: I) -> bool {
        let i = i.bit_index();
        if i < 0 {
            return false;
        }
        let i = i as usize;
        let w = i / 64;
        w < self.words.len() && (self.words[w] >> (i % 64)) & 1 == 1
    }
    /// `set(i)` — set bit `i` to true (1-arg overload).
    pub fn set<I: BitIndex>(&mut self, i: I) {
        let i = i.bit_index();
        if i < 0 {
            return;
        }
        let i = i as usize;
        let w = i / 64;
        self.ensure(w);
        self.words[w] |= 1u64 << (i % 64);
    }
    /// `set(i, value)` — set bit `i` to `value` (2-arg overload -> `set_2`).
    pub fn set_2<I: BitIndex>(&mut self, i: I, value: bool) {
        if value {
            self.set(i);
        } else {
            self.clear(i);
        }
    }
    /// `set(from, to)` — set bits in `[from, to)` (3-arg overload -> `set_3`).
    pub fn set_3<I: BitIndex, J: BitIndex>(&mut self, from: I, to: J) {
        for i in from.bit_index()..to.bit_index() {
            self.set(i);
        }
    }
    /// `clear(i)` — clear bit `i` (1-arg overload).
    pub fn clear<I: BitIndex>(&mut self, i: I) {
        let i = i.bit_index();
        if i < 0 {
            return;
        }
        let i = i as usize;
        let w = i / 64;
        if w < self.words.len() {
            self.words[w] &= !(1u64 << (i % 64));
        }
    }
    /// `clear()` — clear all bits (0-arg overload; translator keeps the base name
    /// for the first overload, so a no-arg `clear()` lands here only if it is the
    /// sole `clear`; the indexed forms below cover the common cases).
    pub fn clear_all(&mut self) {
        self.words.clear();
    }
    /// `clear(from, to)` — clear bits in `[from, to)` (2-arg overload -> `clear_2`).
    pub fn clear_2<I: BitIndex, J: BitIndex>(&mut self, from: I, to: J) {
        for i in from.bit_index()..to.bit_index() {
            self.clear(i);
        }
    }
    /// `flip(i)` — toggle bit `i` (1-arg overload).
    pub fn flip<I: BitIndex>(&mut self, i: I) {
        let i = i.bit_index();
        if i < 0 {
            return;
        }
        let i = i as usize;
        let w = i / 64;
        self.ensure(w);
        self.words[w] ^= 1u64 << (i % 64);
    }
    /// `flip(from, to)` — toggle bits in `[from, to)` (2-arg overload -> `flip_2`).
    pub fn flip_2<I: BitIndex, J: BitIndex>(&mut self, from: I, to: J) {
        for i in from.bit_index()..to.bit_index() {
            self.flip(i);
        }
    }
    /// `cardinality()` — number of set bits.
    pub fn cardinality(&self) -> i32 {
        self.words.iter().map(|w| w.count_ones() as i32).sum()
    }
    /// `length()` — index of the highest set bit, plus one (0 if empty).
    pub fn length(&self) -> i32 {
        for (wi, w) in self.words.iter().enumerate().rev() {
            if *w != 0 {
                return (wi * 64) as i32 + (64 - w.leading_zeros() as i32);
            }
        }
        0
    }
    /// `size()` — the number of bits of allocated storage (multiple of 64).
    pub fn size(&self) -> i32 {
        (self.words.len() * 64).max(64) as i32
    }
    /// `isEmpty()` — true if no bit is set.
    pub fn is_empty(&self) -> bool {
        self.words.iter().all(|w| *w == 0)
    }
    /// `nextSetBit(from)` — index of the first set bit at or after `from`, or -1.
    pub fn next_set_bit<I: BitIndex>(&self, from: I) -> i32 {
        let from = from.bit_index();
        let from = if from < 0 { 0 } else { from as usize };
        let mut w = from / 64;
        if w >= self.words.len() {
            return -1;
        }
        // mask off bits below `from` in the first word.
        let mut word = self.words[w] & (!0u64 << (from % 64));
        loop {
            if word != 0 {
                return (w * 64) as i32 + word.trailing_zeros() as i32;
            }
            w += 1;
            if w >= self.words.len() {
                return -1;
            }
            word = self.words[w];
        }
    }
    /// `nextClearBit(from)` — index of the first clear bit at or after `from`
    /// (never -1: the conceptual set is infinite past the highest word).
    pub fn next_clear_bit<I: BitIndex>(&self, from: I) -> i32 {
        let from = from.bit_index();
        let from = if from < 0 { 0 } else { from as usize };
        let mut w = from / 64;
        if w >= self.words.len() {
            return from as i32;
        }
        let mut word = !self.words[w] & (!0u64 << (from % 64));
        loop {
            if word != 0 {
                return (w * 64) as i32 + word.trailing_zeros() as i32;
            }
            w += 1;
            if w >= self.words.len() {
                return (w * 64) as i32;
            }
            word = !self.words[w];
        }
    }
    /// `and(other)` — in-place intersection.
    pub fn and(&mut self, other: &JavaBitSet) {
        if other.words.len() < self.words.len() {
            self.words.truncate(other.words.len());
        }
        for (i, w) in self.words.iter_mut().enumerate() {
            *w &= other.words.get(i).copied().unwrap_or(0);
        }
    }
    /// `or(other)` — in-place union.
    pub fn or(&mut self, other: &JavaBitSet) {
        if other.words.len() > self.words.len() {
            self.words.resize(other.words.len(), 0);
        }
        for (i, w) in other.words.iter().enumerate() {
            self.words[i] |= *w;
        }
    }
    /// `xor(other)` — in-place symmetric difference.
    pub fn xor(&mut self, other: &JavaBitSet) {
        if other.words.len() > self.words.len() {
            self.words.resize(other.words.len(), 0);
        }
        for (i, w) in other.words.iter().enumerate() {
            self.words[i] ^= *w;
        }
    }
    /// `andNot(other)` — clear in self every bit set in `other`.
    pub fn and_not(&mut self, other: &JavaBitSet) {
        for (i, w) in self.words.iter_mut().enumerate() {
            *w &= !other.words.get(i).copied().unwrap_or(0);
        }
    }
    /// `intersects(other)` — true if the two sets share any set bit.
    pub fn intersects(&self, other: &JavaBitSet) -> bool {
        self.words
            .iter()
            .zip(other.words.iter())
            .any(|(a, b)| a & b != 0)
    }
    /// `get(from, to)` — a new BitSet of bits `[from, to)`, re-based to 0
    /// (2-arg overload -> `get_2`).
    pub fn get_2<I: BitIndex, J: BitIndex>(&self, from: I, to: J) -> JavaBitSet {
        let mut out = JavaBitSet::new();
        let (from, to) = (from.bit_index().max(0), to.bit_index().max(0));
        for i in from..to {
            if self.get(i) {
                out.set(i - from);
            }
        }
        out
    }
}
impl std::fmt::Display for JavaBitSet {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{{")?;
        let mut first = true;
        let mut i = self.next_set_bit(0);
        while i >= 0 {
            if !first {
                write!(f, ", ")?;
            }
            write!(f, "{}", i)?;
            first = false;
            i = self.next_set_bit(i + 1);
        }
        write!(f, "}}")
    }
}

#[cfg(test)]
mod bitset_tests {
    use super::*;

    #[test]
    fn set_get_across_words() {
        let mut b = JavaBitSet::new();
        for &i in &[0, 63, 64, 130] {
            assert!(!b.get(i));
            b.set(i);
            assert!(b.get(i), "bit {i} should be set");
        }
        assert!(!b.get(1));
        assert!(!b.get(62));
        assert!(!b.get(65));
        assert!(!b.get(129));
        assert_eq!(b.cardinality(), 4);
    }

    #[test]
    fn set_2_clear_flip() {
        let mut b = JavaBitSet::new();
        b.set_2(10, true);
        assert!(b.get(10));
        b.set_2(10, false);
        assert!(!b.get(10));
        b.flip(64);
        assert!(b.get(64));
        b.flip(64);
        assert!(!b.get(64));
        b.set(5);
        b.clear(5);
        assert!(!b.get(5));
    }

    #[test]
    fn length_and_size() {
        let mut b = JavaBitSet::new();
        assert_eq!(b.length(), 0);
        assert!(b.is_empty());
        b.set(0);
        assert_eq!(b.length(), 1);
        b.set(130);
        assert_eq!(b.length(), 131);
        assert!(!b.is_empty());
        assert!(b.size() >= 192);
    }

    #[test]
    fn next_set_bit_across_words() {
        let mut b = JavaBitSet::new();
        b.set(64);
        b.set(130);
        assert_eq!(b.next_set_bit(0), 64);
        assert_eq!(b.next_set_bit(64), 64);
        assert_eq!(b.next_set_bit(65), 130);
        assert_eq!(b.next_set_bit(131), -1);
        let empty = JavaBitSet::new();
        assert_eq!(empty.next_set_bit(0), -1);
    }

    #[test]
    fn logical_ops() {
        let mut a = JavaBitSet::new();
        a.set(1);
        a.set(64);
        let mut c = JavaBitSet::new();
        c.set(64);
        c.set(200);
        let mut and = a.clone();
        and.and(&c);
        assert!(and.get(64) && !and.get(1) && !and.get(200));
        let mut or = a.clone();
        or.or(&c);
        assert!(or.get(1) && or.get(64) && or.get(200));
        let mut xor = a.clone();
        xor.xor(&c);
        assert!(xor.get(1) && !xor.get(64) && xor.get(200));
        let mut an = a.clone();
        an.and_not(&c);
        assert!(an.get(1) && !an.get(64));
        assert!(a.intersects(&c));
    }

    #[test]
    fn ctor_nbits_and_range_ops() {
        let mut b = JavaBitSet::new_2(200);
        assert!(b.is_empty());
        b.set_3(10, 13);
        assert!(b.get(10) && b.get(11) && b.get(12) && !b.get(13));
        b.clear_2(11, 12);
        assert!(b.get(10) && !b.get(11) && b.get(12));
        let sub = b.get_2(10, 13);
        assert!(sub.get(0) && !sub.get(1) && sub.get(2));
    }
}
