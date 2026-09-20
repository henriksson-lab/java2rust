interface CapturedPairOps {
    int first();
    int second(int input);
}

interface CapturedCounter {
    int add(int delta);
}

public class AnonymousObjectCapturePaths {
    static CapturedPairOps pair(int bonus) {
        return new CapturedPairOps() {
            public int first() { return bonus; }
            public int second(int input) { return bonus + input; }
        };
    }

    static CapturedCounter counter(int bonus) {
        return new CapturedCounter() {
            int total = 0;
            public int add(int delta) {
                this.total = total + delta + bonus;
                return total;
            }
        };
    }

    static int aliased() {
        CapturedCounter first = counter(2);
        CapturedCounter second = first;
        first.add(1);
        return second.add(2);
    }

    public static void main(String[] args) {
        CapturedPairOps pair = pair(5);
        System.out.println(pair.first() + pair.second(4));
        CapturedCounter counter = counter(2);
        System.out.println(counter.add(1));
        System.out.println(counter.add(2));
        System.out.println(aliased());
    }
}
