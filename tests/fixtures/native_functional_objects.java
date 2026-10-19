import java.util.Comparator;
import java.util.function.BiFunction;
import java.util.function.IntSupplier;

class NamedCounter implements IntSupplier {
    private int value;
    NamedCounter(int start) { value = start; }
    public int getAsInt() { int result = value; value++; return result; }
}

class ChildCounter extends NamedCounter {
    ChildCounter(int start) { super(start); }
}

class NamedOrder implements Comparator<Integer> {
    public int compare(Integer first, Integer second) {
        return first < second ? -1 : first > second ? 1 : 0;
    }
}

class ThrowingOrder implements Comparator<Integer> {
    public int compare(Integer first, Integer second) {
        if (first < 0) { throw new IllegalArgumentException(); }
        return first < second ? -1 : first > second ? 1 : 0;
    }
}

class NamedJoiner implements BiFunction<String, String, String> {
    public String apply(String first, String second) { return first + second; }
}

class NativeFunctionalObjects {
    private Comparator<Integer> storedOrder = new NamedOrder();
    static IntSupplier counter(int start) { return new NamedCounter(start); }
    static IntSupplier inherited(int start) { return new ChildCounter(start); }
    static Comparator<Integer> order() { return new NamedOrder(); }
    static BiFunction<String, String, String> joiner() { return new NamedJoiner(); }
    static IntSupplier maybe(boolean present) { return present ? new NamedCounter(5) : null; }
    static IntSupplier nullableSource(NamedCounter source) { return source; }
    static int sampled() {
        IntSupplier first = counter(3);
        IntSupplier alias = first;
        return first.getAsInt() * 100 + alias.getAsInt() * 10 + first.getAsInt();
    }
    static int sampledInherited() { return inherited(8).getAsInt(); }
    static int compared() { return order().compare(2, 1); }
    static int fieldConversion() {
        NativeFunctionalObjects holder = new NativeFunctionalObjects();
        return holder.storedOrder.compare(2, 1);
    }
    static int replacedField() {
        NativeFunctionalObjects holder = new NativeFunctionalObjects();
        holder.storedOrder = new ThrowingOrder();
        try { holder.storedOrder.compare(-1, 2); return 0; }
        catch (IllegalArgumentException error) { return 9; }
    }
    static void fieldFailure() {
        NativeFunctionalObjects holder = new NativeFunctionalObjects();
        holder.storedOrder = new ThrowingOrder();
        holder.storedOrder.compare(-1, 2);
    }
    static int throughField() {
        try { fieldFailure(); return 0; }
        catch (IllegalArgumentException error) { return 9; }
    }
    static String joined() { return joiner().apply("first", "second"); }
    static int nullable(boolean present) {
        IntSupplier result = maybe(present);
        return result == null ? -1 : result.getAsInt();
    }
    static int nullSource() {
        IntSupplier result = nullableSource(null);
        return result == null ? 1 : 0;
    }
    static int sharedObject() {
        NamedCounter source = new NamedCounter(7);
        IntSupplier adapted = source;
        int first = adapted.getAsInt();
        return first * 10 + source.getAsInt();
    }
    static int caught() {
        Comparator<Integer> throwing = new ThrowingOrder();
        try { throwing.compare(-1, 2); return 0; }
        catch (IllegalArgumentException error) { return 9; }
    }
}
