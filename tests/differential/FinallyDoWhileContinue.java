public class FinallyDoWhileContinue {
    public static String run() {
        StringBuilder sb = new StringBuilder();
        int i = 0;
        do {
            try {
                sb.append(i);
                i++;
                if (i < 2) {
                    continue;
                }
                sb.append("x");
            } finally {
                sb.append("f");
            }
        } while (i < 3);
        return sb.toString();
    }

    public static void main(String[] args) {
        System.out.print(run());
    }
}
