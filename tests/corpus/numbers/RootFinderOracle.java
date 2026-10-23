package org.apache.commons.numbers.rootfinder;

import java.util.ArrayList;
import java.util.List;
import java.util.function.DoubleUnaryOperator;

/** Caller-only oracle: all solver algorithms and validation are upstream. */
public final class RootFinderOracle {
    static String bits(double x) { return Long.toHexString(Double.doubleToRawLongBits(x)); }

    static void run(String label, int function, double min, double initial, double max,
                    boolean explicit, int failAt) {
        runWithAccuracy(label, function, min, initial, max, explicit, failAt, 1e-14, 1e-14, 0.0);
    }

    static void runWithAccuracy(String label, int function, double min, double initial, double max,
                                boolean explicit, int failAt, double relative, double absolute,
                                double valueAccuracy) {
        BrentSolver solver = new BrentSolver(relative, absolute, valueAccuracy);
        List<String> trace = new ArrayList<>();
        DoubleUnaryOperator callback = x -> {
            trace.add(bits(x));
            if (trace.size() == failAt || trace.size() > 1024)
                throw new IllegalStateException("callback-" + trace.size());
            switch (function) {
            case 0: return x * x - 2.0;
            case 1: return 1e-200 * (x - 0.1);
            case 2: return -1e-200 * (x - 0.1);
            case 3: return Math.cos(x) - x;
            case 4: return (x - 0.25) * (x - 0.25) * (x - 0.25);
            case 5: return x;
            case 6: return x * x + 1.0;
            case 7: return Double.NaN;
            case 8: return x - 1.0;
            default: throw new AssertionError("unknown callback");
            }
        };
        try {
            double root = explicit ? solver.findRoot(callback, min, initial, max)
                                   : solver.findRoot(callback, min, max);
            System.out.println(label + ".root.bits=" + bits(root));
        } catch (RuntimeException error) {
            System.out.println(label + ".error=" + error.getClass().getSimpleName() + ":" + error.getMessage());
        }
        System.out.println(label + ".calls=" + trace.size());
        System.out.println(label + ".trace=" + String.join(",", trace));
    }

    public static void main(String[] ignored) {
        run("sqrt.default", 0, 0, 0, 2, false, 0);
        for (double initial : new double[] {0, 0.25, 0.9, 1.9, 2})
            run("sqrt.initial." + bits(initial), 0, 0, initial, 2, true, 0);
        run("tiny.positive", 1, 0, 0, 1, false, 0);
        run("tiny.negative", 2, 0, 0, 1, false, 0);
        run("cosine", 3, 0, 0, 1, false, 0);
        run("cubic", 4, -1, 0.9, 2, true, 0);
        run("endpoint.low", 8, 1, 1.5, 2, true, 0);
        run("endpoint.high", 8, 0, 0.25, 1, true, 0);
        run("initial.root", 8, 0, 1, 2, true, 0);
        run("zero.negative", 5, -0.0, 0, 0.0, false, 0);
        run("zero.positive", 5, 0.0, 0, 0.0, false, 0);
        run("huge", 5, -Double.MAX_VALUE, 0, Double.MAX_VALUE, false, 0);
        run("subnormal.root", 5, 0, 0, Double.MIN_VALUE, false, 0);
        run("subnormal.singleton", 5, Double.MIN_VALUE, 0, Double.MIN_VALUE, false, 0);
        run("invalid.interval", 0, 2, 0, 1, false, 0);
        run("invalid.initial.low", 0, 0, -1, 2, true, 0);
        run("invalid.initial.high", 0, 0, 3, 2, true, 0);
        run("invalid.bracket", 6, -2, 0, 2, false, 0);
        run("nan.function", 7, -1, 0, 1, false, 0);
        run("nan.bound", 5, Double.NaN, 0, 1, false, 0);
        run("infinite.bound", 5, -1, 0, Double.POSITIVE_INFINITY, false, 0);
        for (int call : new int[] {1, 2, 3, 5})
            run("throw.default." + call, 0, 0, 0, 2, false, call);
        for (int call : new int[] {1, 2, 3, 5})
            run("throw.explicit." + call, 0, 0, 0.25, 2, true, call);
        runWithAccuracy("accuracy.absolute", 0, 0, 0, 2, false, 0, 1e-12, 1e-2, 0);
        runWithAccuracy("accuracy.relative", 0, 0, 0, 2, false, 0, 1e-2, 1e-12, 0);
        runWithAccuracy("accuracy.zero.relative", 0, 0, 0, 2, false, 0, 0, 1e-12, 0);
        runWithAccuracy("accuracy.function.initial", 8, 0, 0.9, 2, true, 0, 1e-14, 1e-14, 0.2);
        runWithAccuracy("accuracy.function.low", 8, 0.9, 1.8, 2, true, 0, 1e-14, 1e-14, 0.2);
        runWithAccuracy("accuracy.function.high", 8, -2, 0, 1.1, true, 0, 1e-14, 1e-14, 0.2);
        BrentSolver solver = new BrentSolver(1e-14, 1e-14, 0.0);
        final int[] calls = {0};
        DoubleUnaryOperator callback = x -> { calls[0]++; return x - 0.75; };
        DoubleUnaryOperator alias = callback;
        System.out.println("repeat.first.bits=" + bits(solver.findRoot(callback, 0, 1)));
        System.out.println("repeat.first.calls=" + calls[0]);
        System.out.println("repeat.alias.bits=" + bits(solver.findRoot(alias, 0, 0.9, 1)));
        System.out.println("repeat.alias.calls=" + calls[0]);
        System.out.println("oracle=ok");
    }
}
