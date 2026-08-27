public class NullCallArgCatch {
    static String maybe() {
        return null;
    }

    static int len(String s) {
        return s.length();
    }

    public static String run() {
        try {
            String value = maybe();
            return "ok:" + len(value);
        } catch (NullPointerException e) {
            return "npe";
        }
    }

    public static void main(String[] args) {
        System.out.println(run());
    }
}
