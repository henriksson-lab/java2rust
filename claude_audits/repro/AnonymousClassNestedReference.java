class NestedBranchNode {
    int value;

    NestedBranchNode(int value) { this.value = value; }
    int read() { return value; }
}

class NestedBranchBase {
    NestedBranchBase() {}
    int value(boolean replace) { return 0; }
}

public class AnonymousClassNestedReference {
    public static void main(String[] args) {
        NestedBranchNode original = new NestedBranchNode(3);
        NestedBranchBase branch = new NestedBranchBase() {
            NestedBranchNode node = original;

            @Override int value(boolean replace) {
                if (replace) { this.node = new NestedBranchNode(7); }
                return this.node.read();
            }
        };
        System.out.println(branch.value(false) * 100
            + branch.value(true) * 10 + original.read());
    }
}
