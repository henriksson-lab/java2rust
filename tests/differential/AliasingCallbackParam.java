public class AliasingCallbackParam {
    static class Node {
        int v;
    }

    interface Action {
        void run();
    }

    static class Owner {
        int apply(Action action, Node node) {
            action.run();
            return node.v;
        }
    }

    static class NamedAction implements Action {
        Node node;

        NamedAction(Node node) {
            this.node = node;
        }

        public void run() {
            this.node.v = 17;
        }
    }

    public static String run() {
        Node a = new Node();
        Node b = a;
        Owner owner = new Owner();
        return String.valueOf(owner.apply(new NamedAction(a), b));
    }

    public static void main(String[] args) {
        System.out.println(run());
    }
}
