public class R1 {
    static class Node { int v; }
    public boolean distinct() { Node a = new Node(); Node b = new Node(); return a == b; }
    public boolean sameRef()  { Node a = new Node(); Node c = a;          return a == c; }
    public boolean neq()      { Node a = new Node(); Node b = new Node(); return a != b; }
}
