class Math {
    static String multiplyExact(int a, int b) { return "source-multiply"; }
    static int getExponent(double value) { return 17; }
    static String scalb(double value, int exponent) { return "source-scale"; }
}
class Integer {
    static String signum(int value) { return "source-signum"; }
    static String divideUnsigned(int a, int b) { return "source-divide"; }
    static String toHexString(char value) { return "source-hex"; }
    static String compare(int a, int b) { return "source-compare"; }
    static String compareUnsigned(int a, int b) { return "source-unsigned-compare"; }
    static String toString(int value) { return "source-decimal"; }
}
public class NativeNumericShadowProbe {
    static int run() {
        if (!Math.multiplyExact(1, 2).equals("source-multiply")) return 1;
        if (Math.getExponent(0.0) != 17) return 2;
        if (!Integer.divideUnsigned(1, 0).equals("source-divide")) return 3;
        if (java.lang.Math.multiplyExact(2, 3) != 6) return 4;
        if (java.lang.Integer.divideUnsigned(-1, 2) != 2147483647) return 5;
        if (!Math.scalb(1.0, 2).equals("source-scale")) return 6;
        if (java.lang.Math.scalb(1.5, 2) != 6.0) return 7;
        if (!Integer.toHexString('A').equals("source-hex")) return 8;
        if (!java.lang.Integer.toHexString('A').equals("41")) return 9;
        if (!Integer.compare(1, 2).equals("source-compare")) return 10;
        if (!Integer.compareUnsigned(1, 2).equals("source-unsigned-compare")) return 11;
        if (java.lang.Integer.compare(-1, 0) != -1) return 12;
        if (java.lang.Integer.compareUnsigned(-1, 0) != 1) return 13;
        if (!Integer.toString(1).equals("source-decimal")) return 14;
        if (!java.lang.Integer.toString(-12).equals("-12")) return 15;
        if (!Integer.signum(-1).equals("source-signum")) return 16;
        if (java.lang.Integer.signum(-1) != -1 || java.lang.Long.signum(0) != 0) return 17;
        return 0;
    }
    public static void main(String[] args) { System.out.println(run()); }
}
