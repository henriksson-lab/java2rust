package org.apache.commons.numbers.angle;

import java.util.function.DoubleUnaryOperator;

/** Independent caller; every angle/vector/reduction algorithm is production. */
public class AngleOracle {
    interface Operation { String run(); }
    static String bits(double x) { return Long.toHexString(Double.doubleToRawLongBits(x)); }
    static String units(String text) {
        if (text == null) return "null";
        StringBuilder out = new StringBuilder();
        for (int i = 0; i < text.length(); i++) {
            if (i != 0) out.append(',');
            out.append(Integer.toHexString(text.charAt(i)));
        }
        return out.toString();
    }
    static void out(String key, Operation operation) {
        try { System.out.println(key + "=" + operation.run()); }
        catch (Exception e) {
            System.out.println(key + "=ERR:" + e.getClass().getSimpleName() + ":" + units(e.getMessage()));
        }
    }
    static Angle angle(int unit, double value) {
        if (unit == 0) return Angle.Turn.of(value);
        if (unit == 1) return Angle.Rad.of(value);
        return Angle.Deg.of(value);
    }
    static void angles() {
        double[] values = {0,-0.0,0.125,0.25,0.5,1,-1,45,-90,180,360,720,-1080,1e6,
            Double.NaN,Double.POSITIVE_INFINITY,Double.NEGATIVE_INFINITY,
            Double.longBitsToDouble(0x7ff8000000000011L)};
        for (int unit = 0; unit < 3; unit++) {
            for (int i = 0; i < values.length; i++) {
                Angle a = angle(unit, values[i]); String k = "angle." + unit + "." + i;
                out(k + ".values", () -> bits(a.getAsDouble()) + "," + bits(a.toTurn().getAsDouble())
                    + "," + bits(a.toRad().getAsDouble()) + "," + bits(a.toDeg().getAsDouble()));
                out(k + ".cycles", () -> bits(a.toTurn().toRad().toDeg().getAsDouble())
                    + "," + bits(a.toRad().toDeg().toTurn().getAsDouble())
                    + "," + bits(a.toDeg().toTurn().toRad().getAsDouble()));
                out(k + ".hash", () -> Integer.toString(a.hashCode()));
                out(k + ".identity", () -> (a == a.toTurn()) + "," + (a == a.toRad()) + "," + (a == a.toDeg()));
                out(k + ".self", () -> Boolean.toString(a.equals(a)));
                out(k + ".null", () -> Boolean.toString(a.equals(null)));
                out(k + ".foreign", () -> Boolean.toString(a.equals("foreign")));
                out(k + ".class", () -> a.getClass().getName());
                for (int other = 0; other < 3; other++) {
                    for (int j = 0; j < values.length; j++) {
                        Angle b = angle(other, values[j]);
                        out(k + ".equals." + other + "." + j, () -> Boolean.toString(a.equals(b)));
                    }
                }
                out(k + ".after", () -> bits(a.getAsDouble()));
            }
        }
        out("constant.twoPi", () -> bits(Angle.TWO_PI));
        out("constant.halfPi", () -> bits(Angle.PI_OVER_TWO));
        out("constant.turnZero", () -> bits(Angle.Turn.ZERO.getAsDouble()));
        out("constant.radZero", () -> bits(Angle.Rad.ZERO.getAsDouble()));
        out("constant.degZero", () -> bits(Angle.Deg.ZERO.getAsDouble()));
        out("constant.radPi", () -> bits(Angle.Rad.PI.getAsDouble()));
        out("constant.radTwoPi", () -> bits(Angle.Rad.TWO_PI.getAsDouble()));
    }
    static void reductions() {
        // Ordinary wrapped headings plus production's near-bound correction cases.
        double[] values = {0,-0.0,-5e-17,5e-17,-1,1,-Angle.TWO_PI,-Math.PI,Math.PI,
            Angle.TWO_PI,45,90,180,360,720,-1080,1e6,Math.nextDown(Math.PI / 3),
            Math.PI / 3,Math.nextUp(Math.PI / 3),Double.NaN,Double.POSITIVE_INFINITY,Double.NEGATIVE_INFINITY};
        DoubleUnaryOperator[] operators = {
            Angle.Turn.WITHIN_0_AND_1, Angle.Rad.WITHIN_0_AND_2PI,
            Angle.Rad.WITHIN_MINUS_PI_AND_PI, Angle.Deg.WITHIN_0_AND_360,
            Angle.Turn.normalizer(-0.5),Angle.Rad.normalizer(Math.PI / 3),Angle.Deg.normalizer(-180),
            new Reduce(0,360),new Reduce(10,-360),new Reduce(-0.5,1),new Reduce(2,0)
        };
        for (int i = 0; i < operators.length; i++) {
            DoubleUnaryOperator op = operators[i];
            for (int j = 0; j < values.length; j++) {
                double x = values[j]; String k = "reduce." + i + "." + j;
                out(k + ".value", () -> bits(op.applyAsDouble(x)));
                out(k + ".repeat", () -> bits(op.applyAsDouble(x)));
                out(k + ".twice", () -> bits(op.applyAsDouble(op.applyAsDouble(x))));
            }
        }
        Reduce direct = new Reduce(10,-360);
        for (int j = 0; j < values.length; j++) {
            double x = values[j]; out("direct." + j, () -> bits(direct.applyAsDouble(x)));
        }
    }
    static void vectors() {
        double[][] values = {{1,0},{0,1},{7,7},{-5,0},{-100,100},{0,0},
            {1e-200,1e-200},{-1e200,-1e200},{1e200,-1e200},
            {1,1,0},{1,1,1},{},{1},{Double.NaN,1},{Double.POSITIVE_INFINITY,1}};
        for (int i = 0; i < values.length; i++) {
            for (int j = 0; j < values.length; j++) {
                double[] a = values[i]; double[] b = values[j]; String k = "cos." + i + "." + j;
                out(k + ".value", () -> bits(CosAngle.value(a,b)));
                out(k + ".repeat", () -> bits(CosAngle.value(a,b)));
            }
        }
    }
    public static void main(String[] args) {
        angles(); reductions(); vectors();
        System.out.println("oracle=ok");
    }
}
