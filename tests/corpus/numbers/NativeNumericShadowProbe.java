class Math {
    static String multiplyExact(int a, int b) { return "source-multiply"; }
    static int getExponent(double value) { return 17; }
    static String scalb(double value, int exponent) { return "source-scale"; }
}
class Integer {
    static String divideUnsigned(int a, int b) { return "source-divide"; }
    static String toHexString(char value) { return "source-hex"; }
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
        return 0;
    }
    public static void main(String[] args) { System.out.println(run()); }
}
