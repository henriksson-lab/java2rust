package erasedclasses;

public class ErasedClassProbe {
    static abstract class Base {
        int value;
        Base(int value) { this.value = value; }
        void change(int next) { value = next; }
        public boolean equals(Object other) {
            return other != null && getClass().equals(other.getClass())
                && value == ((Base)other).value;
        }
    }
    static final class Left extends Base { Left(int value) { super(value); } }
    static final class Right extends Base { Right(int value) { super(value); } }
    static abstract class Frozen {
        final int value;
        Frozen(int value) { this.value = value; }
        int read() { return value; }
    }
    static abstract class Middle extends Frozen { Middle(int value) { super(value); } }
    static final class FrozenLeaf extends Middle { FrozenLeaf(int value) { super(value); } }
    static Object erase(Base value) { return value; }
    static String kind(Object value) { return value.getClass().getName(); }
    public static int run() {
        Left a = new Left(4);
        Base base = a;
        Object erased = erase(base);
        if (!a.equals(a) || !a.equals(new Left(4))) return 1;
        if (a.equals(new Right(4)) || a.equals(new Left(5)) || a.equals(null) || a.equals("foreign")) return 2;
        if (!kind(a).equals("erasedclasses.ErasedClassProbe$Left")) return 3;
        if (!kind(erased).equals(kind(a)) || !kind(new Right(4)).equals("erasedclasses.ErasedClassProbe$Right")) return 4;
        if (!base.equals(new Left(4))) return 5;
        base.change(7);
        if (!a.equals(new Left(7)) || a.equals(new Left(4))) return 6;
        if (!kind(erased).equals("erasedclasses.ErasedClassProbe$Left")) return 7;
        if (!kind("text").equals("java.lang.String") || !kind(new Object()).equals("java.lang.Object")) return 8;
        if (a.equals(new double[]{4}) || !kind(new double[]{4}).equals("[D")) return 9;
        try { kind(null); return 10; } catch (NullPointerException expected) { }
        if (!kind(new double[]{}).equals("[D") || !kind(new long[]{4}).equals("[J")) return 11;
        FrozenLeaf leaf = new FrozenLeaf(9);
        Object frozen = leaf;
        if (!(frozen instanceof Frozen) || !(frozen instanceof Middle)) return 12;
        Frozen recovered = (Frozen)frozen;
        if (recovered.read() != 9 || recovered != leaf) return 13;
        Object erasedRoot = recovered;
        Middle middle = (Middle)erasedRoot;
        if (middle.read() != 9 || middle != leaf) return 14;
        return 0;
    }
    public static void main(String[] args) { System.out.println(run()); }
}
