package checkedfill;
import java.util.Arrays;
import java.util.function.IntToDoubleFunction;
public class CheckedFillAudit {
    static double[] current;
    static int trace;
    static boolean fail;
    static double[][] rectangular;
    static double rowValue() { trace=trace*10+5; rectangular[1][0]=11; return 6; }
    static double[] selected() { trace=trace*10+1; return current; }
    static int from() { trace=trace*10+2; return 0; }
    static int to() { trace=trace*10+3; return 2; }
    static double sample(int ignored) {
        trace=trace*10+4;
        if(current!=null && current.length>2) current[2]=9;
        if(fail) throw new IllegalStateException("value");
        return 7;
    }
    static void fill(double[] values, boolean empty, IntToDoubleFunction callback) {
        Arrays.fill(values, empty ? Double.NaN : callback.applyAsDouble(0));
    }
    public static int run() {
        current=new double[] {1,2,3};
        double[] alias=current;
        trace=0;
        Arrays.fill(selected(), false ? 0 : sample(0));
        if(trace!=14 || alias[0]!=7 || alias[2]!=7) return 1;
        trace=0; alias[0]=1; alias[1]=2;
        Arrays.fill(selected(), from(), to(), sample(0));
        if(trace!=1234 || alias[0]!=7 || alias[1]!=7 || alias[2]!=9) return 2;
        trace=0; fail=true;
        try { Arrays.fill(selected(), from(), to(), sample(0)); return 3; }
        catch(IllegalStateException expected) { if(trace!=1234 || alias[2]!=9) return 4; }
        fail=false;
        current=null; trace=0;
        try { Arrays.fill(selected(), sample(0)); return 5; }
        catch(NullPointerException expected) { if(trace!=14) return 6; }
        trace=0; fail=true;
        try { Arrays.fill(selected(), sample(0)); return 7; }
        catch(IllegalStateException expected) { if(trace!=14) return 8; }
        fail=false; trace=0;
        try { Arrays.fill(selected(), from(), to(), sample(0)); return 9; }
        catch(NullPointerException expected) { if(trace!=1234) return 10; }
        current=alias; trace=0;
        try { Arrays.fill(selected(), 3, 1, sample(0)); return 11; }
        catch(IllegalArgumentException expected) { if(trace!=14) return 12; }
        trace=0; alias[0]=1;
        try { Arrays.fill(selected(),0,9,sample(0)); return 18; }
        catch(ArrayIndexOutOfBoundsException expected) { if(trace!=14 || alias[0]!=1) return 19; }
        trace=0;
        fill(alias,false,CheckedFillAudit::sample);
        if(trace!=4 || alias[0]!=7 || alias[2]!=7) return 13;
        trace=0;
        fill(alias,true,null);
        if(trace!=0 || !Double.isNaN(alias[0]) || !Double.isNaN(alias[2])) return 14;
        trace=0;
        fill(new double[0],false,CheckedFillAudit::sample);
        if(trace!=4) return 15;
        trace=0;
        try { fill(null,false,CheckedFillAudit::sample); return 16; }
        catch(NullPointerException expected) { if(trace!=4) return 17; }
        rectangular=new double[2][2]; trace=0;
        Arrays.fill(rectangular[0],rowValue());
        if(trace!=5 || rectangular[0][0]!=6 || rectangular[0][1]!=6 || rectangular[1][0]!=11) return 20;
        trace=0;
        try { Arrays.fill(rectangular[2],rowValue()); return 21; }
        catch(ArrayIndexOutOfBoundsException expected) { if(trace!=0) return 22; }
        return 0;
    }
    public static void main(String[] args) { System.out.println(run()); }
}
