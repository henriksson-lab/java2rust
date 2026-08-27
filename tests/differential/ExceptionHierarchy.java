public class ExceptionHierarchy {
    static String parse(String s) {
        try {
            return "v:" + Integer.parseInt(s);
        } catch (IllegalArgumentException e) {
            return "iae";
        }
    }
    public static String run() { return parse("12") + "," + parse("xy"); }
    public static void main(String[] args) { System.out.println(run()); }
}
