package anonymousprimitivedefaults;

public class AnonymousPrimitiveDefaultsProbe {
    interface Counter {
        int next();
        boolean initial();
    }
    static abstract class Base {
        abstract int next();
        abstract int count();
        abstract long wide();
        abstract double fraction();
        abstract float small();
        abstract boolean flag();
        abstract char unit();
        abstract byte octet();
        abstract short word();
    }
    static Counter counter() {
        return new Counter() {
            int n;
            long wide;
            double fraction;
            float small;
            boolean flag;
            char unit;
            byte octet;
            short word;
            public int next() { this.n = this.n + 1; return this.n; }
            public boolean initial() {
                return n == 0 && wide == 0L && fraction == 0.0 && small == 0.0f
                    && !flag && unit == 0 && octet == 0 && word == 0;
            }
        };
    }
    static Base base() {
        return new Base() {
            int n;
            long wide;
            double fraction;
            float small;
            boolean flag;
            char unit;
            byte octet;
            short word;
            public int next() { this.n = this.n + 1; return this.n; }
            public int count() { return this.n; }
            public long wide() { return this.wide; }
            public double fraction() { return this.fraction; }
            public float small() { return this.small; }
            public boolean flag() { return this.flag; }
            public char unit() { return this.unit; }
            public byte octet() { return this.octet; }
            public short word() { return this.word; }
        };
    }
    public static int run() {
        Counter first = counter();
        Counter alias = first;
        Counter separate = counter();
        if (!first.initial() || !separate.initial()) return 1;
        if (first.next() != 1 || alias.next() != 2 || first.initial()) return 2;
        if (!separate.initial() || separate.next() != 1) return 3;
        Base original = base();
        Base copy = original;
        Base independent = base();
        if (original.count() != 0 || independent.count() != 0) return 4;
        if (original.next() != 1 || copy.next() != 2 || original.count() != 2) return 5;
        if (independent.count() != 0 || independent.next() != 1) return 6;
        if (original.wide() != 0L || original.fraction() != 0.0 || original.small() != 0.0f
            || original.flag() || original.unit() != 0 || original.octet() != 0 || original.word() != 0) return 7;
        return 0;
    }
    public static void main(String[] args) {
        int result = run();
        if (result != 0) throw new AssertionError("anonymous defaults: " + result);
        System.out.println(result);
    }
}
