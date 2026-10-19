import java.util.function.IntSupplier;

class LocalCounter implements IntSupplier {
    private int next = 3;
    public int getAsInt() { int result = next; next++; return result; }
}

class NativeFunctionalAlias {
    static int sharedCounter() {
        LocalCounter source = new LocalCounter();
        IntSupplier first = source;
        IntSupplier second = source;
        int a = first.getAsInt();
        int b = second.getAsInt();
        return a * 100 + b * 10 + source.getAsInt();
    }
}
