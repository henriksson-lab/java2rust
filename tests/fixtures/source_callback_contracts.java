import java.util.function.IntPredicate;
import java.util.function.ToIntFunction;

class CallbackCell {
    private int value;
    CallbackCell(int value) { this.value = value; }
    void set(int value) { this.value = value; }
    int get() { return value; }
}

class SourceCallbackContracts {
    static int invoke(ToIntFunction<CallbackCell> function, CallbackCell value) {
        return function.applyAsInt(value);
    }
    static int aliased() {
        CallbackCell value = new CallbackCell(1);
        CallbackCell alias = value;
        alias.set(4);
        return invoke(item -> item.get(), value);
    }
    static boolean check(IntPredicate predicate, int value) {
        return predicate.test(value);
    }
    static boolean checked(int value) {
        if (value < 0) throw new IllegalArgumentException("negative");
        return value > 0;
    }
    static boolean lambda(int value) {
        return check(item -> checked(item), value);
    }
    static boolean reference(int value) {
        return check(SourceCallbackContracts::checked, value);
    }
    static boolean forward(IntPredicate predicate, int value) {
        return check(predicate, value);
    }
    static boolean forwarded(int value) {
        return forward(item -> checked(item), value);
    }
    static boolean plain(int value) {
        return check(item -> item > 0, value);
    }
    static boolean caught(int value) {
        try { return lambda(value); }
        catch (IllegalArgumentException error) { return false; }
    }
}
