public class NullableUnboxCatch {
    public static String run() {
        Integer value = null;
        try {
            int primitive = value;
            return "ok:" + primitive;
        } catch (NullPointerException ex) {
            return "npe";
        }
    }

    public static void main(String[] args) {
        System.out.print(run());
    }
}
