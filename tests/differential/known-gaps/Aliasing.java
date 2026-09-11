/**
 * Java reference assignment creates a second name for one object. The
 * translation has no shared representation to lower that into, so it copies.
 *
 * Every line below is a case where the copy is observable. Java prints
 * "5|7|3|9|11|13|17|19"; the translation computes stale values once each struct
 * derives Clone.
 */
public class Aliasing {
    static class Node {
        int v;
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

    static class Counter {
        int v;

        void bump() {
            this.v = this.v + 1;
        }

        int get() {
            return this.v;
        }
    }

    interface Action {
        void run();
    }

    static class BlockLambdaOwner {
        int apply(Action action, Node node) {
            action.run();
            return node.v;
        }
    }

    static class StoredRunnable implements Action {
        Node node;

        StoredRunnable(Node node) {
            this.node = node;
        }

        public void run() {
            this.node.v = 19;
        }
    }

    /** Write through the original, read through the alias. */
    static int writeThroughOriginal() {
        Node a = new Node();
        Node b = a;
        a.v = 5;
        return b.v;
    }

    /** Write through the alias, read through the original. */
    static int writeThroughAlias() {
        Node a = new Node();
        Node b = a;
        b.v = 7;
        return a.v;
    }

    /** The write is a method that mutates its receiver, not a field store. */
    static int writeThroughMutatingCall() {
        Counter a = new Counter();
        Counter b = a;
        a.bump();
        a.bump();
        a.bump();
        return b.get();
    }

    /** An alias stored in an array element rather than a second local. */
    static int writeThroughArrayElement() {
        Node a = new Node();
        Node[] cell = new Node[1];
        cell[0] = a;
        a.v = 9;
        return cell[0].v;
    }

    /** An alias stored through another object's field. */
    static int writeThroughHolderField() {
        Node a = new Node();
        Holder holder = new Holder();
        holder.stored = a;
        a.v = 11;
        return holder.stored.v;
    }

    /** The same field-storage alias through a this-field target. */
    static int writeThroughThisField() {
        Node a = new Node();
        HasField owner = new HasField();
        return owner.storeAndRead(a);
    }

    /** A block-bodied anonymous action mutates an alias captured from the outer scope. */
    static int writeThroughBlockAction() {
        Node a = new Node();
        Node b = a;
        BlockLambdaOwner owner = new BlockLambdaOwner();
        return owner.apply(new Action() {
            public void run() {
                a.v = 17;
            }
        }, b);
    }

    /** A stored Runnable-style implementation mutates the same object through its field. */
    static int writeThroughStoredRunnable() {
        Node a = new Node();
        Node b = a;
        Action action = new StoredRunnable(a);
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
                + writeThroughBlockAction()
                + "|"
                + writeThroughStoredRunnable();
    }

    public static void main(String[] args) {
        System.out.println(run());
    }
}
