public class NullableUnboxArrayDimensionCatch {
    public static String run() {
        Integer size = null;
        try {
            int[] values = new int[size];
            return "len:" + values.length;
        } catch (NullPointerException ex) {
            return "npe";
        }
    }

    public static void main(String[] args) {
        System.out.print(run());
    }
}
