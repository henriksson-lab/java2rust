import java.math.BigInteger;
import org.apache.commons.numbers.fraction.Fraction;
import org.apache.commons.numbers.fraction.BigFraction;
import org.apache.commons.numbers.fraction.GeneralizedContinuedFraction;
import org.apache.commons.numbers.fraction.GeneralizedContinuedFraction.Coefficient;
import java.util.function.Supplier;

/** Independent direct calls: the translated null-boundary fixture is not invoked. */
public final class FractionNullExceptionOracle {
    interface Action { void run(); }
    static void emit(int request, Action action) {
        try { action.run(); System.out.println(request + "\tOK"); }
        catch (RuntimeException error) {
            String message = error.getMessage();
            StringBuilder units = new StringBuilder();
            if (message == null) units.append("NULL");
            else for (int i = 0; i < message.length(); i++) units.append(String.format("%04x", (int)message.charAt(i)));
            System.out.println(request + "\t" + error.getClass().getName() + "\t" + (error instanceof NullPointerException) + "\t" + units);
        }
    }
    public static void main(String[] args) {
        Fraction fraction = Fraction.of(2, 3);
        BigFraction big = BigFraction.of(2, 3);
        emit(0, () -> Fraction.parse(null));
        emit(1, () -> BigFraction.parse(null));
        emit(2, () -> BigFraction.of((BigInteger)null));
        emit(3, () -> big.add((BigFraction)null));
        emit(4, () -> fraction.compareTo(null));
        emit(5, () -> big.compareTo(null));
        emit(6, () -> big.add((BigInteger)null));
        emit(7, () -> BigFraction.of(BigInteger.ONE, (BigInteger)null));
        emit(8, () -> BigFraction.of((BigInteger)null, BigInteger.ONE));
        emit(9, () -> big.subtract((BigInteger)null));
        emit(10, () -> big.multiply((BigInteger)null));
        emit(11, () -> big.divide((BigInteger)null));
        emit(12, () -> big.subtract((BigFraction)null));
        emit(13, () -> big.multiply((BigFraction)null));
        emit(14, () -> big.divide((BigFraction)null));
        emit(15, () -> fraction.add((Fraction)null));
        emit(16, () -> fraction.subtract((Fraction)null));
        emit(17, () -> fraction.multiply((Fraction)null));
        emit(18, () -> fraction.divide((Fraction)null));
        emit(19, () -> big.bigDecimalValue((java.math.RoundingMode)null));
        emit(20, () -> big.bigDecimalValue(3, (java.math.RoundingMode)null));
        emit(21, () -> GeneralizedContinuedFraction.value((Supplier<Coefficient>)null));
        emit(22, () -> GeneralizedContinuedFraction.value(1.0,(Supplier<Coefficient>)null));
    }
}
