public class StringValueOfNullObject {
    static String label(Object value) {
        return String.valueOf(value);
    }

    public static String run() {
        return label(null);
    }

    public static void main(String[] args) {
        System.out.print(run());
    }
}
