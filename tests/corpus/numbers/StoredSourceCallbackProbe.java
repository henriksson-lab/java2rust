package storedsourcecallback;

public class StoredSourceCallbackProbe {
    private interface StoredOperation { int apply(int value); }
    static class Store {
        final StoredOperation body;
        Store(StoredOperation body) { this.body = body; }
        int call(int value) { return body.apply(value); }
    }
    static final class Delegated extends Store {
        Delegated() { this(StoredSourceCallbackProbe::checked); }
        Delegated(StoredOperation body) { super(body); }
    }
    static int invocations;
    static int checked(int value) {
        invocations++;
        if (value < 0) throw new IllegalArgumentException("negative");
        return value + 1;
    }
    public static int run() {
        invocations = 0;
        Store direct = new Store(StoredSourceCallbackProbe::checked);
        Store delegated = new Delegated();
        Store lambda = new Store(value -> {
            invocations++;
            if (value == 0) throw new IllegalStateException("zero");
            return value * 2;
        });
        if (invocations != 0) return 1;
        if (direct.call(4) != 5 || delegated.call(7) != 8 || lambda.call(3) != 6) return 2;
        if (invocations != 3) return 3;
        try { direct.call(-1); return 4; } catch (IllegalArgumentException expected) { }
        try { delegated.call(-1); return 5; } catch (IllegalArgumentException expected) { }
        try { lambda.call(0); return 6; } catch (IllegalStateException expected) { }
        if (invocations != 6 || direct.call(1) != 2 || delegated.call(2) != 3 || lambda.call(4) != 8) return 7;
        if (invocations != 9) return 8;
        return 0;
    }
    public static void main(String[] args) { System.out.println(run()); }
}
