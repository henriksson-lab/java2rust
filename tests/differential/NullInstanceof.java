public class NullInstanceof {
    public static String run() {
        Object o = null;
        Object p = "text";
        boolean a = (o instanceof String);
        boolean b = (p instanceof String);
        return a + "," + b;
    }
    public static void main(String[] args) { System.out.println(run()); }
}
