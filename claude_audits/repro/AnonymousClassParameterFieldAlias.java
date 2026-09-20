class ParameterBranchNode {
    int value;

    ParameterBranchNode(int value) { this.value = value; }
    int read() { return value; }
}

class ParameterBranchBase {
    ParameterBranchBase() {}
    int value() { return 0; }
    boolean same() { return false; }
}

public class AnonymousClassParameterFieldAlias {
    static ParameterBranchBase make(ParameterBranchNode original, boolean replace) {
        return new ParameterBranchBase() {
            ParameterBranchNode node = original;

            @Override int value() {
                if (replace) { this.node = new ParameterBranchNode(7); }
                return this.node.read();
            }
            @Override boolean same() { return this.node == original; }
        };
    }

    public static void main(String[] args) {
        ParameterBranchNode original = new ParameterBranchNode(3);
        ParameterBranchBase first = make(original, true);
        ParameterBranchBase second = make(original, false);
        if (!first.same() || !second.same()) {
            throw new AssertionError("initial handles do not match");
        }
        int firstValue = first.value();
        if (first.same() || !second.same()) {
            throw new AssertionError("replacement changed another handle");
        }
        System.out.println(firstValue * 100 + second.value() * 10 + original.read());
    }
}
