package mappednumericfields;

import java.math.BigInteger;

interface Product<T> { T multiply(T value); }

public class MappedNumericFieldsProbe {
    static final class Pair implements Product<Pair> {
        static final Pair ZERO = new Pair(BigInteger.ZERO, BigInteger.ONE);
        private final BigInteger numerator;
        private final BigInteger denominator;

        Pair(BigInteger numerator, BigInteger denominator) {
            this.numerator = numerator;
            this.denominator = denominator;
        }
        BigInteger getNumerator() { return numerator; }
        BigInteger getDenominator() { return denominator; }
        public Pair multiply(Pair other) {
            return new Pair(numerator.multiply(other.numerator), denominator.multiply(other.denominator));
        }
        Pair identity() { return this; }
        Pair withNumerator(BigInteger value) { return new Pair(value, denominator); }
        Pair withDenominator(BigInteger value) { return new Pair(numerator, value); }
        String text() { return numerator + " / " + denominator; }
        String numeratorText() { return String.valueOf(numerator); }
        Pair multiply(int value) {
            return new Pair(numerator.multiply(BigInteger.valueOf(value)), denominator);
        }
        Pair multiply(BigInteger value) {
            return new Pair(value.multiply(numerator), denominator);
        }
        Pair add(Pair other) {
            return new Pair(numerator.multiply(other.denominator).add(other.numerator.multiply(denominator)),
                    denominator.multiply(other.denominator));
        }
        int compareTo(Pair other) {
            return numerator.multiply(other.denominator).compareTo(other.numerator.multiply(denominator));
        }
    }

    public static int run() {
        BigInteger huge = BigInteger.valueOf(2).pow(100);
        Pair left = new Pair(huge, BigInteger.valueOf(3));
        Pair right = new Pair(BigInteger.valueOf(5), BigInteger.valueOf(7));
        for (int i = 0; i < 3; ++i) {
            Pair product = left.multiply(right);
            if (product.getNumerator().compareTo(huge.multiply(BigInteger.valueOf(5))) != 0) return 1;
            if (product.getDenominator().intValue() != 21) return 2;
            Pair sum = left.add(right);
            if (sum.getNumerator().compareTo(huge.multiply(BigInteger.valueOf(7)).add(BigInteger.valueOf(15))) != 0) return 3;
            if (sum.getDenominator().intValue() != 21 || left.compareTo(right) != 1) return 4;
            if (left.multiply(2).getNumerator().compareTo(huge.add(huge)) != 0) return 11;
            if (left.multiply(BigInteger.valueOf(2)).getNumerator().compareTo(huge.add(huge)) != 0) return 12;
        }
        if (left.getNumerator().compareTo(huge) != 0 || left.getDenominator().intValue() != 3) return 5;
        Pair missing = new Pair(null, BigInteger.ONE);
        if (missing.getNumerator() != null) return 6;
        try { left.multiply(missing); return 7; } catch (NullPointerException expected) { }
        try { missing.multiply(left); return 8; } catch (NullPointerException expected) { }
        try { left.multiply((Pair)null); return 9; } catch (NullPointerException expected) { }
        if (left.multiply(right).getDenominator().intValue() != 21) return 10;
        if (left.identity() != left || Pair.ZERO.identity() != Pair.ZERO) return 13;
        if (Pair.ZERO.multiply(right).getNumerator().signum() != 0) return 14;
        Pair changed = left.withDenominator(BigInteger.valueOf(11));
        if (changed.getNumerator().compareTo(huge) != 0 || changed.getDenominator().intValue() != 11) return 15;
        if (missing.withDenominator(BigInteger.valueOf(11)).getNumerator() != null) return 16;
        Pair missingDenominator = new Pair(BigInteger.ONE, null);
        if (missingDenominator.withNumerator(BigInteger.valueOf(9)).getDenominator() != null) return 17;
        if (!left.text().equals("1267650600228229401496703205376 / 3")) return 18;
        if (!missing.text().equals("null / 1") || !missingDenominator.text().equals("1 / null")) return 19;
        if (!missing.numeratorText().equals("null") || !left.numeratorText().equals("1267650600228229401496703205376")) return 20;
        if (left.getDenominator().intValue() != 3 || missing.getNumerator() != null) return 21;
        return 0;
    }
    public static void main(String[] args) { System.out.println(run()); }
}
