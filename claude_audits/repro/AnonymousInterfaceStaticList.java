interface StaticListCounter {
    int add(int delta);
    int read();
}

public class AnonymousInterfaceStaticList {
    static java.util.ArrayList<StaticListCounter> counters = new java.util.ArrayList<>();

    static int run() {
        counters.add(new StaticListCounter() {
            int total = 1;

            public int add(int delta) {
                this.total = this.total + delta;
                return this.total;
            }

            public int read() { return this.total; }
        });
        counters.add(counters.get(0));
        counters.add(0, counters.get(0));
        counters.get(2).add(3);
        return counters.get(1).read();
    }

    static int structural() {
        StaticListCounter displaced = counters.set(1, new StaticListCounter() {
            int total = 7;

            public int add(int delta) {
                this.total = this.total + delta;
                return this.total;
            }

            public int read() { return this.total; }
        });
        StaticListCounter removed = counters.remove(0);
        removed.add(3);
        counters.get(0).add(3);
        return counters.size() * 100 + counters.get(0).read() * 10 + displaced.read();
    }

    public static void main(String[] args) {
        System.out.println(run());
        System.out.println(structural());
    }
}
