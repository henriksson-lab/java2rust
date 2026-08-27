public class FinallyContinueOverridesBreak {
    public static String run() {
        StringBuilder sb = new StringBuilder();
        for (int i = 0; i < 3; i++) {
            try {
                sb.append("t");
                break;
            } finally {
                sb.append("f");
                continue;
            }
        }
        sb.append("a");
        return sb.toString();
    }

    public static void main(String[] args) {
        System.out.println(run());
    }
}
