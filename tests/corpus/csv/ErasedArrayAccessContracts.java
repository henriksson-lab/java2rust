public class ErasedArrayAccessContracts {
    static int order;
    static Object held;
    static Object replacement;
    static Object source(Object value) { order = order * 10 + 1; return value; }
    static int index() { order = order * 10 + 2; return 0; }
    static int badIndex() { order = order * 10 + 2; return 9; }
    static String rhs() { order = order * 10 + 3; return "changed"; }
    static String rebound() { held = replacement; return "original"; }
    static Object get(Object value, int index) { return ((Object[]) value)[index]; }
    static void put(Object value, int index, Object item) { ((Object[]) value)[index] = item; }
    static Object assigned(Object value, Object item) { return (((Object[]) value)[0] = item); }
    static class Text {
        public String toString() { ((Object[]) held)[1] = "callback"; return "text"; }
    }
    public static int run() {
        String[] values = new String[]{"before", null};
        Object erased = values;
        put(erased, 0, "after");
        if (!"after".equals(get(erased, 0)) || get(erased, 1) != null) return 1;
        if (get(erased, 0) != values[0]) return 2;
        try { put(erased, 0, Integer.valueOf(3)); return 3; }
        catch (ArrayStoreException expected) { }
        if (!"after".equals(values[0])) return 4;
        try { put(erased, 9, Integer.valueOf(3)); return 5; }
        catch (IndexOutOfBoundsException expected) { }
        order = 0;
        try { ((Object[]) source(null))[badIndex()] = rhs(); return 6; }
        catch (NullPointerException expected) { if (order != 123) return 7; }
        order = 0;
        try { ((Object[]) source(Integer.valueOf(3)))[index()] = rhs(); return 8; }
        catch (ClassCastException expected) { if (order != 1) return 9; }
        order = 0;
        try { ((Object[]) source(erased))[badIndex()] = rhs(); return 10; }
        catch (IndexOutOfBoundsException expected) { if (order != 123) return 11; }
        order = 0;
        try { ((Object[]) source(erased))[badIndex()] += rhs(); return 12; }
        catch (IndexOutOfBoundsException expected) { if (order != 12) return 13; }
        order = 0;
        ((Object[]) source(erased))[index()] += rhs();
        if (order != 123 || !"afterchanged".equals(values[0])) return 14;
        Object assigned = assigned(erased, "assigned");
        if (assigned != get(erased, 0)) return 15;
        String[] other = new String[]{"other"};
        held = erased;
        replacement = other;
        ((Object[]) held)[0] = rebound();
        if (!"original".equals(values[0]) || !"other".equals(other[0])) return 16;
        Object primitive = new int[]{1};
        try { get(primitive, 0); return 17; }
        catch (ClassCastException expected) { }
        Object[] callbacks = new Object[]{new Text(), null};
        held = callbacks;
        ((Object[]) held)[0] += "!";
        if (!"text!".equals(callbacks[0]) || !"callback".equals(callbacks[1])) return 18;
        return 0;
    }
    public static void main(String[] args) { System.out.println(run()); }
}
