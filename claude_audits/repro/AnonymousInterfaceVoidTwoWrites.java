interface PairAction {
    void step(int delta);
    int read();
}

public class AnonymousInterfaceVoidTwoWrites {
    public static void main(String[] args) {
        PairAction action = new PairAction() {
            int first = 1;
            int second = 2;

            public void step(int delta) {
                int next = this.first + delta;
                this.first = next;
                this.second = this.second + next;
            }

            public int read() {
                return this.first + this.second;
            }
        };
        action.step(3);
        System.out.println(action.read());
    }
}
