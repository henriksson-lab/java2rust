import org.apache.commons.numbers.fraction.Fraction;
import org.apache.commons.numbers.core.NativeOperators;

/** Independent caller: this source is compiled by javac, never translated. */
public final class FractionOracle {
    interface Request { String run(); }
    static String units(String text) {
        if (text == null) return "NULL";
        StringBuilder result = new StringBuilder();
        for (int i = 0; i < text.length(); i++) result.append(String.format("%04x", (int) text.charAt(i)));
        return result.toString();
    }
    static void emit(String label, Request request) {
        try { System.out.println(label + "\t" + request.run()); }
        catch (RuntimeException error) {
            System.out.println(label + "\tERR\t" + error.getClass().getSimpleName() + "\t" +
                (error instanceof ArithmeticException) + "\t" + units(error.getMessage()));
        }
    }
    static String view(Fraction value) {
        return value.getNumerator() + "/" + value.getDenominator() + "|" +
            String.format("%016x", Double.doubleToRawLongBits(value.doubleValue())) + "|" +
            String.format("%08x", Float.floatToRawIntBits(value.floatValue())) + "|" +
            value.intValue() + "|" + value.longValue() + "|" + value.byteValue() + "|" + value.shortValue() + "|" + value.signum() + "|" +
            value.isZero() + "|" + value.isOne() + "|" + value.hashCode() + "|" + units(value.toString());
    }
    static String result(Fraction input, Fraction output) {
        return view(output) + "|same=" + (input == output);
    }
    public static void main(String[] args) {
        int[][] pairs = {{0,1},{1,1},{-1,1},{2,3},{-2,3},{2,-3},{-2,-3},
            {Integer.MIN_VALUE,1},{1,Integer.MIN_VALUE},{Integer.MIN_VALUE,Integer.MIN_VALUE},
            {Integer.MAX_VALUE,2},{0,-7},{1,0},{0,0},{Integer.MAX_VALUE,-Integer.MAX_VALUE},
            {Integer.MIN_VALUE,-1},{Integer.MIN_VALUE,-2},{Integer.MIN_VALUE,Integer.MIN_VALUE+1}};
        for (int i = 0; i < pairs.length; i++) {
            final int index = i, n = pairs[i][0], d = pairs[i][1];
            emit("of-pair:" + i, () -> view(Fraction.of(n,d)));
            emit("of-int:" + i, () -> view(Fraction.of(n)));
            Fraction constructed;
            try { constructed = Fraction.of(n,d); } catch (RuntimeException ignored) { continue; }
            final Fraction value = constructed;
            for (int op = 0; op < 6; op++) {
                final int operation = op;
                emit("unary:" + i + ":" + op, () -> {
                    Fraction output;
                    switch (operation) {
                    case 0: output = value.abs(); break;
                    case 1: output = value.negate(); break;
                    case 2: output = value.reciprocal(); break;
                    case 3: output = value.zero(); break;
                    case 4: output = value.one(); break;
                    default: output = value.pow(1);
                    }
                    return result(value, output);
                });
            }
            for (int exponent : new int[]{-3,-1,0,1,2,7,Integer.MIN_VALUE})
                emit("pow:" + i + ":" + exponent, () -> result(value, value.pow(exponent)));
            for (int arg : new int[]{Integer.MIN_VALUE,-2,-1,0,1,2,Integer.MAX_VALUE}) {
                for (int op = 0; op < 4; op++) {
                    final int operation = op;
                    emit("integer:" + i + ":" + arg + ":" + op, () -> {
                        Fraction output;
                        switch (operation) {
                        case 0: output = value.add(arg); break;
                        case 1: output = value.subtract(arg); break;
                        case 2: output = value.multiply(arg); break;
                        default: output = value.divide(arg);
                        }
                        return result(value, output);
                    });
                }
            }
            // Include the same allocation, equal distinct allocation and opposite signs.
            Fraction[] others = {value, Fraction.of(n,d), Fraction.ZERO, Fraction.ONE, Fraction.of(-2,3)};
            for (int j = 0; j < others.length; j++) {
                final Fraction other = others[j];
                for (int op = 0; op < 6; op++) {
                    final int operation = op;
                    emit("binary:" + i + ":" + j + ":" + op, () -> {
                        switch (operation) {
                        case 0: return result(value, value.add(other));
                        case 1: return result(value, value.subtract(other));
                        case 2: return result(value, value.multiply(other));
                        case 3: return result(value, value.divide(other));
                        case 4: return Integer.toString(value.compareTo(other));
                        default: return Boolean.toString(value.equals(other));
                        }
                    });
                }
            }
            emit("equals-null:" + index, () -> Boolean.toString(value.equals(null)));
            emit("equals-other:" + index, () -> Boolean.toString(value.equals("fraction")));
            final NativeOperators<Fraction> operators = value;
            for (int op = 0; op < 12; op++) {
                final int operation = op;
                emit("interface:" + i + ":" + op, () -> {
                    Fraction output;
                    switch (operation) {
                    case 0: output = operators.add(value); break;
                    case 1: output = operators.subtract(value); break;
                    case 2: output = operators.multiply(value); break;
                    case 3: output = operators.divide(value); break;
                    case 4: output = operators.pow(-1); break;
                    case 5: output = operators.multiply(2); break;
                    case 6: output = operators.negate(); break;
                    case 7: output = operators.reciprocal(); break;
                    case 8: output = operators.zero(); break;
                    case 9: output = operators.one(); break;
                    case 10: return Boolean.toString(operators.isZero());
                    default: return Boolean.toString(operators.isOne());
                    }
                    return result(value, output);
                });
            }
            emit("unchanged:" + index, () -> view(value));
        }
        double[] inputs = {-0.0,0.0,0.5,-1.25,Math.PI,1.0/3.0,1e9,Double.NaN,Double.POSITIVE_INFINITY};
        for (int i = 0; i < inputs.length; i++) {
            final double value = inputs[i];
            emit("from:" + i, () -> view(Fraction.from(value)));
            emit("from-epsilon:" + i, () -> view(Fraction.from(value,1e-8,100)));
            emit("from-limit:" + i, () -> view(Fraction.from(value,100)));
            emit("from-iterations:" + i, () -> view(Fraction.from(value,0.0,1)));
        }
        String[] texts = {"0","1","-2 / 3"," 4 / -6 ","2147483647 / 2","1/0","x","1/2/3","", "\u0661 / \uff12"};
        for (int i = 0; i < texts.length; i++) {
            final String text = texts[i];
            emit("parse:" + i, () -> view(Fraction.parse(text)));
        }
        emit("constant-zero", () -> view(Fraction.ZERO));
        emit("constant-one", () -> view(Fraction.ONE));
        emit("invalid-denominator-limit", () -> view(Fraction.from(1.0,0)));
        emit("invalid-epsilon-nan", () -> view(Fraction.from(1.0,Double.NaN,100)));
        emit("invalid-epsilon-negative", () -> view(Fraction.from(1.0,-1.0,100)));
        emit("invalid-iterations", () -> view(Fraction.from(1.0,1e-8,0)));
        // Upstream regression workloads for continued-fraction overflow/convergence.
        double[] difficult={0.75000000001455192,1e10,-1e10,-43979.60679604749,(1.0+Math.sqrt(5.0))/2.0};
        for (int i=0;i<difficult.length;i++) {
            final double value=difficult[i];
            emit("conversion-overflow:"+i, () -> view(Fraction.from(value,1e-12,25)));
            emit("denominator-overflow:"+i, () -> view(Fraction.from(value,1000)));
        }
    }
}
