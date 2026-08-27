public class StringValueOfObject {
    static class P {
        public String toString() {
            return "P6";
        }
    }

    static String label(Object value) {
        return String.valueOf(value);
    }

    public static String run() {
        return label(new P());
    }

    public static void main(String[] args) {
        System.out.println(run());
    }
}
