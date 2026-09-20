interface CollectionCounter {
    int add(int delta);
    int read();
}

public class AnonymousInterfaceCollection {
    static CollectionCounter[] counters = new CollectionCounter[1];

    static int arrayCase() {
        counters[0] = new CollectionCounter() {
            int total = 1;

            public int add(int delta) {
                this.total = this.total + delta;
                return this.total;
            }

            public int read() { return this.total; }
        };
        counters[0].add(3);
        return counters[0].read();
    }

    static int localListCase() {
        java.util.ArrayList<CollectionCounter> values = new java.util.ArrayList<>();
        values.add(new CollectionCounter() {
            int total = 1;

            public int add(int delta) {
                this.total = this.total + delta;
                return this.total;
            }

            public int read() { return this.total; }
        });
        values.get(0).add(3);
        return values.get(0).read();
    }

    public static void main(String[] args) {
        System.out.println(arrayCase());
        System.out.println(localListCase());
    }
}
