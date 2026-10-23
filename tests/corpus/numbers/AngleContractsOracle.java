package org.apache.commons.numbers.angle;

import java.util.function.DoubleSupplier;
import java.util.function.DoubleUnaryOperator;

/** Client only: all conversions, reductions and vector work call production. */
public class AngleContractsOracle {
    public static DoubleSupplier supplier(Angle value) { return value; }
    public static DoubleUnaryOperator operator(Reduce value) { return value; }
    public static Angle angle(int unit, double value) {
        if (unit == 0) return Angle.Turn.of(value);
        if (unit == 1) return Angle.Rad.of(value);
        return Angle.Deg.of(value);
    }
    public static int run() {
        Angle.Turn t = Angle.Turn.of(0.25);
        Angle.Rad r = Angle.Rad.of(Math.PI / 2);
        Angle.Deg d = Angle.Deg.of(90);
        if (t.toTurn() != t || r.toRad() != r || d.toDeg() != d) return 1;
        if (t.toRad().getAsDouble() != Math.PI / 2 || t.toDeg().getAsDouble() != 90) return 2;
        if (r.toTurn().getAsDouble() != 0.25 || r.toDeg().getAsDouble() != 90) return 3;
        if (d.toTurn().getAsDouble() != 0.25 || d.toRad().getAsDouble() != Math.PI / 2) return 4;
        if (!t.equals(Angle.Turn.of(0.25)) || t.equals(r) || r.equals(d) || d.equals(t)) return 5;
        if (t.equals(null) || r.equals("foreign") || d.equals(null)) return 6;
        if (t.hashCode() != Double.hashCode(0.25) || r.hashCode() != Double.hashCode(Math.PI / 2)
                || d.hashCode() != Double.hashCode(90)) return 7;
        for (int unit = 0; unit < 3; unit++) {
            Angle a = angle(unit, 0.25);
            if (a.getAsDouble() != 0.25 || !a.equals(angle(unit, 0.25))) return 8;
            if (a.equals(angle((unit + 1) % 3, 0.25)) || a.equals(null)) return 9;
            if (supplier(a).getAsDouble() != 0.25 || supplier(a).getAsDouble() != 0.25) return 10;
            if (!angle(unit, Double.NaN).equals(angle(unit, Double.longBitsToDouble(0x7ff8000000000011L)))) return 11;
            if (angle(unit, 0).equals(angle(unit, -0.0))) return 12;
            if (a.toTurn().toRad().toDeg().getAsDouble() != a.toTurn().toRad().toDeg().getAsDouble()) return 13;
        }
        if (Angle.Turn.ZERO != Angle.Turn.ZERO || Angle.Rad.ZERO != Angle.Rad.ZERO
                || Angle.Deg.ZERO != Angle.Deg.ZERO || Angle.Rad.PI != Angle.Rad.PI
                || Angle.Rad.TWO_PI != Angle.Rad.TWO_PI) return 14;
        if (Angle.Turn.ZERO.getAsDouble() != 0 || Angle.Rad.ZERO.getAsDouble() != 0
                || Angle.Deg.ZERO.getAsDouble() != 0 || Angle.Rad.PI.getAsDouble() != Math.PI
                || Angle.Rad.TWO_PI.getAsDouble() != Angle.TWO_PI || Angle.PI_OVER_TWO != Math.PI / 2) return 15;
        if (Angle.Turn.WITHIN_0_AND_1 != Angle.Turn.WITHIN_0_AND_1
                || Angle.Rad.WITHIN_0_AND_2PI != Angle.Rad.WITHIN_0_AND_2PI
                || Angle.Rad.WITHIN_MINUS_PI_AND_PI != Angle.Rad.WITHIN_MINUS_PI_AND_PI
                || Angle.Deg.WITHIN_0_AND_360 != Angle.Deg.WITHIN_0_AND_360) return 16;
        if (Angle.Turn.WITHIN_0_AND_1.applyAsDouble(1.25) != 0.25
                || Angle.Rad.WITHIN_0_AND_2PI.applyAsDouble(Angle.TWO_PI) != 0
                || Angle.Rad.WITHIN_MINUS_PI_AND_PI.applyAsDouble(Math.PI) != -Math.PI
                || Angle.Deg.WITHIN_0_AND_360.applyAsDouble(-90) != 270) return 17;
        DoubleUnaryOperator turn = Angle.Turn.normalizer(-0.5);
        DoubleUnaryOperator rad = Angle.Rad.normalizer(-Math.PI);
        DoubleUnaryOperator deg = Angle.Deg.normalizer(-180);
        if (turn.applyAsDouble(0.75) != -0.25 || rad.applyAsDouble(Math.PI) != -Math.PI
                || deg.applyAsDouble(270) != -90 || turn.applyAsDouble(0.75) != -0.25) return 18;
        // The production correction branch handles values rounded to the upper bound.
        if (Angle.Rad.WITHIN_0_AND_2PI.applyAsDouble(-5e-17) != 0) return 19;
        if (Double.doubleToRawLongBits(Angle.Rad.WITHIN_MINUS_PI_AND_PI.applyAsDouble(-0.0))
                != Double.doubleToRawLongBits(-0.0)) return 20;
        Reduce reduce = new Reduce(10, -360);
        if (reduce.applyAsDouble(0) != 350 || reduce.applyAsDouble(370) != 0
                || operator(reduce).applyAsDouble(0) != 350 || reduce.applyAsDouble(0) != 350) return 21;
        double[] x = {1,0}; double[] y = {0,1};
        if (CosAngle.value(x,x) != 1 || CosAngle.value(x,y) != 0 || CosAngle.value(x,x) != 1) return 22;
        if (x[0] != 1 || x[1] != 0 || y[0] != 0 || y[1] != 1) return 23;
        try { CosAngle.value(x,new double[]{1}); return 24; }
        catch (IllegalArgumentException expected) { }
        try { CosAngle.value((double[])null,y); return 25; }
        catch (NullPointerException expected) { }
        try { CosAngle.value(x,(double[])null); return 26; }
        catch (NullPointerException expected) { }
        if (!Double.isNaN(CosAngle.value(new double[]{0,0},y))) return 27;
        if (turn.applyAsDouble(-0.25) != -0.25 || deg.applyAsDouble(-90) != -90) return 28;
        return 0;
    }
    public static void main(String[] args) {
        int code = run();
        if (code != 0) throw new AssertionError("Angle client contract: " + code);
        System.out.println("contract=" + code);
        System.out.println("oracle=ok");
    }
}
