interface EqualitySource<T> { T get(); }
interface EqualityBound<T> { boolean equals(EqualityBound<T> other); }
interface EqualityDefault<Unused, T> {
    default boolean same(T left, T right) { return left.equals(right); }
    default boolean different(T left, T right) { return !left.equals(right); }
}
class EqualityImplementation<Unused, T> implements EqualityDefault<Unused, T> {}
class ValueEquality {
    static <T> boolean same(T left, T right) { return left.equals(right); }
    static <T> boolean different(T left, T right) { return !left.equals(right); }
    static <T> boolean via(T left, T right) { return same(left, right); }
    static <T extends EqualityBound<T>> boolean bounded(T left, T right) {
        return left.equals(right);
    }
    static <T> boolean nullableRight(T left, T right) { return left.equals(right); }
    static boolean forceNullableRight(String left) { return nullableRight(left, null); }
    static <T> boolean from(T left, EqualitySource<T> right) {
        return left.equals(right.get());
    }
    static boolean nullable(EqualitySource<String> right) { return from(null, right); }
    static <T> boolean fromNot(T left, EqualitySource<T> right) {
        return !left.equals(right.get());
    }
    static boolean nullableNot(EqualitySource<String> right) { return fromNot(null, right); }
}
class OwnEquality {
    boolean equals(OwnEquality other) { return false; }
    static boolean run() {
        OwnEquality value = new OwnEquality();
        return value.equals(value);
    }
}
