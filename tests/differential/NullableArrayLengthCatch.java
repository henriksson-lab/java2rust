public class NullableArrayLengthCatch {
    public static String run() {
        String[] labels = null;
        try {
            int n = labels.length;
            return "len:" + n;
        } catch (NullPointerException e) {
            return "npe";
        }
    }

    public static void main(String[] args) {
        System.out.print(run());
    }
}
