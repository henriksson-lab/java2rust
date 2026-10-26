package hierarchyconstructor;
class Marks {
    static int trace;
    static double value(int digit, double value) { trace = trace * 10 + digit; return value; }
}
class First {
    protected double mean;
    protected long n;
    First(double mean, long n) { this.mean = mean; this.n = n; }
    First(First source) { mean = source.mean; n = source.n; }
    void bump() { mean += 1; }
}
class Second extends First {
    protected double sumSquared;
    Second(double sumSquared, double mean, long n) { super(mean, n); this.sumSquared = sumSquared; }
    Second(Second source) { super(source); sumSquared = source.sumSquared; }
}
class Third extends Second {
    protected double sumCubed;
    Third(double sc, double ss, double mean, long n) { super(ss, mean, n); sumCubed = sc; }
    Third(Third source) { super(source); sumCubed = source.sumCubed; }
}
class Fourth extends Third {
    protected double sumFourth;
    Fourth(double sq, double sc, double ss, double mean, long n) {
        super(Marks.value(1, sc), Marks.value(2, ss), Marks.value(3, mean), n);
        sumFourth = sq;
    }
    Fourth(double sq, Third source) { super(source); sumFourth = sq; }
}
class Holder {
    First moment;
    Holder(First moment) { this.moment = moment; }
    Fourth selected() {
        if (moment == null) return null;
        return (Fourth)moment;
    }
}
public class HierarchyConstructorAudit {
    public static int run() {
        Fourth original = new Fourth(17, 13, 11, 7, 5000000000L);
        if (Marks.trace != 123) return 1;
        if (original.mean != 7 || original.n != 5000000000L || original.sumSquared != 11 || original.sumCubed != 13 || original.sumFourth != 17) return 2;
        First root = original;
        Holder holder = new Holder(root);
        Fourth selected = holder.selected();
        if (selected != original || holder.selected() != original) return 3;
        selected.bump();
        if (original.mean != 8 || root.mean != 8) return 4;
        First firstCopy = new First(root);
        Fourth copy = new Fourth(29, (Third)root);
        if (copy.mean != 8 || copy.n != 5000000000L || copy.sumSquared != 11 || copy.sumCubed != 13 || copy.sumFourth != 29) return 5;
        original.bump();
        if (copy.mean != 8 || firstCopy.mean != 8 || original.mean != 9 || root.mean != 9) return 6;
        if (holder.selected() != original) return 7;
        Holder empty = new Holder(null);
        if (empty.selected() != null) return 8;
        if (Marks.trace != 123) return 9;
        try { new First(null); return 10; }
        catch (NullPointerException e) { }
        try { new Fourth(31, (Third)null); return 11; }
        catch (NullPointerException e) { }
        Holder wrong = new Holder(new First(3, 2));
        try { wrong.selected(); return 12; }
        catch (ClassCastException e) { }
        if (wrong.moment.mean != 3) return 13;
        try { wrong.selected(); return 14; }
        catch (ClassCastException e) { }
        return 0;
    }
    public static void main(String[] args) { System.out.println(run()); }
}
