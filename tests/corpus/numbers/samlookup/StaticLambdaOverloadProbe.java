package samlookup;

import java.util.function.ToDoubleFunction;
import java.util.function.BiPredicate;

public class StaticLambdaOverloadProbe {
    static class Value {
        static final Value ONE = new Value(1);
        static final Value TWO = new Value(2);
        final double number;
        Value(double number) { this.number=number; }
    }
    private static class Default {
        static final ToDoubleFunction<Value> NORM = value -> value.number;
        static final BiPredicate<Value, Double> CHECK = (value, epsilon) ->
            Overloads.same(NORM.applyAsDouble(value), 1d, epsilon);
    }
    private static class Normalized {
        static final ToDoubleFunction<Value> NORM = value -> 1;
    }
    public static int run() {
        if (!Default.CHECK.test(Value.ONE,0.0)) return 1;
        if (Default.CHECK.test(Value.TWO,0.0)) return 2;
        if (Normalized.NORM.applyAsDouble(Value.ONE) != 1) return 3;
        return 0;
    }
    public static void main(String[] args) { System.out.println(run()); }
}
