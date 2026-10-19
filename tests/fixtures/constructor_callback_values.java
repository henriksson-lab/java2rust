import java.util.function.Supplier;
import java.util.function.IntSupplier;
import java.util.Arrays;
import java.util.Iterator;
import java.util.List;

class CallbackValueHolder {
    private Supplier<Integer> boxed;
    private IntSupplier primitive;

    CallbackValueHolder() { this(() -> 8, () -> 1); }

    CallbackValueHolder(Supplier<Integer> boxed, IntSupplier primitive) {
        this.boxed = boxed;
        this.primitive = primitive;
    }

    int read() { return boxed.get() + primitive.getAsInt(); }
}

class NullableCallbackValue {
    private Supplier<Integer> value;
    NullableCallbackValue(Supplier<Integer> value) { this.value = value; }
    int read() {
        try { return value.get(); }
        catch (NullPointerException error) { return -2; }
    }
}

class CallbackSequence<T> {
    private Supplier<Iterable<T>> value;
    CallbackSequence(Supplier<Iterable<T>> value) { this.value = value; }
    Iterator<T> iterator() { return value.get().iterator(); }
}

interface CallbackValueSource { int read(); }

class ArrayCallbackValueSource implements CallbackValueSource {
    private int value;
    ArrayCallbackValueSource(int[] values) { this.value = values[0]; }
    public int read() { return value; }
}

class DelegatedCallbackValue {
    private CallbackValueSource source;
    private Supplier<Integer> extra;
    private int offset;
    DelegatedCallbackValue(CallbackValueSource source, Supplier<Integer> extra) {
        this.source = source;
        this.extra = extra;
    }
    DelegatedCallbackValue(int[] values) { this(new ArrayCallbackValueSource(values), () -> 2); }
    DelegatedCallbackValue(int[] values, int offset) {
        this(new ArrayCallbackValueSource(values), () -> 2);
        this.offset = offset;
    }
    DelegatedCallbackValue(int[] values, int offset, boolean ignored) { this(values, offset); }
    int read() { return source.read() + extra.get() + offset; }
}

class ConstructorCallbackValues {
    static int five() { return 5; }

    static int nestedDelegation() {
        DelegatedCallbackValue first = new DelegatedCallbackValue(new int[] { 4 });
        DelegatedCallbackValue second = new DelegatedCallbackValue(new int[] { 4 }, 3);
        return first.read() * 10 + second.read();
    }

    static int nestedFailure() {
        try { return new DelegatedCallbackValue(new int[0]).read(); }
        catch (ArrayIndexOutOfBoundsException error) { return -3; }
    }

    static int delegatedResult() {
        return new DelegatedCallbackValue(new int[] { 4 }, 3, true).read();
    }

    static int delegated() { return new CallbackValueHolder().read(); }

    static int nullable() {
        NullableCallbackValue present = new NullableCallbackValue(() -> 6);
        NullableCallbackValue missing = new NullableCallbackValue(null);
        return present.read() + missing.read();
    }

    static int sequenceResult() {
        Supplier<Iterable<Integer>> factory = () -> Arrays.asList(4, 5);
        Iterator<Integer> cursor = factory.get().iterator();
        return cursor.next() + cursor.next();
    }

    static List<Integer> identity(List<Integer> values) { return values; }

    static int borrowedResult() {
        List<Integer> values = Arrays.asList(4, 5);
        Iterator<Integer> cursor = identity(values).iterator();
        return cursor.next() + values.size();
    }

    static int storedSequence() {
        List<Integer> values = Arrays.asList(4, 5);
        Iterator<Integer> cursor = values.iterator();
        return cursor.next() + values.size();
    }

    static int lambdas() {
        CallbackValueHolder holder = new CallbackValueHolder(() -> 20, () -> 3);
        return holder.read();
    }

    static int references() {
        CallbackValueHolder holder = new CallbackValueHolder(ConstructorCallbackValues::five, () -> 2);
        return holder.read();
    }

    static int deferredFailure() {
        CallbackValueHolder holder = new CallbackValueHolder(() -> Integer.parseInt("bad"), () -> 0);
        try { return holder.read(); }
        catch (NumberFormatException error) { return -1; }
    }
}
