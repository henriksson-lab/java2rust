package superimplementation;
class Root {
    int total;
    int mixedCase;
    Root(int value) { total = value; }
    Root combine(Root other) { total += other.total; return this; }
    Root identity() { return this; }
    static Root make(int value) {
        Root result = new Root(value);
        result.mixedCase = value + 1;
        return result;
    }
}
class Second extends Root {
    int second;
    Second(int value) { super(value); second = value * 10; }
    Second combine(Second other) { second += other.second; super.combine(other); return this; }
}
class Third extends Second {
    int third;
    Third(int value) { super(value); third = value * 100; }
    Third combine(Third other) {
        third += other.third;
        super.combine(other);
        if (other.total < 0) throw new IllegalArgumentException("negative");
        return this;
    }
    Root parentIdentity() { return super.identity(); }
}
class Fourth extends Third {
    Fourth(int value) { super(value); }
    Fourth combine(Fourth other) { super.combine(other); return this; }
    Third parentCombine(Third other) { return super.combine(other); }
}
public class SuperImplementationAudit {
    static int trace;
    static int mark() { trace = trace * 10 + 2; return 23; }
    static int fail() { trace = trace * 10 + 3; throw new IllegalStateException("rhs"); }
    static Root receiver(Root root) { trace = trace * 10 + 1; return root; }
    public static int run() {
        Fourth a = new Fourth(2);
        Fourth b = new Fourth(3);
        Root retained = a;
        if (a.combine(b) != a) return 1;
        if (a.total != 5 || a.second != 50 || a.third != 500 || retained.total != 5) return 2;
        Third inherited = a;
        Third c = new Third(4);
        if (inherited.combine(c) != a || a.total != 9 || a.second != 90 || a.third != 900) return 3;
        if (a.parentIdentity() != a) return 4;
        try { a.combine(new Fourth(-1)); return 5; }
        catch (IllegalArgumentException e) { }
        if (a.total != 8 || a.second != 80 || a.third != 800) return 6;
        if (a.combine(b) != a || a.total != 11 || a.second != 110 || a.third != 1100) return 7;
        Root made = Root.make(6);
        if (made.mixedCase != 7) return 8;
        receiver(made).mixedCase = mark();
        if (trace != 12 || made.mixedCase != 23) return 9;
        trace = 0;
        try { receiver(null).mixedCase = mark(); return 10; }
        catch (NullPointerException e) { }
        if (trace != 12) return 11;
        trace = 0;
        try { receiver(null).mixedCase = fail(); return 12; }
        catch (IllegalStateException e) { }
        if (trace != 13) return 13;
        Fourth parent = new Fourth(1);
        if (parent.parentCombine(new Third(2)) != parent || parent.total != 3 || parent.second != 30 || parent.third != 300) return 14;
        try { parent.parentCombine(new Third(-1)); return 15; }
        catch (IllegalArgumentException e) { }
        if (parent.total != 2 || parent.second != 20 || parent.third != 200) return 16;
        return 0;
    }
    public static void main(String[] args) { System.out.println(run()); }
}
