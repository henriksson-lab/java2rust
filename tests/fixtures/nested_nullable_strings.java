class NestedTextOwner<V> {
    class Node {
        V vertex;
        Node[] neighbors;
        Node parent;
        public String toString() { return "node"; }
        String toString(boolean full) {
            if (full) return toString();
            return vertex.toString();
        }
        String adjacentText() {
            String first = neighbors[0] == null ? "missing" : neighbors[0].toString(false);
            return "[" + first + "]";
        }
    }
    String render(V value) {
        Node root = new Node();
        Node child = new Node();
        child.vertex = value;
        root.neighbors = new NestedTextOwner.Node[] { child };
        return root.adjacentText();
    }
    String describe(V value) {
        Node node = new Node();
        node.vertex = value;
        return String.format("[%s]", node.toString(false));
    }
}
class NestedNullText { public String toString() { return null; } }
class NestedTextPeer {
    class Node {
        public String toString() { return "peer"; }
        String toString(boolean full) { return "peer"; }
    }
}
class NestedNullableStrings {
    static String nullNeighbor() {
        NestedTextOwner<NestedNullText> owner = new NestedTextOwner<NestedNullText>();
        return owner.render(new NestedNullText());
    }
    static String presentNeighbor() {
        NestedTextOwner<String> owner = new NestedTextOwner<String>();
        return owner.render("present");
    }
    static String nullDescription() {
        NestedTextOwner<NestedNullText> owner = new NestedTextOwner<NestedNullText>();
        return owner.describe(new NestedNullText());
    }
}
