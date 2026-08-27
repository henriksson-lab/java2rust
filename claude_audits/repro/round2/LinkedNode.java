public class LinkedNode {
    static class Node {
        int value;
        Node next;
        Node(int v) { this.value = v; }
    }
    public static String run() {
        Node a = new Node(1);
        Node b = new Node(2);
        a.next = b;
        int total = 0;
        Node cur = a;
        while (cur != null) { total += cur.value; cur = cur.next; }
        return total + "," + (b.next == null);
    }
    public static void main(String[] args) { System.out.println(run()); }
}
