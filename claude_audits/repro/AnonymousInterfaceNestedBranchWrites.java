interface BranchCounter {
    int update(int delta, boolean choose);
    int read();
}

public class AnonymousInterfaceNestedBranchWrites {
    public static void main(String[] args) {
        BranchCounter counter = new BranchCounter() {
            int first = 1;
            int second = 2;

            public int update(int delta, boolean choose) {
                int amount = delta + 1;
                if (choose) {
                    int next = this.first + amount;
                    this.first = next;
                } else {
                    int next = this.second + amount;
                    this.second = next;
                }
                int snapshot = this.first + this.second;
                return snapshot;
            }

            public int read() {
                return this.first + this.second;
            }
        };
        int before = counter.update(2, true);
        int after = counter.update(1, false);
        System.out.println(before * 100 + after * 10 + counter.read());
    }
}
