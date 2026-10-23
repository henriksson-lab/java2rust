class SmallNumber extends java.lang.Number {
    int value;
    int calls;
    SmallNumber(int value) { this.value = value; }
    public int intValue() {
        calls++;
        if (value == 7) throw new IllegalStateException("int conversion");
        return value;
    }
    public byte byteValue(int ignored) { return 99; }
    public long longValue() { return value; }
    public float floatValue() { return value; }
    public double doubleValue() { return value; }
}
class ChangedNumber extends SmallNumber {
    ChangedNumber(int value) { super(value); }
    public int intValue() { calls++; return value + 1; }
}
class ByteOverrideNumber extends SmallNumber {
    ByteOverrideNumber(int value) { super(value); }
    public byte byteValue() { return 42; }
}
abstract class AbstractNumber extends java.lang.Number { }
class ConcreteNumber extends AbstractNumber {
    int calls;
    public int intValue() { calls++; return 65535; }
    public long longValue() { return 65535; }
    public float floatValue() { return 65535; }
    public double doubleValue() { return 65535; }
}
class InheritedNumberConversionsProbe {
    static class ImplicitNumber extends Number {
        int calls;
        public int intValue() { calls++; return 511; }
        public long longValue() { return 511; }
        public float floatValue() { return 511; }
        public double doubleValue() { return 511; }
    }
    static int run() {
        SmallNumber value = new SmallNumber(65535);
        if (value.byteValue() != -1 || value.calls != 1) return 1;
        if (value.shortValue() != -1 || value.calls != 2) return 2;
        value.value = -129;
        if (value.byteValue() != 127 || value.calls != 3) return 3;
        if (value.shortValue() != -129 || value.calls != 4) return 4;
        if (value.byteValue(0) != 99 || value.calls != 4) return 5;
        value.value = 7;
        try { value.byteValue(); return 6; } catch (IllegalStateException expected) { }
        if (value.calls != 5) return 7;
        try { value.shortValue(); return 8; } catch (IllegalStateException expected) { }
        if (value.calls != 6) return 9;
        SmallNumber absent = null;
        try { absent.byteValue(); return 10; } catch (NullPointerException expected) { }
        SmallNumber changed = new ChangedNumber(255);
        if (changed.byteValue() != 0 || changed.calls != 1) return 11;
        if (changed.shortValue() != 256 || changed.calls != 2) return 12;
        SmallNumber overridden = new ByteOverrideNumber(255);
        if (overridden.byteValue() != 42 || overridden.calls != 0) return 13;
        if (overridden.shortValue() != 255 || overridden.calls != 1) return 14;
        ConcreteNumber concrete = new ConcreteNumber();
        if (concrete.byteValue() != -1 || concrete.calls != 1) return 15;
        if (concrete.shortValue() != -1 || concrete.calls != 2) return 16;
        ImplicitNumber implicit = new ImplicitNumber();
        if (implicit.byteValue() != -1 || implicit.calls != 1) return 17;
        if (implicit.shortValue() != 511 || implicit.calls != 2) return 18;
        return 0;
    }
    public static void main(String[] args) { System.out.println(run()); }
}
