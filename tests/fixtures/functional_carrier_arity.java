import java.util.function.BiFunction;

class FunctionSlot<A, B, R> {
    private final BiFunction<A, B, R> callback;
    FunctionSlot(BiFunction<A, B, R> callback) { this.callback = callback; }
    R apply(A first, B second) { return callback.apply(first, second); }
    R applyTwice(A first, B second) {
        callback.apply(first, second);
        return callback.apply(first, second);
    }
}

class FunctionalCarrierArity {
    static BiFunction<Integer, Integer, Integer> addition() {
        return (first, second) -> first + second;
    }
    static int sum(int first, int second) {
        return new FunctionSlot<Integer, Integer, Integer>(addition()).apply(first, second);
    }
}
