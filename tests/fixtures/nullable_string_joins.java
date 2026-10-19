class JoinNode<V> {
    V vertex;
    JoinNode<V>[] neighbors;
    JoinNode<V> parent;
    public String toString() { return vertex.toString(); }
    String toString(boolean full) { return full ? toString() : vertex.toString(); }
    String neighbor() {
        String value = neighbors[0] == null ? "missing" : neighbors[0].toString(false);
        return "[" + value + "]";
    }
    String parentText() { return "[" + (parent == null ? "missing" : parent.toString(false)) + "]"; }
}
class JoinNullText { public String toString() { return null; } }
class NullableStringJoins {
    static JoinNode<JoinNullText> nullNode() {
        JoinNode<JoinNullText> node = new JoinNode<JoinNullText>();
        node.vertex = new JoinNullText();
        return node;
    }
    static String neighborNullResult() {
        JoinNode<JoinNullText> node = new JoinNode<JoinNullText>();
        node.neighbors = new JoinNode[] { nullNode() };
        return node.neighbor();
    }
    static String neighborAbsent() {
        JoinNode<String> node = new JoinNode<String>();
        node.neighbors = new JoinNode[] { null };
        return node.neighbor();
    }
    static String parentNullResult() {
        JoinNode<JoinNullText> node = new JoinNode<JoinNullText>();
        node.parent = nullNode();
        return node.parentText();
    }
}
