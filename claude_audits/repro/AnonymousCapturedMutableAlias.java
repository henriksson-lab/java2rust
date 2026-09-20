// The anonymous object must retain Node identity, not a copied Node value.
interface Reader {
    int read();
}

class Node {
    int value;
    Node(int value) { this.value = value; }
}

public class AnonymousCapturedMutableAlias {
    static Reader make(Node node) {
        return new Reader() {
            public int read() { return node.value; }
        };
    }

    public static int run() {
        Node node = new Node(3);
        Reader reader = make(node);
        node.value = 8;
        return reader.read();
    }
}
