public class FinallyPaths {
    static int throwing() { throw new RuntimeException("boom"); }
    static String finallyOnThrow() {
        StringBuilder sb = new StringBuilder();
        try {
            sb.append("t");
            throwing();
            sb.append("unreached");
        } catch (RuntimeException e) {
            sb.append("c");
        } finally {
            sb.append("f");
        }
        sb.append("a");
        return sb.toString();
    }
    static String finallyOnReturn() {
        StringBuilder sb = new StringBuilder();
        try { sb.append("t"); return sb.toString(); }
        finally { sb.append("f"); }
    }
    static String uncaughtWithFinally() {
        StringBuilder sb = new StringBuilder();
        try {
            try { sb.append("t"); throwing(); }
            finally { sb.append("f"); }
        } catch (RuntimeException e) { sb.append("outer"); }
        return sb.toString();
    }
    public static String run() {
        return finallyOnThrow() + "," + finallyOnReturn() + "," + uncaughtWithFinally();
    }
    public static void main(String[] args) { System.out.println(run()); }
}
