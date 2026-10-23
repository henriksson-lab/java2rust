import java.util.function.DoubleFunction;

class NullableEqualityValue {
    final int number;
    NullableEqualityValue(int number) { this.number = number; }
    public boolean equals(Object other) {
        return other instanceof NullableEqualityValue
            && ((NullableEqualityValue) other).number == number;
    }
}

class NullableEqualitySampler implements DoubleFunction<NullableEqualityValue> {
    private final NullableEqualityValue value;
    NullableEqualitySampler(NullableEqualityValue value) { this.value = value; }
    public NullableEqualityValue apply(double t) { return value; }
}

class NullableEqualityHolder {
    final Object value;
    NullableEqualityHolder(Object value) { this.value = value; }
    boolean missing() { return value == null; }
}

class NullableSourceEqualityProbe {
    static boolean same(Object left, Object right) { return left.equals(right); }
    static int run() {
        NullableEqualitySampler present = new NullableEqualitySampler(new NullableEqualityValue(7));
        NullableEqualitySampler absent = new NullableEqualitySampler(null);
        DoubleFunction<NullableEqualityValue> callback = present;
        if (!present.apply(0.5).equals(present.apply(0.5))) return 1;
        if (present.apply(0.5).equals(absent.apply(0.5))) return 2;
        if (!callback.apply(0.5).equals(present.apply(0.5))) return 3;
        if (!present.apply(0.5).equals(callback.apply(0.5))) return 4;
        if (!same(present.apply(0.5), callback.apply(0.5))) return 5;
        if (same(present.apply(0.5), absent.apply(0.5))) return 6;
        Object copied = present.apply(0.5);
        Object missing = absent.apply(0.5);
        if (!(copied instanceof NullableEqualityValue) || missing != null) return 7;
        if (new NullableEqualityHolder(present.apply(0.5)).missing()
            || !new NullableEqualityHolder(absent.apply(0.5)).missing()) return 8;
        return 0;
    }
    public static void main(String[] args) { System.out.println(run()); }
}
