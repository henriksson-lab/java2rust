import org.apache.commons.numbers.core.ArithmeticUtils;
import org.apache.commons.numbers.fraction.Fraction;
import org.apache.commons.numbers.fraction.BigFraction;
import org.apache.commons.numbers.fraction.GeneralizedContinuedFraction;
import org.apache.commons.numbers.fraction.GeneralizedContinuedFraction.Coefficient;

/** Independent observation of complete exception class identity, not its leaf alias. */
public final class FractionExceptionIdentityOracle {
    interface Action { void run(); }
    static void emit(int request,Action action) {
        try { action.run(); System.out.println(request+"\tOK"); }
        catch (RuntimeException error) {
            String message=error.getMessage();
            StringBuilder units=new StringBuilder();
            if (message==null) units.append("NULL");
            else for (int i=0;i<message.length();i++) units.append(String.format("%04x",(int)message.charAt(i)));
            System.out.println(request+"\t"+error.getClass().getName()+"\t"+(error instanceof ArithmeticException)+"\t"+units);
        }
    }
    public static void main(String[] args) {
        emit(0,() -> Fraction.of(1,0));
        emit(1,() -> Fraction.from(Double.NaN));
        emit(2,() -> BigFraction.parse("x"));
        emit(3,() -> ArithmeticUtils.gcd(Integer.MIN_VALUE,0));
        emit(4,() -> GeneralizedContinuedFraction.value(() -> Coefficient.of(Double.NaN,1.0),1e-12,2));
    }
}
