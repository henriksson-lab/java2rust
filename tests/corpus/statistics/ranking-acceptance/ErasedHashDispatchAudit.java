public class ErasedHashDispatchAudit {
    static class Stateful {
        int state = 2;
        int calls;
        public int hashCode() {
            calls++;
            if (state < 0) throw new IllegalArgumentException("hash");
            return state * 31 + calls;
        }
    }
    static class Box<T> {
        T value;
        Box(T value) { this.value = value; }
        public int hashCode() { return 43; }
    }
    static abstract class Base { public int hashCode() { return 5; } }
    static class Child extends Base { public int hashCode() { return 9; } }
    static Base create() { return new Child(); }

    public static int run() {
        Stateful key = new Stateful();
        Object erased = key;
        if (erased.hashCode() != 63 || key.calls != 1) return 1;
        key.state = -1;
        try { erased.hashCode(); return 2; }
        catch (IllegalArgumentException expected) {
            if (!"hash".equals(expected.getMessage()) || key.calls != 2) return 3;
        }
        key.state = 3;
        if (erased.hashCode() != 96 || key.calls != 3) return 4;
        Object boxed = new Box<String>("payload");
        if (boxed.hashCode() != 43) return 5;
        Object polymorphic = create();
        if (polymorphic.hashCode() != 9) return 6;
        Object text = new String("abc");
        if (text.hashCode() != "abc".hashCode()) return 7;
        Object number = Integer.valueOf(5);
        if (number.hashCode() != 5) return 8;
        Object plain = new Object();
        Object alias = plain;
        if (plain.hashCode() != alias.hashCode()) return 9;
        Object absent = null;
        try { absent.hashCode(); return 10; }
        catch (NullPointerException expected) { }
        return 0;
    }
    public static void main(String[] args) { System.out.println(run()); }
}
