package nativeconsumerdestinations;
import java.util.function.DoubleConsumer;
public class NativeConsumerDestinationsAudit {
    interface Result { double value(); }
    interface Statistic extends Result, DoubleConsumer {}
    static final class Counter implements Statistic {
        int calls;
        double total;
        public void accept(double value) { calls++; total+=value; }
        public double value() { return total; }
    }
    private static final DoubleConsumer NOOP=new DoubleConsumer() {
        public void accept(double value) {}
        public DoubleConsumer andThen(DoubleConsumer after) { return after; }
    };
    static DoubleConsumer compose(DoubleConsumer... consumers) {
        DoubleConsumer action=NOOP;
        for(DoubleConsumer consumer:consumers) { if(consumer!=null) action=action.andThen(consumer); }
        return action==NOOP ? null : action;
    }
    static final class Holder {
        DoubleConsumer action;
        Holder(Statistic first,Statistic second) { action=compose(first,second); }
        void accept(double value) { action.accept(value); }
    }
    static int evaluations;
    static DoubleConsumer argument() { evaluations++; return NOOP; }
    public static int run() {
        Counter first=new Counter();
        Counter second=new Counter();
        Holder holder=new Holder(first,second);
        holder.accept(2);
        if(first.calls!=1 || second.calls!=1 || first.value()!=2 || second.value()!=2) return 1;
        if(compose()!=null || compose(NOOP)!=null || compose(null,NOOP,null)!=null) return 2;
        if(NOOP.andThen(null)!=null || compose(NOOP,first)!=first) return 3;
        DoubleConsumer[] array={first,null,second};
        DoubleConsumer composed=compose(array);
        array[0]=NOOP;
        composed.accept(3);
        if(first.calls!=2 || second.calls!=2 || first.value()!=5 || second.value()!=5) return 4;
        DoubleConsumer throwing=value->{throw new IllegalArgumentException("stop");};
        DoubleConsumer failing=compose(first,throwing,second);
        try { failing.accept(7); return 5; }
        catch(IllegalArgumentException expected) {
            if(first.calls!=3 || first.value()!=12 || second.calls!=2 || second.value()!=5) return 6;
        }
        try { first.andThen(null); return 7; }
        catch(NullPointerException expected) { if(first.calls!=3 || second.calls!=2) return 8; }
        Counter absent=null;
        try { absent.andThen(argument()); return 9; }
        catch(NullPointerException expected) { if(evaluations!=1) return 10; }
        first.andThen(second).accept(1);
        if(first.calls!=4 || second.calls!=3 || first.value()!=13 || second.value()!=6) return 11;
        return 0;
    }
    public static void main(String[] args) { System.out.println(run()); }
}
