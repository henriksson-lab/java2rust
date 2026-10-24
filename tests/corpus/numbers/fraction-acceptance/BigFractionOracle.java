import java.math.BigDecimal;
import java.math.BigInteger;
import java.math.RoundingMode;
import org.apache.commons.numbers.fraction.BigFraction;
import org.apache.commons.numbers.core.NativeOperators;

/** Independent javac caller of the untouched production BigFraction. */
public final class BigFractionOracle {
    interface Request { String run(); }
    static String units(String text) {
        if (text == null) return "NULL";
        StringBuilder result = new StringBuilder();
        for (int i=0; i<text.length(); i++) result.append(String.format("%04x", (int)text.charAt(i)));
        return result.toString();
    }
    static void emit(String label, Request request) {
        try { System.out.println(label + "\t" + request.run()); }
        catch (RuntimeException error) {
            System.out.println(label + "\tERR\t" + error.getClass().getSimpleName() + "\t" +
                (error instanceof ArithmeticException) + "\t" + units(error.getMessage()));
        }
    }
    static String view(BigFraction value) {
        return value.getNumerator() + "/" + value.getDenominator() + "|" +
            String.format("%016x", Double.doubleToRawLongBits(value.doubleValue())) + "|" +
            String.format("%08x", Float.floatToRawIntBits(value.floatValue())) + "|" +
            value.intValue() + "|" + value.longValue() + "|" + value.byteValue() + "|" + value.shortValue() + "|" +
            value.signum() + "|" + value.isZero() + "|" + value.isOne() + "|" + value.hashCode() + "|" +
            value.getNumeratorAsInt() + "|" + value.getNumeratorAsLong() + "|" +
            value.getDenominatorAsInt() + "|" + value.getDenominatorAsLong() + "|" + units(value.toString());
    }
    static String result(BigFraction input, BigFraction output) {
        return view(output) + "|same=" + (input == output);
    }
    static String decimal(BigDecimal value) { return value.toString() + "|" + value.scale(); }
    public static void main(String[] args) {
        String[][] pairs = {{"0","1"},{"1","1"},{"-1","1"},{"2","3"},{"-2","3"},{"2","-3"},
            {"9223372036854775808","-7"},{"123456789012345678901234567890","998877665544332211"},
            {"1","10000000000000000000000000000000000000000000000000000000000"},{"1","0"}};
        for (int i=0; i<pairs.length; i++) {
            final BigInteger n = new BigInteger(pairs[i][0]), d = new BigInteger(pairs[i][1]);
            emit("of-big-pair:"+i, () -> view(BigFraction.of(n,d)));
            emit("of-big:"+i, () -> view(BigFraction.of(n)));
            emit("of-int:"+i, () -> view(BigFraction.of(n.intValue())));
            emit("of-long:"+i, () -> view(BigFraction.of(n.longValue())));
            emit("of-int-pair:"+i, () -> view(BigFraction.of(n.intValue(),d.intValue())));
            emit("of-long-pair:"+i, () -> view(BigFraction.of(n.longValue(),d.longValue())));
            BigFraction constructed;
            try { constructed=BigFraction.of(n,d); } catch (RuntimeException ignored) { continue; }
            final BigFraction value=constructed;
            emit("retained-integer:"+i, () -> Boolean.toString(BigFraction.of(n).getNumerator()==n));
            for (int op=0; op<5; op++) {
                final int operation=op;
                emit("unary:"+i+":"+op, () -> result(value, operation==0 ? value.abs() : operation==1 ? value.negate() :
                    operation==2 ? value.reciprocal() : operation==3 ? value.zero() : value.one()));
            }
            for (int exponent : new int[]{-3,-1,0,1,2,7})
                emit("pow:"+i+":"+exponent, () -> result(value,value.pow(exponent)));
            for (int arg : new int[]{-2,0,1,3}) {
                for (int kind=0; kind<4; kind++) {
                    final int argumentKind=kind;
                    final BigFraction other=arg==1 ? value : BigFraction.of(arg,2);
                    for (int op=0; op<4; op++) {
                        final int operation=op;
                        emit("arithmetic:"+i+":"+arg+":"+kind+":"+op, () -> {
                            BigFraction output;
                            if (argumentKind==0) output=operation==0 ? value.add(arg) : operation==1 ? value.subtract(arg) : operation==2 ? value.multiply(arg) : value.divide(arg);
                            else if (argumentKind==1) output=operation==0 ? value.add((long)arg) : operation==1 ? value.subtract((long)arg) : operation==2 ? value.multiply((long)arg) : value.divide((long)arg);
                            else if (argumentKind==2) output=operation==0 ? value.add(BigInteger.valueOf(arg)) : operation==1 ? value.subtract(BigInteger.valueOf(arg)) : operation==2 ? value.multiply(BigInteger.valueOf(arg)) : value.divide(BigInteger.valueOf(arg));
                            else output=operation==0 ? value.add(other) : operation==1 ? value.subtract(other) : operation==2 ? value.multiply(other) : value.divide(other);
                            return result(value,output);
                        });
                    }
                }
            }
            BigFraction[] others={value,BigFraction.of(n,d),BigFraction.ZERO,BigFraction.ONE,BigFraction.of(-2,3)};
            for (int j=0;j<others.length;j++) {
                final BigFraction other=others[j];
                emit("compare:"+i+":"+j, () -> Integer.toString(value.compareTo(other)));
                emit("equals:"+i+":"+j, () -> Boolean.toString(value.equals(other)));
            }
            emit("equals-null:"+i, () -> Boolean.toString(value.equals(null)));
            emit("equals-other:"+i, () -> Boolean.toString(value.equals("fraction")));
            emit("decimal-exact:"+i, () -> decimal(value.bigDecimalValue()));
            for (RoundingMode mode : RoundingMode.values()) {
                emit("decimal-round:"+i+":"+mode.ordinal(), () -> decimal(value.bigDecimalValue(mode)));
                for (int scale : new int[]{-2,0,3})
                    emit("decimal-scale:"+i+":"+mode.ordinal()+":"+scale, () -> decimal(value.bigDecimalValue(scale,mode)));
            }
            final NativeOperators<BigFraction> operators=value;
            for (int op=0;op<12;op++) {
                final int operation=op;
                emit("interface:"+i+":"+op, () -> {
                    BigFraction output;
                    switch (operation) {
                    case 0: output=operators.add(value); break;
                    case 1: output=operators.subtract(value); break;
                    case 2: output=operators.multiply(value); break;
                    case 3: output=operators.divide(value); break;
                    case 4: output=operators.pow(-1); break;
                    case 5: output=operators.multiply(2); break;
                    case 6: output=operators.negate(); break;
                    case 7: output=operators.reciprocal(); break;
                    case 8: output=operators.zero(); break;
                    case 9: output=operators.one(); break;
                    case 10: return Boolean.toString(operators.isZero());
                    default: return Boolean.toString(operators.isOne());
                    }
                    return result(value,output);
                });
            }
            emit("unchanged:"+i, () -> view(value));
        }
        double[] inputs={-0.0,0.0,0.5,-1.25,Math.PI,1.0/3.0,1e20,Double.MIN_NORMAL,Double.MIN_VALUE,Double.NaN,Double.POSITIVE_INFINITY};
        for (int i=0;i<inputs.length;i++) {
            final double value=inputs[i];
            emit("from:"+i, () -> view(BigFraction.from(value)));
            emit("from-epsilon:"+i, () -> view(BigFraction.from(value,1e-8,100)));
            emit("from-limit:"+i, () -> view(BigFraction.from(value,100)));
        }
        String[] texts={"0","1","-2 / 3"," 4 / -6 ","123456789012345678901234567890 / 7","1/0","x","1/2/3","", "\u0661 / \uff12"};
        for (int i=0;i<texts.length;i++) {
            final String text=texts[i];
            emit("parse:"+i, () -> view(BigFraction.parse(text)));
        }
        emit("constant-zero", () -> view(BigFraction.ZERO));
        emit("constant-one", () -> view(BigFraction.ONE));
        emit("invalid-denominator-limit", () -> view(BigFraction.from(1.0,0)));
        emit("invalid-epsilon-nan", () -> view(BigFraction.from(1.0,Double.NaN,100)));
        emit("invalid-epsilon-negative", () -> view(BigFraction.from(1.0,-1.0,100)));
        emit("invalid-iterations", () -> view(BigFraction.from(1.0,1e-8,0)));
        for (int i=0;i<3;i++) {
            final BigFraction value=i==0 ? BigFraction.ZERO : i==1 ? BigFraction.ONE : BigFraction.of(-1);
            emit("minimum-exponent:"+i, () -> result(value,value.pow(Integer.MIN_VALUE)));
        }
    }
}
