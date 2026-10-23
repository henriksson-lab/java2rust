package org.apache.commons.numbers.core;

import java.math.BigDecimal;
import java.math.BigInteger;
import java.math.RoundingMode;

/** Caller-only observations; every numeric algorithm is untouched production code. */
public final class CoreOracle {
    private interface Operation { Object run(); }
    private static String bits(double x) { return Long.toHexString(Double.doubleToRawLongBits(x)); }
    private static String dd(DD value) { return bits(value.hi()) + "," + bits(value.lo()); }
    private static String arithmeticBits(double value) {
        return String.format("f64-arith(%016x)", Double.doubleToRawLongBits(value));
    }
    private static String arithmeticDD(DD value) {
        return arithmeticBits(value.hi()) + "," + arithmeticBits(value.lo());
    }
    private static String powerDD(DD value, int exponent) {
        // pow(1) returns this, and pow(0) returns the literal ONE.
        return exponent == 0 || exponent == 1 ? dd(value) : arithmeticDD(value);
    }
    private static void out(String key, Operation operation) {
        try { System.out.println(key + "=" + operation.run()); }
        catch (Exception error) {
            System.out.println(key + "=ERR:" + error.getClass().getSimpleName() + ":" + error.getMessage());
        }
    }

    private static void arithmetic() {
        int[][] pairs = {{0,0},{0,-7},{-7,0},{21,14},{-21,14},{Integer.MIN_VALUE,1},
            {Integer.MIN_VALUE,0},{Integer.MIN_VALUE,Integer.MIN_VALUE},{Integer.MAX_VALUE,2},{-1,-1},{1,0}};
        for (int i = 0; i < pairs.length; i++) {
            final int a = pairs[i][0], b = pairs[i][1]; String k = "arith.i." + i;
            out(k+".gcd", () -> ArithmeticUtils.gcd(a,b));
            out(k+".lcm", () -> ArithmeticUtils.lcm(a,b));
            out(k+".rem", () -> ArithmeticUtils.remainderUnsigned(a,b));
            out(k+".div", () -> ArithmeticUtils.divideUnsigned(a,b));
        }
        long[][] longs = {{0,0},{-7,0},{21,14},{-21,14},{Long.MIN_VALUE,1},
            {Long.MIN_VALUE,0},{Long.MIN_VALUE,Long.MIN_VALUE},{Long.MAX_VALUE,2},{-1,-1},{1,0},
            {0x123456789abcdefL,0x1234567L},{-1,3}};
        for (int i = 0; i < longs.length; i++) {
            final long a=longs[i][0], b=longs[i][1]; String k="arith.l."+i;
            out(k+".gcd", () -> ArithmeticUtils.gcd(a,b));
            out(k+".lcm", () -> ArithmeticUtils.lcm(a,b));
            out(k+".rem", () -> ArithmeticUtils.remainderUnsigned(a,b));
            out(k+".div", () -> ArithmeticUtils.divideUnsigned(a,b));
        }
        for (int base : new int[]{0,1,-1,2,-2,7,Integer.MIN_VALUE,Integer.MAX_VALUE}) {
            for (int e : new int[]{-1,0,1,2,3,8,31,32,63,64}) {
                String k="arith.pow."+base+"."+e;
                out(k+".int", () -> ArithmeticUtils.pow(base,e));
                out(k+".long", () -> ArithmeticUtils.pow((long)base,e));
            }
        }
        for (String s : new String[]{"0","1","-1","2","-3","12345678901234567890"}) {
            BigInteger base = new BigInteger(s);
            for (int e : new int[]{-1,0,1,2,5,31}) {
                String k="arith.big."+s+"."+e;
                out(k+".int", () -> ArithmeticUtils.pow(base,e));
                out(k+".long", () -> ArithmeticUtils.pow(base,(long)e));
                out(k+".big", () -> ArithmeticUtils.pow(base,BigInteger.valueOf(e)));
            }
        }
        for (long n : new long[]{Long.MIN_VALUE,-8,-1,0,1,2,3,4,1024,Long.MAX_VALUE})
            out("arith.power2."+n, () -> ArithmeticUtils.isPowerOfTwo(n));
        // Exercise the full exponent widths without constructing huge results.
        // These are calls into production loops, not replacement power logic.
        for (String s : new String[]{"0", "1", "-1"}) {
            BigInteger base = new BigInteger(s);
            for (long e : new long[]{Long.MIN_VALUE, -1, 0, 1, 2, 63, 64,
                    2147483647L, 2147483648L, Long.MAX_VALUE}) {
                out("arith.wide.long."+s+"."+e, () -> ArithmeticUtils.pow(base,e));
            }
            for (String text : new String[]{"-18446744073709551617", "-1", "0", "1", "2",
                    "63", "64", "2147483648", "9223372036854775808",
                    "18446744073709551616", "18446744073709551617",
                    "1267650600228229401496703205377"}) {
                BigInteger e = new BigInteger(text);
                out("arith.wide.big."+s+"."+text, () -> ArithmeticUtils.pow(base,e));
            }
            out("arith.wide.unchanged."+s, () -> base);
        }
    }

    private static double[] values() {
        return new double[]{0.0,-0.0,1.0,-1.0,0.1,-0.1,1e-200,1e200,
            Double.MIN_VALUE,Double.MIN_NORMAL,Double.MAX_VALUE,
            Double.POSITIVE_INFINITY,Double.NEGATIVE_INFINITY,Double.NaN};
    }

    private static void norms() {
        out("norm.values", () -> Norm.values().length);
        for (Norm n : Norm.values()) {
            String k="norm."+n.name();
            out(k+".ordinal", () -> n.ordinal());
            out(k+".lookup", () -> Norm.valueOf(n.name()) == n);
            out(k+".repeat", () -> Norm.values()[n.ordinal()] == n);
            for (int i=0; i<values().length; i++) {
                double x=values()[i], y=values()[(i+3)%values().length], z=values()[(i+7)%values().length];
                String p=k+"."+i;
                out(p+".two", () -> bits(n.of(x,y)));
                out(p+".three", () -> bits(n.of(x,y,z)));
                out(p+".array", () -> bits(n.of(new double[]{x,y,z,-x})));
            }
            out(k+".empty", () -> bits(n.of(new double[0])));
            out(k+".single", () -> bits(n.of(new double[]{-3})));
        }
        out("norm.badname", () -> Norm.valueOf("missing").name());
    }

    private static void precision() {
        out("precision.epsilon", () -> bits(Precision.EPSILON));
        out("precision.safe", () -> bits(Precision.SAFE_MIN));
        double[] vs=values();
        for (int i=0; i<vs.length; i++) for (int j=0; j<vs.length; j++) {
            double x=vs[i], y=vs[j]; float a=(float)x, b=(float)y;
            String k="precision."+i+"."+j;
            out(k+".cmp.eps", () -> Precision.compareTo(x,y,1e-12));
            out(k+".cmp.ulps", () -> Precision.compareTo(x,y,2));
            out(k+".f.eq", () -> Precision.equals(a,b));
            out(k+".f.nan", () -> Precision.equalsIncludingNaN(a,b));
            out(k+".f.eps", () -> Precision.equals(a,b,1e-6f));
            out(k+".f.nan.eps", () -> Precision.equalsIncludingNaN(a,b,1e-6f));
            out(k+".f.ulps", () -> Precision.equals(a,b,2));
            out(k+".f.nan.ulps", () -> Precision.equalsIncludingNaN(a,b,2));
            out(k+".d.eq", () -> Precision.equals(x,y));
            out(k+".d.nan", () -> Precision.equalsIncludingNaN(x,y));
            out(k+".d.eps", () -> Precision.equals(x,y,1e-12));
            out(k+".d.rel", () -> Precision.equalsWithRelativeTolerance(x,y,1e-12));
            out(k+".d.nan.eps", () -> Precision.equalsIncludingNaN(x,y,1e-12));
            out(k+".d.ulps", () -> Precision.equals(x,y,2));
            out(k+".d.nan.ulps", () -> Precision.equalsIncludingNaN(x,y,2));
            out(k+".delta", () -> bits(Precision.representableDelta(x,y)));
        }
        for (double x : new double[]{-0.0,0.0,0.1,2.5,-2.5,1.235,-1.235,1e100,Double.NaN,Double.POSITIVE_INFINITY}) {
            for (int scale : new int[]{-2,0,2,15}) {
                String k="precision.round."+bits(x)+"."+scale;
                out(k+".default", () -> bits(Precision.round(x,scale)));
                for (RoundingMode mode : RoundingMode.values())
                    out(k+"."+mode.ordinal(), () -> bits(Precision.round(x,scale,mode)));
            }
        }
        for (double eps : new double[]{0,1e-12,-1,Double.NaN,Double.POSITIVE_INFINITY}) {
            String k="precision.equiv."+bits(eps);
            out(k+".create", () -> { Precision.doubleEquivalenceOfEpsilon(eps); return "ok"; });
            if (!(eps>=0) || Double.isInfinite(eps)) continue;
            Precision.DoubleEquivalence eq=Precision.doubleEquivalenceOfEpsilon(eps);
            for (int i=0; i<vs.length; i++) {
                double x=vs[i], y=vs[(i+1)%vs.length]; String p=k+"."+i;
                out(p+".eq", () -> eq.eq(x,y)); out(p+".zero", () -> eq.eqZero(x));
                out(p+".lt", () -> eq.lt(x,y)); out(p+".lte", () -> eq.lte(x,y));
                out(p+".gt", () -> eq.gt(x,y)); out(p+".gte", () -> eq.gte(x,y));
                out(p+".sign", () -> bits(eq.signum(x))); out(p+".cmp", () -> eq.compare(x,y));
            }
        }
    }

    private static void sums() {
        double[][] arrays={{},{1},{1e16,1,-1e16},{1e-200,1e-200,-1e-200},
            {Double.MAX_VALUE,Double.MAX_VALUE},{Double.POSITIVE_INFINITY,1},
            {Double.POSITIVE_INFINITY,Double.NEGATIVE_INFINITY},{Double.NaN,1},{-0.0,0.0}};
        for (int i=0; i<arrays.length; i++) {
            double[] v=arrays[i]; String k="sum."+i;
            out(k+".of", () -> bits(Sum.of(v).getAsDouble()));
            Sum s=Sum.create(), saved=s;
            out(k+".identity", () -> s.add(v)==saved);
            out(k+".array", () -> bits(saved.getAsDouble()));
            for (double x : v) s.accept(x);
            out(k+".accept", () -> bits(s.getAsDouble()));
            out(k+".add", () -> { s.add(Sum.of(3)); return bits(s.getAsDouble()); });
            out(k+".subtract", () -> { s.subtract(Sum.of(3)); return arithmeticBits(s.getAsDouble()); });
            out(k+".selfadd", () -> { s.add(s); return bits(s.getAsDouble()); });
            out(k+".selfsubtract", () -> { s.subtract(s); return arithmeticBits(s.getAsDouble()); });
            out(k+".product", () -> bits(Sum.create().addProduct(0.1,0.2).getAsDouble()));
            out(k+".products", () -> bits(Sum.ofProducts(v,v).getAsDouble()));
            out(k+".addproducts", () -> bits(Sum.of(1).addProducts(v,v).getAsDouble()));
        }
        out("sum.scalar", () -> bits(Sum.of(2).add(3).getAsDouble()));
        out("sum.badproducts", () -> bits(Sum.ofProducts(new double[]{1},new double[0]).getAsDouble()));
        out("sum.badaddproducts", () -> bits(Sum.create().addProducts(new double[0],new double[]{1}).getAsDouble()));
        Sum object = Sum.create();
        String text = object.toString();
        out("sum.object.display", () -> text.equals(object.getClass().getName() + "@" + Integer.toHexString(object.hashCode())));
        Sum alias = object;
        out("sum.object.alias", () -> alias.toString().equals(text));
        out("sum.object.mutation", () -> { alias.add(3); return object.toString().equals(text); });
        Object erased = object;
        out("sum.object.erased", () -> erased.toString().equals(text));
    }

    private static void doubles() {
        for (int i=0; i<values().length; i++) {
            double x=values()[i]; DD a=DD.of(x); String k="dd."+i;
            out(k+".of", () -> dd(a));
            out(k+".finite", () -> a.isFinite()); out(k+".double", () -> bits(a.doubleValue()));
            out(k+".float", () -> Integer.toHexString(Float.floatToRawIntBits(a.floatValue())));
            out(k+".int", () -> a.intValue()); out(k+".long", () -> a.longValue());
            out(k+".decimal", () -> a.bigDecimalValue().toString());
            out(k+".negate", () -> dd(a.negate())); out(k+".abs", () -> dd(a.abs()));
            out(k+".floor", () -> dd(a.floor())); out(k+".ceil", () -> dd(a.ceil()));
            out(k+".square", () -> dd(a.square())); out(k+".sqrt", () -> dd(a.sqrt()));
            out(k+".reciprocal", () -> arithmeticDD(a.reciprocal()));
            out(k+".identity.pow1", () -> a.pow(1) == a);
            out(k+".identity.pow0", () -> a.pow(0) == DD.ONE);
            out(k+".identity.pow2fresh", () -> a.pow(2) != a);
            out(k+".identity.abs", () -> a.abs() == a);
            out(k+".add.d", () -> dd(a.add(0.1))); out(k+".add.dd", () -> dd(a.add(DD.ofSum(0.1,1e-18))));
            out(k+".sub.d", () -> dd(a.subtract(0.1))); out(k+".sub.dd", () -> dd(a.subtract(DD.of(0.1))));
            out(k+".mul.d", () -> dd(a.multiply(0.1))); out(k+".mul.dd", () -> dd(a.multiply(DD.of(0.1))));
            out(k+".mul.i", () -> dd(a.multiply(3)));
            out(k+".div.d", () -> arithmeticDD(a.divide(3))); out(k+".div.dd", () -> arithmeticDD(a.divide(DD.of(3))));
            out(k+".zero", () -> a.zero()==DD.ZERO); out(k+".iszero", () -> a.isZero());
            out(k+".one", () -> a.one()==DD.ONE); out(k+".isone", () -> a.isOne());
            out(k+".equals.self", () -> a.equals(a)); out(k+".equals.copy", () -> a.equals(DD.of(x)));
            out(k+".equals.null", () -> a.equals(null)); out(k+".equals.other", () -> a.equals("x"));
            out(k+".hash", () -> a.hashCode()); out(k+".string", () -> a.toString());
            out(k+".scalb", () -> dd(a.scalb(17)));
            out(k+".frexp", () -> { int[] e={123}; return dd(a.frexp(e))+","+e[0]; });
            for (int n : new int[]{-3,-1,0,1,2,5,31}) {
                String p=k+".pow."+n;
                out(p+".plain", () -> powerDD(a.pow(n),n));
                out(p+".scaled", () -> { long[] e={123}; return dd(a.pow(n,e))+","+e[0]; });
                out(p+".math", () -> { long[] e={123}; return dd(DDMath.pow(a,n,e))+","+e[0]; });
            }
            out(k+".unchanged", () -> dd(a));
        }
        for (long n : new long[]{Long.MIN_VALUE,-9007199254740993L,-1,0,1,9007199254740993L,Long.MAX_VALUE}) {
            out("dd.long."+n, () -> dd(DD.of(n))); out("dd.unsignedlong."+n, () -> dd(DD.ofUnsigned(n)));
        }
        for (int n : new int[]{Integer.MIN_VALUE,-1,0,1,Integer.MAX_VALUE}) {
            out("dd.int."+n, () -> dd(DD.of(n))); out("dd.unsignedint."+n, () -> dd(DD.ofUnsigned(n)));
        }
        for (String s : new String[]{"0","0.1","1.0000000000000000000000000001","1e300","1e-300","-12345678901234567890.12345"})
            out("dd.from."+s, () -> dd(DD.from(new BigDecimal(s))));
        double[][] pairs={{1e16,1},{0.1,0.2},{Double.MAX_VALUE,2},{Double.MIN_VALUE,0.5},{-0.0,0.0},{Double.NaN,1}};
        for (int i=0; i<pairs.length; i++) {
            double x=pairs[i][0], y=pairs[i][1]; String k="dd.factory."+i;
            out(k+".sum", () -> dd(DD.ofSum(x,y))); out(k+".diff", () -> dd(DD.ofDifference(x,y)));
            out(k+".prod", () -> dd(DD.ofProduct(x,y))); out(k+".square", () -> dd(DD.ofSquare(x)));
            out(k+".quot", () -> dd(DD.fromQuotient(x,y)));
        }
        DD a=DD.ofSum(3,1e-17), b=DD.of(2); NativeOperators<DD> n=a;
        Addition<DD> add=a; Multiplication<DD> mul=a;
        out("interfaces.add", () -> dd(add.add(b))); out("interfaces.zero", () -> dd(add.zero()));
        out("interfaces.negate", () -> dd(add.negate())); out("interfaces.iszero", () -> add.isZero());
        out("interfaces.multiply", () -> dd(mul.multiply(b))); out("interfaces.one", () -> dd(mul.one()));
        out("interfaces.reciprocal", () -> arithmeticDD(mul.reciprocal())); out("interfaces.isone", () -> mul.isOne());
        out("interfaces.subtract", () -> dd(n.subtract(b))); out("interfaces.divide", () -> arithmeticDD(n.divide(b)));
        out("interfaces.multiply.int", () -> dd(n.multiply(3))); out("interfaces.pow", () -> powerDD(n.pow(5),5));
        out("dd.frexp.empty", () -> dd(a.frexp(new int[0])));
        out("dd.pow.empty", () -> dd(a.pow(2,new long[0])));
        out("dd.math.empty", () -> dd(DDMath.pow(a,2,new long[0])));
    }

    private static void compensated() {
        DD[] xs = {DD.ofSum(1, 0x1p-55), DD.ofSum(1, -0x1p-55),
            DD.ofSum(-1, 0x1p-55), DD.ofSum(-1, -0x1p-55),
            DD.ofSum(2, 0x1p-54), DD.ofSum(2, -0x1p-54),
            DD.ofSum(-2, 0x1p-54), DD.ofSum(-2, -0x1p-54),
            DD.ofSum(0.5, 0x1p-56), DD.ofSum(-0.5, -0x1p-56)};
        for (int i = 0; i < xs.length; i++) {
            DD a = xs[i]; String k = "compensated."+i;
            out(k+".input", () -> dd(a));
            out(k+".floor", () -> dd(a.floor())); out(k+".ceil", () -> dd(a.ceil()));
            out(k+".negate", () -> dd(a.negate())); out(k+".abs", () -> dd(a.abs()));
            out(k+".square", () -> dd(a.square())); out(k+".sqrt", () -> dd(a.sqrt()));
            out(k+".reciprocal", () -> arithmeticDD(a.reciprocal()));
            out(k+".double", () -> bits(a.doubleValue()));
            out(k+".float", () -> Integer.toHexString(Float.floatToRawIntBits(a.floatValue())));
            out(k+".int", () -> a.intValue()); out(k+".long", () -> a.longValue());
            out(k+".decimal", () -> a.bigDecimalValue().toString());
            out(k+".finite", () -> a.isFinite());
            out(k+".equalscopy", () -> a.equals(DD.ofSum(a.hi(),a.lo())));
            out(k+".hash", () -> a.hashCode()); out(k+".string", () -> a.toString());
            out(k+".iszero", () -> a.isZero()); out(k+".isone", () -> a.isOne());
            for (int j = 0; j < xs.length; j++) {
                DD b = xs[j]; String p = k+".pair."+j;
                out(p+".add.d", () -> dd(a.add(b.hi()))); out(p+".add.dd", () -> dd(a.add(b)));
                out(p+".sub.d", () -> dd(a.subtract(b.hi()))); out(p+".sub.dd", () -> dd(a.subtract(b)));
                out(p+".mul.d", () -> dd(a.multiply(b.hi()))); out(p+".mul.dd", () -> dd(a.multiply(b)));
                out(p+".mul.i", () -> dd(a.multiply(3)));
                out(p+".div.d", () -> arithmeticDD(a.divide(b.hi())));
                out(p+".div.dd", () -> arithmeticDD(a.divide(b)));
            }
            out(k+".unchanged", () -> dd(a));
        }
    }

    private static void boundaries() {
        long[] encodings = {0L, 1L, 2L, Long.MIN_VALUE, Long.MIN_VALUE + 1,
            0x000fffffffffffffL, 0x0010000000000000L,
            0x3fefffffffffffffL, 0x3ff0000000000000L, 0x3ff0000000000001L,
            0x3ff0000000000002L, 0xbff0000000000000L, 0xbff0000000000001L,
            0x7fefffffffffffffL, 0x7ff0000000000000L, 0xfff0000000000000L,
            0x7ff8000000000000L, 0xfff8000000000000L};
        int[] floatEncodings = {0, 1, 2, Integer.MIN_VALUE, Integer.MIN_VALUE + 1,
            0x007fffff, 0x00800000, 0x3f7fffff, 0x3f800000, 0x3f800001,
            0x3f800002, 0xbf800000, 0xbf800001, 0x7f7fffff, 0x7f800000,
            0xff800000, 0x7fc00000, 0xffc00000};
        for (int i = 0; i < encodings.length; i++) {
            double x = Double.longBitsToDouble(encodings[i]);
            double y = Double.longBitsToDouble(encodings[(i + 1) % encodings.length]);
            float a = Float.intBitsToFloat(floatEncodings[i]);
            float b = Float.intBitsToFloat(floatEncodings[(i + 1) % floatEncodings.length]);
            for (int ulps : new int[]{-1, 0, 1, 2, 3, 4, Integer.MAX_VALUE}) {
                String k = "boundary.ulps." + i + "." + ulps;
                out(k+".d.eq", () -> Precision.equals(x,y,ulps));
                out(k+".d.nan", () -> Precision.equalsIncludingNaN(x,y,ulps));
                out(k+".d.cmp", () -> Precision.compareTo(x,y,ulps));
                out(k+".f.eq", () -> Precision.equals(a,b,ulps));
                out(k+".f.nan", () -> Precision.equalsIncludingNaN(a,b,ulps));
            }
        }
        DD[] values = {DD.of(0.0), DD.of(-0.0), DD.of(1), DD.of(-1),
            DD.ofSum(1.5, 0x1p-54), DD.ofSum(-0.75, 0x1p-56),
            DD.of(Double.MIN_VALUE), DD.of(Double.MIN_NORMAL), DD.of(Double.MAX_VALUE),
            DD.ofSum(1, 0x1p-53)};
        for (int i = 0; i < values.length; i++) {
            DD value = values[i];
            for (int scale : new int[]{Integer.MIN_VALUE, Integer.MIN_VALUE+1, -4097, -2048,
                    -1075, -1074, -1023, -1022, -1021, -53, -1, 0, 1, 52, 53,
                    1023, 1024, 2047, 4097, Integer.MAX_VALUE}) {
                String k = "boundary.scalb."+i+"."+scale;
                out(k, () -> dd(value.scalb(scale)));
            }
            for (int n : new int[]{Integer.MIN_VALUE, Integer.MAX_VALUE, -1075, -1024,
                    -100, -32, -2, -1, 0, 1, 2, 32, 100, 1024, 1075}) {
                String k = "boundary.pow."+i+"."+n;
                out(k+".plain", () -> powerDD(value.pow(n),n));
                out(k+".scaled", () -> {
                    long[] e = {123}; return dd(value.pow(n,e))+","+e[0];
                });
                out(k+".math", () -> {
                    long[] e = {123}; return dd(DDMath.pow(value,n,e))+","+e[0];
                });
            }
            out("boundary.unchanged."+i, () -> dd(value));
        }
        // Raw copy observations are NOT interchangeable with arithmetic NaNs.
        long[] copies = {0L, Long.MIN_VALUE, 1L, Long.MIN_VALUE + 1,
            0x3ff0000000000000L, 0xbff0000000000000L,
            0x7ff0000000000000L, 0xfff0000000000000L,
            0x7ff8000000000000L, 0xfff8000000000000L,
            0x7ff8000000000042L, 0xfff8000000000042L};
        for (int i = 0; i < copies.length; i++) {
            double input = Double.longBitsToDouble(copies[i]);
            out("boundary.copy."+i, () -> dd(DD.of(input)));
        }
    }

    public static void main(String[] args) {
        arithmetic(); norms(); precision(); sums(); doubles();
        boundaries(); compensated();
        System.out.println("oracle=ok");
    }
}
