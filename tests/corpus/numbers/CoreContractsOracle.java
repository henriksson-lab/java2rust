package org.apache.commons.numbers.core;

import java.math.BigDecimal;
import java.math.BigInteger;

/** Client-only contract checks; numerical operations delegate to production DD. */
class ContractElement implements NativeOperators<ContractElement> {
    final DD value;
    int zeros;
    int ones;
    int equalities;
    ContractElement(DD value) { this.value = value; }
    public ContractElement add(ContractElement other) { return new ContractElement(value.add(other.value)); }
    public ContractElement zero() { zeros++; return new ContractElement(DD.ZERO); }
    public ContractElement negate() { return new ContractElement(value.negate()); }
    public ContractElement multiply(ContractElement other) { return new ContractElement(value.multiply(other.value)); }
    public ContractElement one() { ones++; return new ContractElement(DD.ONE); }
    public ContractElement reciprocal() { return new ContractElement(value.reciprocal()); }
    public ContractElement subtract(ContractElement other) { return new ContractElement(value.subtract(other.value)); }
    public ContractElement divide(ContractElement other) { return new ContractElement(value.divide(other.value)); }
    public ContractElement multiply(int factor) { return new ContractElement(value.multiply(factor)); }
    public ContractElement pow(int exponent) { return new ContractElement(value.pow(exponent)); }
    public boolean equals(Object other) {
        equalities++;
        if (!(other instanceof ContractElement)) return false;
        return value.equals(((ContractElement)other).value);
    }
}

public class CoreContractsOracle {
    public static int run() {
        ContractElement zero = new ContractElement(DD.ZERO);
        Addition<ContractElement> addition = zero;
        if (!addition.isZero()) return 1;
        if (zero.zeros != 1 || zero.equalities != 1) return 2;
        ContractElement one = new ContractElement(DD.ONE);
        Multiplication<ContractElement> multiplication = one;
        if (!multiplication.isOne()) return 3;
        if (one.ones != 1 || one.equalities != 1) return 4;
        ContractElement two = new ContractElement(DD.of(2));
        if (two.isZero() || two.isOne()) return 5;
        if (two.zeros != 1 || two.ones != 1 || two.equalities != 2) return 6;
        NativeOperators<ContractElement> nativeOperations = two;
        if (nativeOperations.isZero() || nativeOperations.isOne()) return 7;
        if (two.zeros != 2 || two.ones != 2 || two.equalities != 4) return 8;
        if (!nativeOperations.subtract(one).value.equals(DD.ONE)) return 9;

        try { Sum.of((double[])null); return 10; } catch (NullPointerException expected) { }
        try { Sum.create().add((Sum)null); return 11; } catch (NullPointerException expected) { }
        try { Sum.ofProducts(null, new double[0]); return 12; } catch (NullPointerException expected) { }
        try { DD.from((BigDecimal)null); return 13; } catch (NullPointerException expected) { }
        try { DD.of(1).add((DD)null); return 14; } catch (NullPointerException expected) { }
        try { DD.of(1).frexp(null); return 15; } catch (NullPointerException expected) { }
        try { DD.of(1.5).pow(2, null); return 16; } catch (NullPointerException expected) { }
        try { DDMath.pow(DD.of(1.5), 3, null); return 17; } catch (NullPointerException expected) { }
        try { Norm.L2.of((double[])null); return 18; } catch (NullPointerException expected) { }
        try { Norm.valueOf(null); return 19; } catch (NullPointerException expected) { }
        try { ArithmeticUtils.pow((BigInteger)null, 2); return 20; } catch (NullPointerException expected) { }
        try { ArithmeticUtils.pow((BigInteger)null, 2L); return 21; } catch (NullPointerException expected) { }
        try { ArithmeticUtils.pow((BigInteger)null, BigInteger.TWO); return 22; } catch (NullPointerException expected) { }
        try { ArithmeticUtils.pow(BigInteger.ONE, (BigInteger)null); return 23; } catch (NullPointerException expected) { }
        try { Precision.round(1.25, 1, null); return 24; } catch (NullPointerException expected) { }
        try { DD.of(1).subtract((DD)null); return 25; } catch (NullPointerException expected) { }
        try { DD.of(1).multiply((DD)null); return 26; } catch (NullPointerException expected) { }
        try { DD.of(1).divide((DD)null); return 27; } catch (NullPointerException expected) { }
        try { Sum.create().subtract(null); return 28; } catch (NullPointerException expected) { }
        try { Sum.create().add((double[])null); return 29; } catch (NullPointerException expected) { }
        try { Sum.create().addProducts(null, new double[0]); return 30; } catch (NullPointerException expected) { }
        try { Sum.create().addProducts(new double[0], null); return 31; } catch (NullPointerException expected) { }
        try { Sum.ofProducts(new double[0], null); return 32; } catch (NullPointerException expected) { }
        if (DD.of(255).byteValue() != -1) return 33;
        if (DD.of(65535).shortValue() != -1) return 34;
        return 0;
    }
    public static void main(String[] args) {
        int result = run();
        if (result != 0) throw new AssertionError("Core contract " + result);
        System.out.println("contract=0");
        System.out.println("oracle=ok");
    }
}
