interface BranchSnapshot {
    int update(int delta, boolean choose);
    int read();
}

public class AnonymousInterfaceNestedOuterLocal {
    public static void main(String[] args) {
        BranchSnapshot counter = new BranchSnapshot() {
            int first = 1;
            int second = 2;

            public int update(int delta, boolean choose) {
                int snapshot = this.first + this.second;
                if (choose) {
                    snapshot = snapshot + delta;
                    this.first = snapshot;
                } else {
                    snapshot = snapshot - delta;
                    this.second = snapshot;
                }
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
