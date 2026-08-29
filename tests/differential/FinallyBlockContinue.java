public class FinallyBlockContinue {
    public static String run() {
        StringBuilder sb = new StringBuilder();
        for (int i = 0; i < 2; i++) {
            try {
                sb.append("t");
                {
                    continue;
                }
            } finally {
                sb.append("f");
            }
        }
        return sb.toString();
    }

    public static void main(String[] args) {
        System.out.print(run());
    }
}
