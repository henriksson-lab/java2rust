class LexicalRoot {
    int calls;
    int step() { calls++; return calls; }
    int read(LexicalRoot other) { return other.calls; }
    LexicalRoot self() { return this; }
}
class LexicalMiddle extends LexicalRoot {
    @Override int step() { return super.step() + 10; }
    @Override int read(LexicalRoot other) { return super.read(other); }
}
class LexicalLeaf extends LexicalMiddle { }
class AliasedLexicalSuper {
    static int run() {
        LexicalMiddle middle = new LexicalMiddle();
        LexicalMiddle alias = middle;
        if (middle.step() != 11 || alias.calls != 1) return 1;
        if (middle.read(middle) != 1) return 2;
        if (middle.self() != middle) return 3;
        LexicalLeaf leaf = new LexicalLeaf();
        if (leaf.step() != 11 || leaf.calls != 1) return 4;
        if (leaf.read(leaf) != 1) return 5;
        return 0;
    }
    public static void main(String[] args) { System.out.println(run()); }
}
