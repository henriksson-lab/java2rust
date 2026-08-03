/**
 * Java reference assignment creates a second name for one object. The
 * translation has no shared representation to lower that into, so it copies.
 *
 * Every line below is a case where the copy is observable. Java prints
 * "5|7|3|9"; the translation prints "0|0|0|0" once each struct derives Clone.
 */
public class Aliasing {
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

    public static String run() {
        return writeThroughOriginal()
                + "|"
                + writeThroughAlias()
                + "|"
                + writeThroughMutatingCall()
                + "|"
                + writeThroughArrayElement();
    }

    public static void main(String[] args) {
        System.out.println(run());
    }
}
