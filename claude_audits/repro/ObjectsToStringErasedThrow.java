import java.util.Objects;

public class ObjectsToStringErasedThrow {
    static class Value {
        public String toString() { throw new IllegalStateException("bad value"); }
    }

    static String viaObjects(Object value) {
        try { return Objects.toString(value); }
        catch (IllegalStateException expected) { return "caught-objects"; }
    }

    static String viaMethod(Object value) {
        try { return value.toString(); }
        catch (IllegalStateException expected) { return "caught-method"; }
    }

    public static String run() {
        Object value = new Value();
        return viaObjects(value) + ":" + viaMethod(value)
                + ":" + Integer.toString(7) + ":" + Double.toString(1.5);
    }

    public static void main(String[] args) { System.out.println(run()); }
}
