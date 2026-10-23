/** Independent JDK observations; StrictMath is diagnostic, not a translator target here. */
public class ElementaryMathOracle {
    private static String bits(double value) {
        return Long.toHexString(Double.doubleToRawLongBits(value));
    }
    public static void main(String[] args) {
        System.out.println("cosh.1=" + bits(ElementaryMathProbe.cosh(1.0)));
        System.out.println("cosh.-1=" + bits(ElementaryMathProbe.cosh(-1.0)));
        System.out.println("atan2.-3.-2=" + bits(ElementaryMathProbe.atan2(-3.0, -2.0)));
        System.out.println("cosh.+0=" + bits(ElementaryMathProbe.cosh(0.0)));
        System.out.println("cosh.-0=" + bits(ElementaryMathProbe.cosh(-0.0)));
        System.out.println("atan2.+0.-1=" + bits(ElementaryMathProbe.atan2(0.0, -1.0)));
        System.out.println("atan2.-0.-1=" + bits(ElementaryMathProbe.atan2(-0.0, -1.0)));
        System.out.println("atan2.+0.+1=" + bits(ElementaryMathProbe.atan2(0.0, 1.0)));
        System.out.println("atan2.-0.+1=" + bits(ElementaryMathProbe.atan2(-0.0, 1.0)));
        System.out.println("strict.cosh.1=" + bits(StrictMath.cosh(1.0)));
        System.out.println("strict.atan2.-3.-2=" + bits(StrictMath.atan2(-3.0, -2.0)));
    }
}
