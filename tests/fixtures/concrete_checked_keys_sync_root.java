// A Java static root makes every retained ConcreteCheckedKey use the sync
// carrier. This exercises real storage analysis, not a hand-edited Rust ABI.
class ConcreteCheckedKeysSyncRoot {
    static ConcreteCheckedKey key;
}
