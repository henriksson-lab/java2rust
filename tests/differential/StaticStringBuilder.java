public class StaticStringBuilder {
    static StringBuilder log = new StringBuilder();
    public static String run() {
        log.append("a");
        log.append("b");
        return log.toString();
    }
    public static void main(String[] args) { System.out.println(run()); }
}
