public class AppendIntLocal {
    public static String run() {
        StringBuilder out = new StringBuilder();
        int i = 7;
        out.append(i);
        return out.toString();
    }
    public static void main(String[] args) { System.out.println(run()); }
}
