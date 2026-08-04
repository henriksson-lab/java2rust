public class InterfaceOverloads {
    interface Scorer {
        int score(int a);

        int score(int a, int b);
    }

    static class BaseScorer {
        public int score(int a, int b) {
            return score(a) + score(b);
        }

        public int score(int a) {
            return a;
        }
    }

    static class Doubling extends BaseScorer implements Scorer {
        @Override
        public int score(int a) {
            return a * 2;
        }
    }

    static class Tripling extends BaseScorer implements Scorer {
        @Override
        public int score(int a) {
            return a * 3;
        }
    }

    static int viaInterface(Scorer scorer, int a, int b) {
        return scorer.score(a) + scorer.score(a, b);
    }

    public static String run() {
        Doubling doubling = new Doubling();
        Tripling tripling = new Tripling();
        return doubling.score(5)
                + "|"
                + doubling.score(5, 10)
                + "|"
                + tripling.score(7) / 3
                + "|"
                + tripling.score(3, 4)
                + "|"
                + viaInterface(new Doubling(), 10, 40);
    }

    public static void main(String[] args) {
        System.out.println(run());
    }
}
