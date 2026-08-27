public class FinallyWhileContinue {
    public static String run() {
        StringBuilder sb = new StringBuilder();
        int i = 0;
        while (i < 3) {
            try {
                i++;
                if (i == 2) continue;
                sb.append(i);
            } finally {
                sb.append("f");
            }
        }
        return sb.toString();
    }

    public static void main(String[] args) {
        System.out.println(run());
    }
}
