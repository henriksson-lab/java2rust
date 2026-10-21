interface StaticDraw { int next(); }
class ContractTable {
    int value;
    ContractTable(int initial) { value = initial; }
    int advance() { return ++value; }
}
class ContractParent implements StaticDraw {
    private static final ContractTable TABLE = new ContractTable(10);
    public int next() { return TABLE.advance(); }
}
class ContractChildTable {
    int value = 100;
    int advance() { return ++value; }
}
class ContractChild extends ContractParent {
    private static final ContractChildTable TABLE = new ContractChildTable();
    public int next() { return super.next() + TABLE.advance(); }
}
class ContractBuffers {
    protected void fill(int[] state, int[] seed) {
        System.arraycopy(seed, 0, state, 0, Math.min(state.length, seed.length));
        if (state.length != 0) state[0] += 1;
    }
    protected void fill(long[] state, long[] seed) {
        System.arraycopy(seed, 0, state, 0, Math.min(state.length, seed.length));
        if (state.length != 0) state[0] += 2;
    }
}
class ContractBufferChild extends ContractBuffers {
    int[] stored;
    long[] storedLong;
    int run() {
        int[] tmp = new int[3];
        stored = tmp;
        int[] seed = new int[] {4, 5};
        fill(tmp, seed);
        if (stored[0] != 5 || tmp[1] != 5 || seed[0] != 4) return 1;
        long[] other = new long[3];
        storedLong = other;
        long[] seedLong = new long[] {7, 8};
        fill(other, seedLong);
        if (storedLong[0] != 9 || other[1] != 8 || seedLong[0] != 7) return 2;
        return 0;
    }
}
class ContractCtorRoot {
    final int size;
    ContractCtorRoot(int size) { this.size = size; }
}
class ContractCtorMiddle extends ContractCtorRoot {
    private static final int WIDTH = 3;
    private static int initializations = 0;
    private static final int LIVE = initialize();
    static int initialize() { return ++initializations; }
    ContractCtorMiddle() { super(WIDTH); }
    ContractCtorMiddle(int ignored) { super(LIVE); }
}
class ContractCtorLeaf extends ContractCtorMiddle {
    private static final int WIDTH = 9;
    private static final int LIVE = 99;
    ContractCtorLeaf() { super(); }
    ContractCtorLeaf(int ignored) { super(ignored); }
}
interface ContractStatefulDraw {
    int next();
    int twice();
}
abstract class ContractStatefulBase implements ContractStatefulDraw {
    protected int state;
    ContractStatefulBase(int seed) { initialize(seed); }
    private void initialize(int seed) { this.state = seed; }
    public abstract int next();
    public int twice() { return next() + next(); }
    static class Table {
        int value;
        Table(int value) { this.value = value; }
        int read() { return value; }
    }
}
class ContractStatefulLeaf extends ContractStatefulBase {
    ContractStatefulLeaf(int seed) { super(seed); }
    public int next() { return state++; }
}
class ContractStatefulOther extends ContractStatefulBase {
    ContractStatefulOther(int seed) { super(seed); }
    public int next() { return --state; }
}
class SourceContractBoundaries {
    static int narrowed(long value, long divisor) { return (int) (value % divisor); }
    static int absolute(long value, long divisor) { return (int) (Math.abs(value) % divisor); }
    static int divided(long value, long divisor) { return (int) (short) (value / divisor); }
    static int negated(int value) { return -value; }
    static long negatedLong(long value) { return -value; }
    static long promotedConditional(int value) { return value < 0 ? -value : value; }
    static int negatedByte(byte value) { return -value; }
    static int positiveByte(byte value) { return +value; }
    static int negatedChar(char value) { return -value; }
    static int negatedDivision(int value, int divisor) { return -(value / divisor); }
    static int run() {
        ContractStatefulDraw first = new ContractStatefulLeaf(7);
        ContractStatefulDraw second = new ContractStatefulOther(7);
        if (first.twice() != 15 || second.twice() != 11) return 26;
        ContractStatefulBase.Table table = new ContractStatefulBase.Table(4);
        if (table.read() != 4) return 27;
        Object erasedTable = table;
        if (erasedTable.toString().length() == 0) return 28;
        if (new ContractCtorLeaf().size != 3 || new ContractCtorLeaf(7).size != 1) return 25;
        if (negated(Integer.MIN_VALUE) != Integer.MIN_VALUE
                || negatedLong(Long.MIN_VALUE) != Long.MIN_VALUE) return 20;
        if (promotedConditional(Integer.MIN_VALUE) != -2147483648L) return 21;
        if (negatedByte((byte) -128) != 128 || positiveByte((byte) -128) != -128
                || negatedChar('a') != -97) return 22;
        if (negatedDivision(Integer.MIN_VALUE, -1) != Integer.MIN_VALUE) return 23;
        try { negatedDivision(1, 0); return 24; } catch (ArithmeticException error) {}
        if (narrowed(Long.MIN_VALUE, -1) != 0 || narrowed(-100, 9) != -1) return 10;
        if (absolute(-100, 9) != 1 || divided(65538, 2) != -32767) return 11;
        try { narrowed(5, 0); return 12; } catch (ArithmeticException error) {}
        try { divided(5, 0); return 13; } catch (ArithmeticException error) {}
        if (new ContractBufferChild().run() != 0) return 14;
        ContractChild child = new ContractChild();
        StaticDraw view = child;
        if (child.next() != 112 || view.next() != 114) return 15;
        if (new ContractParent().next() != 13) return 16;
        return 0;
    }
}
