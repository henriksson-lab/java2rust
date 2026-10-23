package org.apache.commons.numbers.complex;

/** Caller-only oracle: every numeric operation invokes untouched production. */
public class ComplexOracle {
    interface Operation { Object run() throws Exception; }
    static String bits(double value) { return Long.toHexString(Double.doubleToRawLongBits(value)); }
    static String pair(Complex value) { return bits(value.getReal()) + "," + bits(value.getImaginary()); }
    static void out(String key, Operation operation) {
        try { System.out.println(key + "=" + operation.run()); }
        catch (Exception error) {
            System.out.println(key + "=ERR:" + error.getClass().getSimpleName() + ":" + error.getMessage());
        }
    }
    static double[][] pairs() {
        return new double[][] {
            {0.0,0.0}, {0.0,-0.0}, {-0.0,0.0}, {-0.0,-0.0},
            {1.0,0.0}, {1.0,-0.0}, {-1.0,0.0}, {-1.0,-0.0},
            {0.0,1.0}, {-0.0,1.0}, {0.0,-1.0}, {-0.0,-1.0},
            {1.0,1.0}, {-1.0,1.0}, {1.0,-1.0}, {-1.0,-1.0},
            {0.5,0.25}, {0.1,-0.3}, {2.0,3.0}, {-2.0,-3.0},
            {1e-200,1e-200}, {1e200,-1e200}, {Double.MIN_NORMAL,Double.MIN_VALUE},
            {Double.MAX_VALUE,Double.MAX_VALUE}, {Double.MAX_VALUE,Double.MIN_VALUE},
            {Double.POSITIVE_INFINITY,1.0}, {1.0,Double.NEGATIVE_INFINITY},
            {Double.POSITIVE_INFINITY,Double.NEGATIVE_INFINITY}, {Double.NaN,1.0},
            {1.0,Double.NaN}, {Double.POSITIVE_INFINITY,Double.NaN},
            {Double.longBitsToDouble(0xfff8000000001234L),-0.0},
            {708.0,-708.0}, {Double.longBitsToDouble(0x3ff0000000000001L),1e-16}
        };
    }
    static double[] scalars() {
        return new double[] {0.0,-0.0,1.0,-1.0,0.5,-0.5,0.1,-0.1,1e-200,1e200,
            Double.MIN_VALUE,Double.MIN_NORMAL,Double.MAX_VALUE,
            Double.POSITIVE_INFINITY,Double.NEGATIVE_INFINITY,Double.NaN};
    }
    static void numeric() {
        double[][] inputs = pairs();
        for (int i=0; i<inputs.length; i++) {
            Complex z=Complex.ofCartesian(inputs[i][0],inputs[i][1]);
            String k="z."+i;
            out(k+".cartesian", () -> pair(z));
            out(k+".getReal", () -> bits(z.getReal())); out(k+".real", () -> bits(z.real()));
            out(k+".getImaginary", () -> bits(z.getImaginary())); out(k+".imag", () -> bits(z.imag()));
            out(k+".abs", () -> bits(z.abs())); out(k+".arg", () -> bits(z.arg()));
            out(k+".norm", () -> bits(z.norm())); out(k+".isNaN", () -> z.isNaN());
            out(k+".isInfinite", () -> z.isInfinite()); out(k+".isFinite", () -> z.isFinite());
            out(k+".conj", () -> pair(z.conj())); out(k+".negate", () -> pair(z.negate()));
            out(k+".proj", () -> pair(z.proj())); out(k+".exp", () -> pair(z.exp()));
            out(k+".log", () -> pair(z.log())); out(k+".log10", () -> pair(z.log10()));
            out(k+".sqrt", () -> pair(z.sqrt())); out(k+".sin", () -> pair(z.sin()));
            out(k+".cos", () -> pair(z.cos())); out(k+".tan", () -> pair(z.tan()));
            out(k+".asin", () -> pair(z.asin())); out(k+".acos", () -> pair(z.acos()));
            out(k+".atan", () -> pair(z.atan())); out(k+".sinh", () -> pair(z.sinh()));
            out(k+".cosh", () -> pair(z.cosh())); out(k+".tanh", () -> pair(z.tanh()));
            out(k+".asinh", () -> pair(z.asinh())); out(k+".acosh", () -> pair(z.acosh()));
            out(k+".atanh", () -> pair(z.atanh()));
            out(k+".equals.self", () -> z.equals(z));
            out(k+".equals.fresh", () -> z.equals(Complex.ofCartesian(z.real(),z.imag())));
            out(k+".equals.null", () -> z.equals(null)); out(k+".equals.string", () -> z.equals("other"));
            out(k+".hash", () -> z.hashCode()); out(k+".string", () -> z.toString());
            out(k+".identity.proj", () -> z.proj()==z);
            out(k+".identity.conj", () -> z.conj()==z);
            out(k+".identity.negate", () -> z.negate()==z);
            for (int j=0; j<scalars().length; j++) {
                double x=scalars()[j]; String s=k+".scalar."+j;
                out(s+".add", () -> pair(z.add(x))); out(s+".addImaginary", () -> pair(z.addImaginary(x)));
                out(s+".subtract", () -> pair(z.subtract(x))); out(s+".subtractImaginary", () -> pair(z.subtractImaginary(x)));
                out(s+".subtractFrom", () -> pair(z.subtractFrom(x))); out(s+".subtractFromImaginary", () -> pair(z.subtractFromImaginary(x)));
                out(s+".multiply", () -> pair(z.multiply(x))); out(s+".multiplyImaginary", () -> pair(z.multiplyImaginary(x)));
                out(s+".divide", () -> pair(z.divide(x))); out(s+".divideImaginary", () -> pair(z.divideImaginary(x)));
                out(s+".pow", () -> pair(z.pow(x)));
            }
            for (int j=0; j<inputs.length; j++) {
                Complex w=Complex.ofCartesian(inputs[j][0],inputs[j][1]); String b=k+".binary."+j;
                out(b+".add", () -> pair(z.add(w))); out(b+".subtract", () -> pair(z.subtract(w)));
                out(b+".multiply", () -> pair(z.multiply(w))); out(b+".divide", () -> pair(z.divide(w)));
                out(b+".pow", () -> pair(z.pow(w)));
                out(b+".equals", () -> z.equals(w));
            }
            for (int n : new int[]{-3,-1,0,1,2,3,4}) {
                out(k+".roots."+n, () -> {
                    StringBuilder result=new StringBuilder();
                    for (Complex root : z.nthRoot(n)) { if (result.length()>0) result.append(';'); result.append(pair(root)); }
                    return result.toString();
                });
            }
            out(k+".after", () -> pair(z));
            double re=inputs[i][0], im=inputs[i][1];
            out(k+".polar", () -> pair(Complex.ofPolar(re,im)));
            out(k+".cis", () -> pair(Complex.ofCis(re)));
        }
    }
    static void parsing() {
        String[] inputs={"(1,2)","(0.0,-0.0)","(-0.0,0.0)","(NaN,Infinity)","(-Infinity,NaN)",
            "( 1 , 2 )","(\t1,2\t)","(0x1p2,-0x1p-4)","(1f,2D)","(+1e2,-2e-3)",
            "(1e9999,1e-9999)","", "(1)","[1,2)","(1,2]","(,2)","(1,)","(1,2,3)",
            "(abc,2)","(1,abc)","(inf,2)","(nan,2)","(1_0,2)","(😀,1)","(é,1)","(1,２)"};
        for (int i=0; i<inputs.length; i++) { String s=inputs[i]; out("parse."+i, () -> pair(Complex.parse(s))); }
    }
    public static void main(String[] args) {
        out("constant.I", () -> pair(Complex.I)); out("constant.ONE", () -> pair(Complex.ONE));
        out("constant.ZERO", () -> pair(Complex.ZERO));
        out("constant.identity", () -> Complex.I==Complex.I && Complex.ONE==Complex.ONE && Complex.ZERO==Complex.ZERO);
        numeric(); parsing(); System.out.println("oracle=ok");
    }
}
