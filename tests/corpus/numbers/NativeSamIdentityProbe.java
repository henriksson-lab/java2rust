package samidentity;

import java.util.function.DoubleUnaryOperator;

class Offset implements DoubleUnaryOperator {
    final double amount;
    int calls;
    Offset(double amount) { this.amount = amount; }
    public double applyAsDouble(double value) { calls++; return value + amount; }
}

public class NativeSamIdentityProbe {
    static DoubleUnaryOperator make(int amount) { return value -> value + amount; }

    static int run() {
        DoubleUnaryOperator first = make(1);
        DoubleUnaryOperator alias = first;
        DoubleUnaryOperator second = make(2);
        if (first != alias) return 1;
        if (first == second) return 2;
        if (alias.applyAsDouble(4) != 5 || second.applyAsDouble(4) != 6) return 3;

        Offset value = new Offset(7);
        Offset other = new Offset(7);
        DoubleUnaryOperator left = value;
        DoubleUnaryOperator right = value;
        DoubleUnaryOperator distinct = other;
        if (left != right) return 4;
        if (left == distinct) return 5;
        if (left.applyAsDouble(1) != 8 || right.applyAsDouble(2) != 9 || value.calls != 2) return 6;
        if (other.calls != 0) return 7;
        return 0;
    }

    public static void main(String[] args) { System.out.println(run()); }
}
