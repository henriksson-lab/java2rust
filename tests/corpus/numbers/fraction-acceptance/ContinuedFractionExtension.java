import org.apache.commons.numbers.fraction.ContinuedFraction;

/** Source extension only: the continued-fraction algorithm stays upstream. */
public final class ContinuedFractionExtension extends ContinuedFraction {
    private final int mode;
    private int calls;
    private int trace;
    private double factor=1.0;
    public ContinuedFractionExtension(int mode) { this.mode=mode; }
    public static ContinuedFractionExtension create(int mode) { return new ContinuedFractionExtension(mode); }
    public int getCalls() { return calls; }
    public int getTrace() { return trace; }
    public void setFactor(double factor) { this.factor=factor; }
    private void record(int n,int kind) {
        calls++;
        trace=31*trace+2*n+kind;
        if (calls>256) throw new IllegalStateException("coefficient budget");
    }
    @Override protected double getA(int n,double x) {
        record(n,0);
        if (mode==2 && n==3) throw new IllegalStateException("coefficient stop");
        if (mode==3) return Double.NaN;
        return mode==1 ? n*factor : factor;
    }
    @Override protected double getB(int n,double x) {
        record(n,1);
        if (mode==5) return 0.0;
        if (n==0) return x*factor;
        if (mode==4) return Double.POSITIVE_INFINITY;
        return mode==1 ? n+1.0+x : 1.0;
    }
    public double run(double x,double epsilon,int limit,boolean defaultLimit) {
        ContinuedFraction base=this;
        return defaultLimit ? base.evaluate(x,epsilon) : base.evaluate(x,epsilon,limit);
    }
}
