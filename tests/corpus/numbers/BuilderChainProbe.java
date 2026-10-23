/** A real formatting idiom: preserve allocation, argument order and UTF-16. */
public class BuilderChainProbe {
    double w=1, x=2, y=3, z=4;
    String text() {
        StringBuilder b = new StringBuilder();
        b.append("[").append(w).append(" ").append(x).append(" ").append(y).append(" ").append(z).append("]");
        return b.toString();
    }
    public static int run() {
        if (!new BuilderChainProbe().text().equals("[1.0 2.0 3.0 4.0]")) return 1;
        StringBuilder b = new StringBuilder();
        StringBuilder alias = b;
        if (b.append("a").append(1).append('\ud800') != alias) return 2;
        if (b.length() != 3 || b.charAt(2) != '\ud800') return 3;
        return 0;
    }
    public static void main(String[] args) { System.out.println(run()); }
}
