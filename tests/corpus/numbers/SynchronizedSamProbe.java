package samsync;

import java.util.function.DoubleUnaryOperator;

class Counter {
    double value;
    double add(double amount) { value += amount; return value; }
}

public class SynchronizedSamProbe {
    static final DoubleUnaryOperator ARRAY = makeArray();
    static final DoubleUnaryOperator OBJECT = makeObject();
    static final DoubleUnaryOperator RECURSIVE = makeRecursive();
    static final DoubleUnaryOperator CHECKED = makeChecked();

    static DoubleUnaryOperator makeArray() {
        double[] values = new double[]{1};
        return amount -> { values[0] += amount; return values[0]; };
    }
    static DoubleUnaryOperator makeObject() {
        Counter counter = new Counter();
        return amount -> counter.add(amount);
    }
    static DoubleUnaryOperator makeRecursive() {
        return depth -> depth <= 0 ? 1 : RECURSIVE.applyAsDouble(depth - 1) + 1;
    }
    static DoubleUnaryOperator makeChecked() {
        return value -> {
            if (value < 0) throw new IllegalArgumentException("negative callback");
            return value + 1;
        };
    }

    static int run() {
        DoubleUnaryOperator alias = ARRAY;
        if (alias != ARRAY || alias == OBJECT) return 1;
        if (ARRAY.applyAsDouble(2) != 3 || alias.applyAsDouble(5) != 8) return 2;
        if (OBJECT.applyAsDouble(3) != 3 || OBJECT.applyAsDouble(4) != 7) return 3;
        if (RECURSIVE.applyAsDouble(4) != 5) return 4;
        try { CHECKED.applyAsDouble(-1); return 5; }
        catch (IllegalArgumentException error) {
            if (!error.getMessage().equals("negative callback")) return 6;
        }
        if (CHECKED.applyAsDouble(2) != 3) return 7;
        return 0;
    }
    public static void main(String[] args) { System.out.println(run()); }
}
