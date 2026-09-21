public class ErasedObjectArrayElements {
    public static String run() {
        Object[] values = new Object[] { "x", 2, null };
        return values[0] + ":" + values[1] + ":" + values[2];
    }

    public static void main(String[] args) {
        System.out.println(run());
    }
}
