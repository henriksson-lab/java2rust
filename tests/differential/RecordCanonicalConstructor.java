public class RecordCanonicalConstructor {
    record Pair(int x, String y) {
    }

    record Normalized(int x, String y) {
        Normalized {
            x = x + 1;
            y = y + "!";
        }
    }

    record WithAuxiliary(int x) {
        WithAuxiliary() {
            this(11);
        }
    }

    public static String run() {
        Pair pair = new Pair(7, "q");
        Normalized normalized = new Normalized(7, "r");
        WithAuxiliary auxiliary = new WithAuxiliary(5);
        return pair.x() + ":" + pair.y() + ":" + normalized.x() + ":" + normalized.y()
                + ":" + auxiliary.x();
    }

    public static void main(String[] args) {
        System.out.print(run());
    }
}
