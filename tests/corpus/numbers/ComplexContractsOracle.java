package org.apache.commons.numbers.complex;

import java.util.List;

/** Client-only use and reference contracts; every complex algorithm is production code. */
public class ComplexContractsOracle {
    static boolean close(double actual, double expected) {
        return Math.abs(actual - expected) <= 1e-12 * Math.max(1.0, Math.abs(expected));
    }
    static boolean close(Complex actual, double real, double imaginary) {
        return close(actual.getReal(), real) && close(actual.getImaginary(), imaginary);
    }
    public static int run() {
        Complex z = Complex.parse(" (3,4) ".trim());
        Complex alias = z;
        if (z != alias || z.proj() != z || z.proj().proj() != z) return 1;
        if (z.conj() == z || z.negate() == z || z.add(0) == z) return 2;
        if (!z.equals(Complex.ofCartesian(3,4)) || z.hashCode() != Complex.ofCartesian(3,4).hashCode()) return 3;
        if (z.equals(null) || z.equals("(3,4)")) return 4;
        if (z.abs() != 5 || z.norm() != 25 || !z.isFinite() || z.isNaN() || z.isInfinite()) return 5;
        if (!close(z.multiply(z.conj()),25,0) || !close(z.divide(z),1,0)) return 6;
        if (!close(z.log().exp(),3,4) || !close(z.sqrt().multiply(z.sqrt()),3,4)) return 7;
        if (!close(z.pow(2),-7,24) || !close(z.pow(Complex.ofCartesian(2,0)),-7,24)) return 8;
        if (!close(z.sin().asin(),3,4) && !close(z.sin().asin(),Math.PI-3,-4)) return 9;

        // A simple AC phasor calculation: resistor in series with capacitive
        // reactance, then reconstruct the applied voltage from the current.
        Complex resistance = Complex.ofCartesian(100,0);
        Complex reactance = Complex.ofCartesian(0,-100);
        Complex impedance = resistance.add(reactance);
        Complex voltage = Complex.ofCartesian(230,0);
        Complex current = voltage.divide(impedance);
        if (!close(current,1.15,1.15) || !close(current.multiply(impedance),230,0)) return 10;
        if (!close(resistance,100,0) || !close(reactance,0,-100)) return 11;
        if (z != alias || z.getReal() != 3 || z.getImaginary() != 4) return 12;
        if (!Complex.parse(z.toString()).equals(z)) return 13;

        // The two sides of an actual complex-function branch cut use signed zero.
        Complex above = Complex.ofCartesian(-4,0.0).sqrt();
        Complex below = Complex.ofCartesian(-4,-0.0).sqrt();
        if (above.getReal() != 0 || below.getReal() != 0 || above.getImaginary() != 2 || below.getImaginary() != -2) return 14;
        if (Complex.ofCartesian(-1,0.0).log().getImaginary() != Math.PI) return 15;
        if (Complex.ofCartesian(-1,-0.0).log().getImaginary() != -Math.PI) return 16;

        List<Complex> roots = z.nthRoot(3);
        if (roots.size() != 3 || roots == z.nthRoot(3)) return 17;
        for (Complex root : roots) if (!close(root.pow(3),3,4)) return 18;
        roots.clear();
        if (z.nthRoot(3).size() != 3 || z.getReal() != 3 || z.getImaginary() != 4) return 19;

        try { z.add((Complex)null); return 20; } catch (NullPointerException expected) { }
        try { z.subtract((Complex)null); return 21; } catch (NullPointerException expected) { }
        try { z.multiply((Complex)null); return 22; } catch (NullPointerException expected) { }
        try { z.divide((Complex)null); return 23; } catch (NullPointerException expected) { }
        try { z.pow((Complex)null); return 24; } catch (NullPointerException expected) { }
        try { Complex.ZERO.pow((Complex)null); return 25; } catch (NullPointerException expected) { }
        try { Complex.parse(null); return 26; } catch (NullPointerException expected) { }
        Complex absent = null;
        try { absent.abs(); return 27; } catch (NullPointerException expected) { }
        try { z.nthRoot(0); return 28; } catch (IllegalArgumentException expected) {
            if (!"cannot compute zeroth root".equals(expected.getMessage())) return 29;
        }
        try { Complex.parse("(1,2,3)"); return 30; } catch (NumberFormatException expected) { }
        if (Complex.ZERO.pow(2) != Complex.ZERO || Complex.ZERO.pow(Complex.ONE) != Complex.ZERO) return 31;
        if (Complex.I != Complex.I || Complex.ONE != Complex.ONE || Complex.ZERO != Complex.ZERO) return 32;
        return 0;
    }
    public static void main(String[] args) {
        int code = run();
        if (code != 0) throw new AssertionError("Complex client contract: " + code);
        System.out.println("contract=" + code);
        System.out.println("oracle=ok");
    }
}
