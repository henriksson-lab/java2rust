import java.math.BigInteger;

interface CanonicalProduct<T> { T multiply(T value); }

public final class CanonicalMappedFieldsProbe implements CanonicalProduct<CanonicalMappedFieldsProbe> {
    static final CanonicalMappedFieldsProbe ZERO = new CanonicalMappedFieldsProbe(BigInteger.ZERO, BigInteger.ONE);
    final BigInteger numerator;
    final BigInteger denominator;
    CanonicalMappedFieldsProbe(BigInteger numerator, BigInteger denominator) {
        this.numerator = numerator;
        this.denominator = denominator;
    }
    public CanonicalMappedFieldsProbe multiply(CanonicalMappedFieldsProbe value) {
        return new CanonicalMappedFieldsProbe(numerator.multiply(value.numerator), denominator.multiply(value.denominator));
    }
    CanonicalMappedFieldsProbe multiply(BigInteger value) {
        return new CanonicalMappedFieldsProbe(value.multiply(numerator), denominator);
    }
    CanonicalMappedFieldsProbe multiply(int value) { return multiply(BigInteger.valueOf(value)); }
    CanonicalMappedFieldsProbe identity() { return this; }
}
