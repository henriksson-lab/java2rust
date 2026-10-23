package org.apache.commons.numbers.quaternion;

import java.util.function.DoubleFunction;

/** Caller only: every quaternion/interpolation algorithm is pinned production. */
public class QuaternionOracle {
    interface Operation { Object run(); }
    static String bits(double x) { return Long.toHexString(Double.doubleToRawLongBits(x)); }
    static String parts(Quaternion q) {
        return bits(q.getW()) + "," + bits(q.getX()) + "," + bits(q.getY()) + "," + bits(q.getZ());
    }
    static String units(String s) {
        if (s == null) return "null";
        StringBuilder out = new StringBuilder();
        for (int i=0; i<s.length(); i++) {
            if (i != 0) out.append(',');
            out.append(Integer.toHexString(s.charAt(i)));
        }
        return out.toString();
    }
    static String error(Throwable e) {
        String result = "ERR:" + e.getClass().getSimpleName() + ":" + units(e.getMessage());
        if (e.getCause() != null) result += ":CAUSE:" + error(e.getCause());
        return result;
    }
    static void out(String key, Operation operation) {
        try { System.out.println(key + "=" + operation.run()); }
        catch (Exception e) { System.out.println(key + "=" + error(e)); }
    }
    static double[][] inputs() {
        return new double[][] {
            {0,0,0,0}, {1,0,0,0}, {-1,0,0,0}, {0,1,0,0}, {0,0,1,0}, {0,0,0,1},
            {1,2,3,4}, {-1,2,-3,4}, {0.1,0.2,-0.3,0.4}, {-0.0,0,-0.0,0},
            {1e-100,-1e-100,0,0}, {1e100,1e100,-1e100,1e100},
            {Double.MIN_NORMAL,0,0,0}, {Double.MAX_VALUE,1,2,3},
            {Double.NaN,1,2,3}, {1,Double.POSITIVE_INFINITY,0,-0.0}
        };
    }
    static double[] epsilons() { return new double[]{-1,0,1e-15,1,Double.NaN}; }
    static void numeric() {
        double[][] values = inputs();
        for (int i=0; i<values.length; i++) {
            double[] v = values[i]; Quaternion q = Quaternion.of(v[0],v[1],v[2],v[3]);
            String k = "q." + i;
            out(k+".parts", () -> parts(q));
            out(k+".scalar", () -> bits(q.getScalarPart()));
            out(k+".vector", () -> {
                double[] a=q.getVectorPart();
                return bits(a[0])+","+bits(a[1])+","+bits(a[2]);
            });
            out(k+".conjugate", () -> parts(q.conjugate()));
            out(k+".norm", () -> bits(q.norm())); out(k+".normSq", () -> bits(q.normSq()));
            out(k+".normalize", () -> parts(q.normalize()));
            out(k+".polar", () -> parts(q.positivePolarForm()));
            out(k+".negate", () -> parts(q.negate())); out(k+".inverse", () -> parts(q.inverse()));
            out(k+".hash", () -> q.hashCode()); out(k+".string", () -> units(q.toString()));
            out(k+".equals.self", () -> q.equals(q));
            out(k+".equals.fresh", () -> q.equals(Quaternion.of(v[0],v[1],v[2],v[3])));
            out(k+".equals.null", () -> q.equals(null)); out(k+".equals.foreign", () -> q.equals("other"));
            out(k+".normalized.state", () -> {
                Quaternion n=q.normalize(); Quaternion p=n.positivePolarForm();
                return (q==n)+","+(n==n.normalize())+","+(p==p.normalize())+","+(p==p.positivePolarForm())
                    +","+bits(n.norm())+","+bits(n.normSq())+","+n.isUnit(-1)+","+n.isUnit(Double.NaN);
            });
            for (int e=0; e<epsilons().length; e++) {
                double eps=epsilons()[e]; String key=k+".eps."+e;
                out(key+".unit", () -> q.isUnit(eps)); out(key+".pure", () -> q.isPure(eps));
            }
            for (int j=0; j<values.length; j++) {
                double[] b=values[j]; Quaternion r=Quaternion.of(b[0],b[1],b[2],b[3]);
                String key=k+".binary."+j;
                out(key+".multiply.static", () -> parts(Quaternion.multiply(q,r)));
                out(key+".multiply", () -> parts(q.multiply(r)));
                out(key+".add.static", () -> parts(Quaternion.add(q,r))); out(key+".add", () -> parts(q.add(r)));
                out(key+".subtract.static", () -> parts(Quaternion.subtract(q,r))); out(key+".subtract", () -> parts(q.subtract(r)));
                out(key+".dot.static", () -> bits(Quaternion.dot(q,r))); out(key+".dot", () -> bits(q.dot(r)));
                out(key+".equals", () -> q.equals(r));
                for (int e=0; e<epsilons().length; e++) {
                    double eps=epsilons()[e]; out(key+".equals."+e, () -> q.equals(r,eps));
                }
            }
            double[] scales={0,-0.0,1,-1,0.5,2,1e-100,1e100,Double.POSITIVE_INFINITY,Double.NaN};
            for (int j=0; j<scales.length; j++) {
                double a=scales[j]; String key=k+".scalar."+j;
                out(key+".multiply", () -> parts(q.multiply(a))); out(key+".divide", () -> parts(q.divide(a)));
            }
            out(k+".after", () -> parts(q));
        }
    }
    static void arraysAndParsing() {
        for (int length=0; length<=4; length++) {
            double[] a=new double[length]; String k="factory.length."+length;
            out(k+".pure", () -> parts(Quaternion.of(a)));
            out(k+".scalar", () -> parts(Quaternion.of(1,a)));
        }
        out("factory.null.pure", () -> parts(Quaternion.of((double[])null)));
        out("factory.null.scalar", () -> parts(Quaternion.of(1,(double[])null)));
        out("factory.copy", () -> {
            double[] a={2,3,4}; Quaternion q=Quaternion.of(1,a); Quaternion r=Quaternion.of(a);
            a[0]=99; double[] b=q.getVectorPart(); b[1]=99;
            return parts(q)+";"+parts(r)+";"+(q.getVectorPart()!=q.getVectorPart());
        });
        String[] strings={"[1 2 3 4]","[-0.0 0.0 -0.0 0.0]","[NaN Infinity -Infinity NaN]",
            "[0x1p2 -0x1p-4 1f 2D]","[1e9999 1e-9999 1 2]","", "[1 2 3]","[1 2 3 4 5]",
            "(1 2 3 4]","[1 2 3 4)","[ 1 2 3 4]","[1  2 3 4]","[1 2 3 4 ]",
            "[a 2 3 4]","[1 a 3 4]","[1 2 a 4]","[1 2 3 a]","[\ud800 2 3 4]",null};
        for (int i=0; i<strings.length; i++) {
            String s=strings[i]; out("parse."+i, () -> parts(Quaternion.parse(s)));
        }
        Quaternion q=Quaternion.of(1,2,3,4);
        out("null.add", () -> parts(q.add((Quaternion)null)));
        out("null.subtract", () -> parts(q.subtract((Quaternion)null)));
        out("null.multiply", () -> parts(q.multiply((Quaternion)null)));
        out("null.dot", () -> bits(q.dot(null)));
        out("null.equals.eps", () -> q.equals(null,1e-10));
    }
    static void interpolations() {
        Quaternion[][] pairs={
            {Quaternion.ONE,Quaternion.I},
            {Quaternion.ONE,Quaternion.of(1,1e-5,0,0)},
            {Quaternion.I,Quaternion.of(0,-1,0,0)},
            {Quaternion.of(1,2,3,4),Quaternion.of(-2,3,-4,5)}
        };
        double[] ts={-0.5,-0.0,0,0.25,0.5,0.75,1,1.5};
        for (int i=0; i<pairs.length; i++) {
            Quaternion start=pairs[i][0].positivePolarForm(), end=pairs[i][1];
            Slerp interpolation=new Slerp(start,end); DoubleFunction<Quaternion> callback=interpolation;
            for (int j=0; j<ts.length; j++) {
                double t=ts[j]; String k="slerp."+i+"."+j;
                out(k+".apply", () -> parts(interpolation.apply(t)));
                out(k+".callback", () -> parts(callback.apply(t)));
                out(k+".repeat", () -> parts(interpolation.apply(t)));
                out(k+".startIdentity", () -> interpolation.apply(t)==start);
            }
            out("slerp."+i+".after", () -> parts(start)+";"+parts(end));
        }
        out("slerp.null.start", () -> parts(new Slerp(null,Quaternion.ONE).apply(0.5)));
        out("slerp.null.end", () -> parts(new Slerp(Quaternion.ONE,null).apply(0.5)));
        out("slerp.zero.start", () -> parts(new Slerp(Quaternion.ZERO,Quaternion.ONE).apply(0.5)));
        out("slerp.zero.end", () -> parts(new Slerp(Quaternion.ONE,Quaternion.ZERO).apply(0.5)));
    }
    public static void main(String[] args) {
        out("constant.ZERO", () -> parts(Quaternion.ZERO)); out("constant.ONE", () -> parts(Quaternion.ONE));
        out("constant.I", () -> parts(Quaternion.I)); out("constant.J", () -> parts(Quaternion.J));
        out("constant.K", () -> parts(Quaternion.K));
        out("constant.identity", () -> Quaternion.ZERO==Quaternion.ZERO && Quaternion.ONE==Quaternion.ONE
            && Quaternion.I==Quaternion.I && Quaternion.J==Quaternion.J && Quaternion.K==Quaternion.K);
        numeric(); arraysAndParsing(); interpolations(); System.out.println("oracle=ok");
    }
}
