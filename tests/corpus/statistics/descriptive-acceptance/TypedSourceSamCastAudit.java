package typedsourcesamcast;
import java.util.function.DoubleSupplier;
import java.util.function.IntSupplier;
import java.util.function.LongSupplier;
public class TypedSourceSamCastAudit {
    enum Choice { FIRST, SECOND }
    static int choice(Choice value) {
        if (value == Choice.FIRST) return 1;
        if (Choice.SECOND == value) return 2;
        if (value != Choice.SECOND) return 3;
        return 4;
    }
    interface Result extends DoubleSupplier,IntSupplier,LongSupplier {
        default int getAsInt() { return (int)getAsDouble(); }
        default long getAsLong() { return (long)getAsDouble(); }
    }
    interface IntView extends Result {
        int getAsInt();
        default double getAsDouble() { return getAsInt(); }
        default long getAsLong() { return getAsInt(); }
    }
    interface LongView extends Result {
        long getAsLong();
        default double getAsDouble() { return getAsLong(); }
        default int getAsInt() { return (int)getAsLong(); }
    }
    interface Value<T> { T get(); default T same() { return get(); } }
    interface ChildValue<T> extends Value<T> {}
    static final class Counter implements Result {
        int value;
        boolean fail;
        Counter(int value) { this.value=value; }
        public double getAsDouble() {
            if(fail) throw new IllegalStateException("result");
            return value;
        }
    }
    static int evaluations;
    static Result selected(Result result) { evaluations++; return result; }
    static Result asIntOrNull(Result result) {
        if(result!=null) return (IntView) result::getAsInt;
        return null;
    }
    static Result asLong(Result result) { return (LongView) result::getAsLong; }
    static Result asSelectedInt(Result result) { return (IntView) selected(result)::getAsInt; }
    static Result constant(long value) { return (LongView) ()->value; }
    static Result lazy(Result result) { return (IntView) ()->result.getAsInt(); }
    static Value<Counter> value(Counter counter) { return (ChildValue<Counter>) ()->counter; }
    public static int run() {
        Counter source=new Counter(13);
        Result first=asIntOrNull(source);
        Result alias=first;
        if(first!=alias || first==source || first.getAsInt()!=13 || first.getAsLong()!=13 || first.getAsDouble()!=13) return 1;
        source.value=21;
        if(alias.getAsInt()!=21 || alias.getAsDouble()!=21) return 2;
        Result longView=asLong(source);
        source.value=27;
        if(longView.getAsInt()!=27 || longView.getAsLong()!=27 || longView.getAsDouble()!=27) return 3;
        Counter other=new Counter(7);
        Result otherView=asIntOrNull(other);
        if(otherView==first || otherView.getAsInt()!=7 || first.getAsInt()!=27) return 4;
        Result selected=asSelectedInt(source);
        if(evaluations!=1 || selected.getAsInt()!=27 || selected.getAsInt()!=27 || evaluations!=1) return 5;
        if(asIntOrNull(null)!=null) return 6;
        try { asLong(null); return 7; }
        catch(NullPointerException expected) { if(source.value!=27) return 8; }
        Result constant=constant(3000000001L);
        if(constant.getAsLong()!=3000000001L || constant.getAsDouble()!=3000000001.0 || constant.getAsInt()!=(int)3000000001L) return 9;
        source.fail=true;
        try { first.getAsInt(); return 10; }
        catch(IllegalStateException expected) { source.fail=false; }
        if(first.getAsInt()!=27 || otherView.getAsInt()!=7) return 11;
        Result lazy=lazy(source);
        Result lazyAlias=lazy;
        source.value=31;
        if(lazy!=lazyAlias || lazy==source || lazy.getAsInt()!=31 || first.getAsInt()!=31) return 12;
        Result lazyNull=lazy(null);
        try { lazyNull.getAsInt(); return 13; }
        catch(NullPointerException expected) { if(source.value!=31) return 14; }
        Value<Counter> retained=value(source);
        if(retained.get()!=source || retained.same()!=source) return 15;
        retained.get().value=37;
        if(source.value!=37 || lazy.getAsInt()!=37 || first.getAsInt()!=37) return 16;
        Value<Counter> nil=value(null);
        if(nil.get()!=null || nil.same()!=null) return 17;
        if(choice(null)!=3 || choice(Choice.FIRST)!=1 || choice(Choice.SECOND)!=2) return 18;
        return 0;
    }
    public static void main(String[] args) { System.out.println(run()); }
}
