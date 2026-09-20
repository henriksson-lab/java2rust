interface ArrayCounter { int sum(int[] values); int read(); }

public class AnonymousInterfaceEnhancedFor {
    public static void main(String[] args) {
        ArrayCounter counter = new ArrayCounter() {
            int total = 0;

            public int sum(int[] values) {
                outer: for (int value : values) {
                    if (value == 2) continue outer;
                    this.total = this.total + value;
                    if (value == 3) break outer;
                }
                return this.total;
            }

            public int read() { return this.total; }
        };
        int[] first = {1, 2, 3, 4};
        int[] second = {5};
        System.out.println(counter.sum(first) * 100 + counter.sum(second) * 10 + counter.read());
    }
}
