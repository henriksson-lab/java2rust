public class AliasingLocalField {
    static class Node {
        int v;
    }

    static class Counter {
        int v;

        void bump() {
            this.v = this.v + 1;
        }

        int get() {
            return this.v;
        }
    }

    static class Holder {
        Node stored;
    }

    static class HasField {
        Node stored;

        int storeAndRead(Node node) {
            this.stored = node;
            node.v = 13;
            return this.stored.v;
        }
    }

    static class StoredRunnable {
        Node node;

        StoredRunnable(Node node) {
            this.node = node;
        }

        void run() {
            this.node.v = 19;
        }
    }

    static int writeThroughOriginal() {
        Node a = new Node();
        Node b = a;
        a.v = 5;
        return b.v;
    }

    static int writeThroughAlias() {
        Node a = new Node();
        Node b = a;
        b.v = 7;
        return a.v;
    }

    static int writeThroughMutatingCall() {
        Counter a = new Counter();
        Counter b = a;
        a.bump();
        a.bump();
        a.bump();
        return b.get();
    }

    static int writeThroughArrayElement() {
        Node a = new Node();
        Node[] cell = new Node[1];
        cell[0] = a;
        a.v = 9;
        return cell[0].v;
    }

    static int writeThroughHolderField() {
        Node a = new Node();
        Holder holder = new Holder();
        holder.stored = a;
        a.v = 11;
        return holder.stored.v;
    }

    static int writeThroughThisField() {
        Node a = new Node();
        HasField owner = new HasField();
        return owner.storeAndRead(a);
    }

    static int writeThroughStoredRunnable() {
        Node a = new Node();
        Node b = a;
        StoredRunnable action = new StoredRunnable(a);
        action.run();
        return b.v;
    }

    public static String run() {
        return writeThroughOriginal()
                + "|"
                + writeThroughAlias()
                + "|"
                + writeThroughMutatingCall()
                + "|"
                + writeThroughArrayElement()
                + "|"
                + writeThroughHolderField()
                + "|"
                + writeThroughThisField()
                + "|"
                + writeThroughStoredRunnable();
    }

    public static void main(String[] args) {
        System.out.println(run());
    }
}
