package genericoverloads;
import java.util.function.DoubleConsumer;
import java.util.function.IntConsumer;
import java.util.function.LongConsumer;
public class Feed {
    public static <T extends DoubleConsumer> T add(T s, double[] v) { for(double x:v) s.accept(x); return s; }
    public static <T extends DoubleConsumer> T add(T s, int[] v) { for(int x:v) s.accept(x); return s; }
    public static <T extends DoubleConsumer> T add(T s, long[] v) { for(long x:v) s.accept(x); return s; }
    public static <T extends IntConsumer> T add(T s, int[] v) { for(int x:v) s.accept(x); return s; }
    public static <T extends LongConsumer> T add(T s, long[] v) { for(long x:v) s.accept(x); return s; }
    public static <T extends DoubleConsumer> T add(T s, int[] v, int from, int to) {
        if(from<0 || to>v.length || from>to) throw new IndexOutOfBoundsException("range");
        for(int i=from;i<to;i++) s.accept(v[i]); return s;
    }
    public static <T extends IntConsumer> T add(T s, int[] v, int from, int to) {
        if(from<0 || to>v.length || from>to) throw new IndexOutOfBoundsException("range");
        for(int i=from;i<to;i++) s.accept(v[i]); return s;
    }
}
