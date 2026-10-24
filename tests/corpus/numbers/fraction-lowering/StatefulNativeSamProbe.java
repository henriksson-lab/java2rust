package statefulnativesam;

import java.util.function.IntSupplier;
import java.util.function.IntBinaryOperator;
import java.util.function.Supplier;

public class StatefulNativeSamProbe {
    static final class Term {
        final double value;
        Term(double value) { this.value = value; }
    }
    static IntSupplier counter() {
        return new IntSupplier() {
            private int count;
            public int getAsInt() { count++; return count; }
        };
    }
    static Supplier<Term> terms(final double offset) {
        return new Supplier<Term>() {
            private int count;
            public Term get() {
                count++;
                if (count == 3) throw new IllegalStateException("third");
                return new Term(count + offset);
            }
        };
    }
    static IntBinaryOperator combine() {
        return new IntBinaryOperator() {
            private int calls;
            public int applyAsInt(int left, int right) {
                calls++;
                return left * 10 + right + calls;
            }
        };
    }
    public static int run() {
        IntSupplier first = counter();
        IntSupplier alias = first;
        IntSupplier independent = counter();
        if (first != alias || first == independent) return 1;
        if (first.getAsInt() != 1 || alias.getAsInt() != 2 || independent.getAsInt() != 1) return 2;
        Supplier<Term> source = terms(0.5);
        Supplier<Term> other = terms(-0.5);
        Supplier<Term> same = source;
        Term retained = source.get();
        Term second = same.get();
        if (retained.value != 1.5 || second.value != 2.5 || retained == second) return 3;
        try { source.get(); return 4; }
        catch (IllegalStateException error) { if (!"third".equals(error.getMessage())) return 5; }
        if (same.get().value != 4.5 || retained.value != 1.5 || other.get().value != 0.5) return 6;
        if (source != same || source == other) return 7;
        IntBinaryOperator binary = combine();
        IntBinaryOperator binaryAlias = binary;
        if (binary.applyAsInt(2, 3) != 24 || binaryAlias.applyAsInt(3, 4) != 36) return 8;
        if (combine().applyAsInt(-2, 3) != -16) return 9;
        return 0;
    }
    public static void main(String[] args) {
        int result = run();
        if (result != 0) throw new AssertionError("native SAM state: " + result);
        System.out.println(result);
    }
}
