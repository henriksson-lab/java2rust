public class ControlFlow {
    public static String run() {
        StringBuilder out = new StringBuilder();
        for (int v = 1; v <= 4; v++) {
            String s = "";
            switch (v) {
                case 1: s = s + "one";
                case 2: s = s + "two"; break;
                case 3: s = s + "three";
                default: s = s + "other";
            }
            out.append(s).append(";");
        }
        int total = 0;
        outer:
        for (int i = 0; i < 3; i++) {
            for (int j = 0; j < 3; j++) {
                if (j == 1) continue outer;
                total += 1;
            }
        }
        return out.toString() + "|" + total;
    }
    public static void main(String[] args) { System.out.println(run()); }
}
