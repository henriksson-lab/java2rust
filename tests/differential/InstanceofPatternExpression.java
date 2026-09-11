public class InstanceofPatternExpression {
    static int ternary(Object o) {
        return (o instanceof String s) ? s.length() : -1;
    }

    static boolean and(Object o) {
        return o instanceof String s && s.isEmpty();
    }

    static String nested(Object o) {
        return (o instanceof String s && s.length() > 2) ? s.substring(1) : "no";
    }

    public static String run() {
        return ternary("abcd") + "," + ternary(Integer.valueOf(1)) + "," +
                and("") + "," + and("x") + "," +
                nested("abcd") + "," + nested("x");
    }

    public static void main(String[] args) {
        System.out.print(run());
    }
}
