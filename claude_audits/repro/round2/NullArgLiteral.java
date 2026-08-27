public class NullArgLiteral {
    static String f(String s) {
        if (s == null) { return "N"; }
        return "v" + s;
    }
    public static String run() {
        return f("a") + "," + f(null);
    }
    public static void main(String[] args) { System.out.println(run()); }
}
