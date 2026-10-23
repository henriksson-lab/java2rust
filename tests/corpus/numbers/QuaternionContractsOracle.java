package org.apache.commons.numbers.quaternion;

import java.util.function.DoubleFunction;

/** Client only: every numerical operation is an untouched production call. */
public class QuaternionContractsOracle {
    public static DoubleFunction<Quaternion> asCallback(Slerp value) { return value; }
    public static int run() {
        Quaternion q = Quaternion.of(1,2,3,4);
        Quaternion alias = q;
        if (q != alias || !q.equals(Quaternion.of(1,2,3,4))) return 1;
        if (q.hashCode() != Quaternion.of(1,2,3,4).hashCode()) return 2;
        if (q.equals(null) || q.equals("other")) return 3;
        if (q.getW() != 1 || q.getX() != 2 || q.getY() != 3 || q.getZ() != 4) return 4;
        if (q.getScalarPart() != 1 || q.normSq() != 30 || q.norm() != Math.sqrt(30)) return 5;
        if (!q.conjugate().equals(Quaternion.of(1,-2,-3,-4))) return 6;
        if (!q.add(q).equals(Quaternion.add(q,q)) || !q.add(q).equals(q.multiply(2))) return 7;
        if (!q.subtract(q).equals(Quaternion.subtract(q,q)) || !q.subtract(q).equals(Quaternion.ZERO)) return 8;
        if (!q.multiply(q).equals(Quaternion.multiply(q,q)) || q.dot(q) != Quaternion.dot(q,q)) return 9;
        if (!q.multiply(2).divide(2).equals(q) || !q.negate().negate().equals(q)) return 10;
        if (!q.multiply(q.inverse()).equals(Quaternion.ONE, 1e-14)) return 11;
        Quaternion normalized = q.normalize();
        if (normalized == q || normalized.normalize() != normalized || normalized.norm() != 1
                || normalized.normSq() != 1 || !normalized.isUnit(-1)) return 12;
        Quaternion polar = normalized.positivePolarForm();
        if (polar.getW() < 0 || polar.positivePolarForm() != polar || polar.normalize() != polar) return 13;
        if (!Quaternion.I.isPure(0) || q.isPure(0) || !Quaternion.ONE.isUnit(0)) return 14;
        if (!Quaternion.parse(q.toString()).equals(q) || q != alias || q.getX() != 2) return 15;
        double[] vector = {2,3,4};
        Quaternion fromVector = Quaternion.of(1,vector);
        Quaternion pure = Quaternion.of(vector);
        vector[0] = 99;
        double[] copy = fromVector.getVectorPart(); copy[1] = 99;
        if (!fromVector.equals(q) || !pure.equals(Quaternion.of(0,2,3,4))
                || fromVector.getVectorPart() == fromVector.getVectorPart()) return 16;
        try { Quaternion.of(new double[2]); return 17; } catch (IllegalArgumentException expected) { }
        try { Quaternion.of(1,new double[4]); return 18; } catch (IllegalArgumentException expected) { }
        try { Quaternion.of((double[])null); return 19; } catch (NullPointerException expected) { }
        try { Quaternion.of(1,(double[])null); return 20; } catch (NullPointerException expected) { }
        try { Quaternion.ZERO.normalize(); return 21; } catch (IllegalStateException expected) { }
        try { Quaternion.ZERO.inverse(); return 22; } catch (IllegalStateException expected) { }
        try { Quaternion.parse("[\ud800 2 3 4]"); return 23; }
        catch (NumberFormatException expected) {
            if (expected.getCause() == null || expected.getMessage().charAt(27) != '\ud800'
                    || expected.getCause().getMessage().indexOf('\ud800') < 0) return 24;
        }
        try { Quaternion.parse(null); return 25; } catch (NullPointerException expected) { }
        try { q.add((Quaternion)null); return 26; } catch (NullPointerException expected) { }
        try { q.subtract((Quaternion)null); return 27; } catch (NullPointerException expected) { }
        try { q.multiply((Quaternion)null); return 28; } catch (NullPointerException expected) { }
        try { q.dot(null); return 29; } catch (NullPointerException expected) { }
        try { q.equals(null,0); return 30; } catch (NullPointerException expected) { }
        Quaternion start = Quaternion.ONE.positivePolarForm();
        Slerp interpolation = new Slerp(start,Quaternion.I);
        DoubleFunction<Quaternion> callback = interpolation;
        if (interpolation.apply(0) != start || !interpolation.apply(1).equals(Quaternion.I)) return 31;
        if (!callback.apply(0.5).equals(interpolation.apply(0.5))
                || !interpolation.apply(0.5).equals(interpolation.apply(0.5))) return 32;
        if (!asCallback(interpolation).apply(0.5).equals(callback.apply(0.5))) return 36;
        try { new Slerp(null,Quaternion.ONE); return 33; } catch (NullPointerException expected) { }
        try { new Slerp(Quaternion.ONE,null); return 34; } catch (NullPointerException expected) { }
        if (Quaternion.ZERO != Quaternion.ZERO || Quaternion.ONE != Quaternion.ONE
                || Quaternion.I != Quaternion.I || Quaternion.J != Quaternion.J || Quaternion.K != Quaternion.K) return 35;
        return 0;
    }
    public static void main(String[] args) {
        int code = run();
        if (code != 0) throw new AssertionError("Quaternion client contract: " + code);
        System.out.println("contract=" + code);
        System.out.println("oracle=ok");
    }
}
