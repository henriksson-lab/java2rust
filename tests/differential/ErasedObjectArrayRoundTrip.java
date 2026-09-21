public class ErasedObjectArrayRoundTrip {
    static Object[] identity(Object[] values) {
        return values;
    }

    public static String run() {
        Object[] values = new Object[] { "x", 2, null };
        Object[] returned = identity(values);
        return values.length + ":" + returned.length + ":" + returned[0] + ":" + returned[2];
    }

    public static void main(String[] args) {
        System.out.println(run());
    }
}
