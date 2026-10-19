class QualifiedCounter implements java.util.function.IntSupplier {
    int value;
    QualifiedCounter(int initial) { value = initial; }
    public int getAsInt() { return value++; }
}

class QualifiedFunctionalValues {
    static int overflow() {
        java.util.function.BiFunction<Integer, Integer, Integer> sum = addition();
        return sum.apply(2147483647, 1) + 1;
    }
    static java.util.function.IntSupplier counter() {
        return new QualifiedCounter(4);
    }
    static java.util.function.BiFunction<Integer, Integer, Integer> addition() {
        return (left, right) -> left + right;
    }
    static java.util.Comparator<Integer> order() {
        return (left, right) -> left - right;
    }
    static int invoke(java.util.function.IntSupplier callback) {
        return callback.getAsInt();
    }
    static int run() {
        java.util.function.IntSupplier supplier = counter();
        java.util.function.BiFunction<Integer, Integer, Integer> sum = addition();
        java.util.Comparator<Integer> comparison = order();
        return invoke(supplier) * 100 + supplier.getAsInt() * 10
            + sum.apply(2, 3) + comparison.compare(3, 1);
    }
}
