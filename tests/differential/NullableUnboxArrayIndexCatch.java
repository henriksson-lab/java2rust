public class NullableUnboxArrayIndexCatch {
    public static String run() {
        Integer value = null;
        int[] values = new int[] { 7 };
        try {
            return "value:" + values[value];
        } catch (NullPointerException ex) {
            return "npe";
        }
    }

    public static void main(String[] args) {
        System.out.print(run());
    }
}
