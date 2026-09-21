import java.util.Objects;

public class SourceMapEntryErasedThrow {
    static class Entry implements java.util.Map.Entry<String, Integer> {
        public String getKey() { return "k"; }
        public Integer getValue() { return 1; }
        public Integer setValue(Integer value) { return 1; }
        public String toString() { throw new IllegalStateException("bad entry"); }
    }

    static String viaConcat(Object object) {
        try { return "entry=" + object; }
        catch (IllegalStateException expected) { return "caught-concat"; }
    }

    static String viaMethod(Object object) {
        try { return object.toString(); }
        catch (IllegalStateException expected) { return "caught-method"; }
    }

    static String viaValueOf(Object object) {
        try { return String.valueOf(object); }
        catch (IllegalStateException expected) { return "caught-value"; }
    }

    static String viaObjects(Object object) {
        try { return Objects.toString(object); }
        catch (IllegalStateException expected) { return "caught-objects"; }
    }

    public static String run() {
        java.util.Map.Entry<String, Integer> entry = new Entry();
        Object object = entry;
        Object direct = new Entry();
        return viaConcat(object) + ":" + viaMethod(object)
                + ":" + viaValueOf(object) + ":" + viaObjects(object)
                + ":" + viaConcat(direct) + ":" + viaMethod(direct);
    }

    public static void main(String[] args) { System.out.println(run()); }
}
