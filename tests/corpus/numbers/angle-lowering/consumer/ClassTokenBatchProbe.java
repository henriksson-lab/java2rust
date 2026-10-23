package consumer;
import producer.Units;

public class ClassTokenBatchProbe {
    static String kind(Object value) { return value.getClass().getName(); }
    static Object echo(Object value) { return value; }
    public static int run() {
        Object zero = Units.Value.ZERO;
        Object fresh = Units.Value.of(4);
        if (!kind(zero).equals("producer.Units$Value") || !kind(fresh).equals(kind(zero))) return 1;
        if (!kind("other").equals("java.lang.String")) return 2;
        try { kind(null); return 3; } catch (NullPointerException expected) { }
        if (echo(zero) != zero || echo(fresh) != fresh) return 4;
        return 0;
    }
    public static void main(String[] args) { System.out.println(run()); }
}
