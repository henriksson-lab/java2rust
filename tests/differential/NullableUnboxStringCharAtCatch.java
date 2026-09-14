public class NullableUnboxStringCharAtCatch {
    public static String run() {
        Integer index = null;
        try {
            return "" + "abc".charAt(index);
        } catch (NullPointerException ex) {
            return "npe";
        }
    }

    public static void main(String[] args) {
        System.out.print(run());
    }
}
