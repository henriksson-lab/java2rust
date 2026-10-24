package statefuloutersupplier;

import java.util.function.Supplier;

/** Generic control for the production ContinuedFraction supplier contract. */
public class StatefulOuterSupplierProbe {
    static final class Term {
        final double a;
        final double b;
        Term(double a, double b) { this.a = a; this.b = b; }
    }
    static abstract class Series {
        abstract double getA(int n, double x);
        abstract double getB(int n, double x);
        Supplier<Term> generator(final double x) {
            return new Supplier<Term>() {
                private int n;
                public Term get() {
                    n++;
                    final double a = getA(n, x);
                    final double b = getB(n, x);
                    return new Term(a, b);
                }
            };
        }
    }
    static abstract class Middle extends Series { }
    static final class Concrete extends Middle {
        int factor;
        int callsA;
        int callsB;
        long events;
        Concrete(int factor) { this.factor = factor; }
        double getA(int n, double x) {
            callsA++;
            events = events * 10 + 1;
            if (n == 3) throw new IllegalStateException("stop");
            return factor * n + x;
        }
        double getB(int n, double x) {
            callsB++;
            events = events * 10 + 2;
            return 10 * n + x;
        }
    }
    static final class Alternative extends Series {
        double getA(int n, double x) { return 100 + n + x; }
        double getB(int n, double x) { return -n; }
    }
    static Supplier<Term> generate(Series owner, double x) {
        return owner.generator(x);
    }
    public static int run() {
        Concrete owner = new Concrete(3);
        Supplier<Term> first = generate(owner, 2);
        Supplier<Term> alias = first;
        if (first != alias || owner.callsA != 0 || owner.callsB != 0) return 1;
        Term one = first.get();
        if (one.a != 5 || one.b != 12) return 2;
        owner.factor = 4;
        Term two = alias.get();
        if (two.a != 10 || two.b != 22 || one.a != 5) return 3;
        Supplier<Term> independent = owner.generator(-1);
        Term separate = independent.get();
        if (separate.a != 3 || separate.b != 9) return 4;
        try { first.get(); return 5; }
        catch (IllegalStateException expected) {
            if (!"stop".equals(expected.getMessage())) return 6;
        }
        Term after = first.get();
        if (after.a != 18 || after.b != 42) return 7;
        if (owner.callsA != 5 || owner.callsB != 4 || owner.events != 121212112L) return 8;
        if (first != alias || first == independent || one == two) return 9;
        Supplier<Term> alternate = generate(new Alternative(), 1);
        Term altOne = alternate.get();
        Term altTwo = alternate.get();
        if (altOne.a != 102 || altOne.b != -1 || altTwo.a != 103 || altTwo.b != -2) return 10;
        if (altOne.a != 102 || altOne == altTwo || alternate == first) return 11;
        return 0;
    }
    public static void main(String[] args) {
        int result = run();
        if (result != 0) throw new AssertionError("supplier contract: " + result);
        System.out.println(result);
    }
}
