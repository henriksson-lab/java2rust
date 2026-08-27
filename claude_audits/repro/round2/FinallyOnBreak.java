public class FinallyOnBreak {
    public static String run() {
        StringBuilder sb = new StringBuilder();
        for (int i = 0; i < 3; i++) {
            try { if (i == 1) break; sb.append("b"); }
            finally { sb.append("f"); }
        }
        return sb.toString();
    }
    public static void main(String[] args) { System.out.println(run()); }
}
