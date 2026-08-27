public class FinallyLabeledContinue {
    public static String run() {
        StringBuilder sb = new StringBuilder();
        outer:
        for (int i = 0; i < 3; i++) {
            for (int j = 0; j < 3; j++) {
                try {
                    if (i == 1 && j == 0) continue outer;
                    sb.append(i);
                    sb.append(j);
                } finally {
                    sb.append("f");
                }
            }
        }
        return sb.toString();
    }

    public static void main(String[] args) {
        System.out.println(run());
    }
}
