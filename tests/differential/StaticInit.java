public class StaticInit {
    static int counter = 0;
    static { counter = 5; }
    static int derived = counter * 2;
    static { derived = derived + 1; }
    public static String run() { return counter + "," + derived; }
    public static void main(String[] args) { System.out.println(run()); }
}
