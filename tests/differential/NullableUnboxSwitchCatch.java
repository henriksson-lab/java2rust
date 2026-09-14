public class NullableUnboxSwitchCatch {
    static Integer value() {
        return null;
    }

    public static String run() {
        try {
            switch (value()) {
                case 1:
                    return "one";
                default:
                    return "other";
            }
        } catch (NullPointerException ex) {
            return "npe";
        }
    }

    public static void main(String[] args) {
        System.out.print(run());
    }
}
