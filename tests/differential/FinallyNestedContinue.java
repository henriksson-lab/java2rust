public class FinallyNestedContinue {
    public static String run() {
        StringBuilder sb = new StringBuilder();
        for (int i = 0; i < 3; i++) {
            try {
                try {
                    if (i == 1) continue;
                    sb.append(i);
                } finally {
                    sb.append("i");
                }
            } finally {
                sb.append("o");
            }
        }
        return sb.toString();
    }

    public static void main(String[] args) {
        System.out.println(run());
    }
}
