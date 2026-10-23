import java.util.function.DoubleUnaryOperator;

public class EnumStateProbe {
    static int events;

    static DoubleUnaryOperator make(int tag) {
        events = events * 10 + tag;
        return value -> value + tag;
    }

    enum Mode {
        FIRST(EnumStateProbe.make(1)), ALIAS(FIRST), SECOND(EnumStateProbe.make(2));

        final DoubleUnaryOperator operator;
        final int index;
        int calls;

        Mode(DoubleUnaryOperator operator) {
            this.operator = operator;
            index = ordinal();
            events = events * 10 + 3;
        }

        Mode(Mode alias) {
            operator = alias.operator;
            index = ordinal();
            events = events * 10 + 4;
        }

        double apply(double value) {
            calls++;
            return operator.applyAsDouble(value);
        }
    }

    static int run() {
        if (Mode.FIRST.apply(5) != 6 || Mode.ALIAS.apply(8) != 9 || Mode.SECOND.apply(10) != 12) return 1;
        if (events != 13423) return 2;
        if (Mode.FIRST == Mode.ALIAS || Mode.FIRST.operator != Mode.ALIAS.operator) return 3;
        if (Mode.FIRST.operator == Mode.SECOND.operator) return 4;
        if (Mode.FIRST.index != 0 || Mode.ALIAS.index != 1 || Mode.SECOND.index != 2) return 5;
        Mode saved = Mode.FIRST;
        if (saved.apply(6) != 7 || Mode.FIRST.calls != 2 || Mode.ALIAS.calls != 1) return 6;
        if (events != 13423 || !Mode.ALIAS.name().equals("ALIAS") || Mode.values().length != 3) return 7;
        return 0;
    }

    public static void main(String[] args) {
        System.out.println(run());
    }
}
