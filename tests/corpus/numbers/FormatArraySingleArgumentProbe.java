public class FormatArraySingleArgumentProbe {
    static String primitives(int[] values) { return String.format("%s", values); }
    static String references(String[] values) { return String.format("%s", (Object) values); }
    static int run() {
        if (!primitives(new int[]{7, 8}).startsWith("[I@")) return 1;
        if (!references(new String[]{"first", "second"}).startsWith("[Ljava.lang.String;@")) return 2;
        return 0;
    }
    public static void main(String[] args) { System.out.println(run()); }
}
