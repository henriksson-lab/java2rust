public class TrimInReturn {
    public static String run() {
        String s = "  padded  ";
        return s.trim();
    }
    public static void main(String[] args) { System.out.println(run()); }
}
