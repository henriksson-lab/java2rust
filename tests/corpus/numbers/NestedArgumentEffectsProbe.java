package nestedeffects;

public class NestedArgumentEffectsProbe {
    static int trace;
    static int constructions;
    static int mark(int x) { trace = trace * 10 + x; return x; }
    static Sample make(int value) { constructions++; return new Sample(value); }
    static final class Sample {
        final int value;
        Sample(int value) { this.value = value; }
        public boolean equals(Object other) {
            return other instanceof Sample && value == ((Sample)other).value;
        }
    }
    public static int run() {
        Sample expected = make(2);
        trace = 0;
        if (!expected.equals(make((mark(7) + 1) % mark(3)))) return 1;
        if (trace != 73 || constructions != 2) return 2;
        trace = 0;
        try { expected.equals(make(mark(7) % mark(0))); return 3; }
        catch (ArithmeticException caught) { }
        if (trace != 70 || constructions != 2) return 4;
        if (!expected.equals(make(8 % 3))) return 5;
        return 0;
    }
    public static void main(String[] args) { System.out.println(run()); }
}
