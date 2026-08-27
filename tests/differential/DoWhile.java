public class DoWhile {
    public static String run() {
        StringBuilder out = new StringBuilder();
        int n = 0;
        do { out.append("d").append(n); n++; } while (n < 2);
        int m = 10;
        do { out.append("z"); m++; } while (m < 3);
        return out.toString();
    }
    public static void main(String[] args) { System.out.println(run()); }
}
