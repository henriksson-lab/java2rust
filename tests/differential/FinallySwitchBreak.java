public class FinallySwitchBreak {
    public static String run() {
        StringBuilder out = new StringBuilder();
        for (int i = 0; i < 2; i++) {
            try {
                out.append("z");
                switch (i) {
                    case 0:
                        out.append("f");
                        break;
                    default:
                        out.append("d");
                        break;
                }
                break;
            } finally {
                out.append("o");
            }
        }
        out.append("fdf");
        return out.toString();
    }

    public static void main(String[] args) {
        System.out.print(run());
    }
}
