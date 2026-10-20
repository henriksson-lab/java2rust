import java.util.Arrays;

class OuterArrayArgument {
    char[] buffer = new char[1];
    class Inner {}
    static void fill(char[] output) { output[0] = 'x'; }
    char first() {
        new Inner();
        fill(buffer);
        return buffer[0];
    }
    static int run() { return new OuterArrayArgument().first() == 'x' ? 0 : 1; }
}

class RetainedArrayCopies {
    byte[] values;
    int calls = 0;
    RetainedArrayCopies(byte[] input) { values = Arrays.copyOf(input, input.length); }
    byte[] snapshot() { return Arrays.copyOf(values, values.length); }
    byte[] range(int from, int to) { return Arrays.copyOfRange(values, from, to); }
    int bound() { calls++; values[0] = 9; return 4; }
    byte[] orderedCopy() { return Arrays.copyOf(values, bound()); }
    static int run() {
        byte[] input = new byte[] { 1, 2, 3 };
        RetainedArrayCopies owner = new RetainedArrayCopies(input);
        input[0] = 7;
        if (owner.values[0] != 1) return 1;
        byte[] copy = owner.snapshot();
        copy[1] = 8;
        if (owner.values[1] != 2) return 2;
        byte[] padded = owner.range(1, 5);
        if (padded.length != 4 || padded[0] != 2 || padded[1] != 3
                || padded[2] != 0 || padded[3] != 0) return 3;
        padded[0] = 99;
        if (owner.values[1] != 2) return 4;
        byte[] ordered = owner.orderedCopy();
        if (owner.calls != 1 || ordered[0] != 9 || ordered.length != 4
                || ordered[3] != 0) return 5;
        try { owner.range(-1, 1); return 6; }
        catch (ArrayIndexOutOfBoundsException error) {}
        try { owner.range(2, 1); return 7; }
        catch (IllegalArgumentException error) {}
        try { byte[] bad = Arrays.copyOf(owner.values, -1); return 8; }
        catch (NegativeArraySizeException error) {}
        return 0;
    }
}

class ConstructorArrayRebinding {
    long[] state;
    ConstructorArrayRebinding(long[] seed, int size) { initialize(seed, size); }
    void initialize(long[] seed, int size) {
        if (seed.length == 0) seed = new long[size];
        state = Arrays.copyOf(seed, seed.length);
    }
    long[] expose() { return state; }
    static int run() {
        long[] seed = new long[0];
        ConstructorArrayRebinding owner = new ConstructorArrayRebinding(seed, 2);
        if (seed.length != 0 || owner.expose().length != 2) return 1;
        long[] data = new long[] {17, -31};
        ConstructorArrayRebinding seeded = new ConstructorArrayRebinding(data, 0);
        data[0] = 99;
        if (seeded.expose()[0] != 17 || seeded.expose()[1] != -31) return 2;
        try { new ConstructorArrayRebinding(seed, -1); return 3; }
        catch (NegativeArraySizeException error) {}
        return 0;
    }
}
