public class NullDefaultReturn {
    static String describe(String s) {
        if (s == null) return "none";
        return s;
    }
    public static String run() {
        return describe("x") + "," + describe(null);
    }
    public static void main(String[] args) { System.out.println(run()); }
}
