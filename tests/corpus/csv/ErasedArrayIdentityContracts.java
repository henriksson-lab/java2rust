public class ErasedArrayIdentityContracts {
    static int order;
    static Object source(Object value) { order = order * 10 + 1; return value; }
    static int index() { order = order * 10 + 2; return 0; }
    static int badIndex() { order = order * 10 + 2; return -1; }
    static String rhs() { order = order * 10 + 3; return "changed"; }
    static Object get(Object value, int index) { return ((Object[]) value)[index]; }
    static void put(Object value, int index, Object item) { ((Object[]) value)[index] = item; }
    static Object assigned(Object value, Object item) { return (((Object[]) value)[0] = item); }
    public static int run() {
        String first = new String("same");
        String second = new String("same");
        String[] values = new String[]{first, null};
        Object erased = values;
        if (get(erased, 0) != first || first != get(erased, 0)) return 1;
        if (get(erased, 0) == second || second == get(erased, 0)) return 2;
        put(erased, 0, second);
        if (get(erased, 0) != second || values[0] != second) return 3;
        if (get(erased, 1) != null) return 4;
        try { put(erased, 0, Integer.valueOf(3)); return 5; }
        catch (ArrayStoreException expected) { }
        if (values[0] != second) return 6;
        order = 0;
        try { ((Object[]) source(null))[badIndex()] = rhs(); return 7; }
        catch (NullPointerException expected) { if (order != 123) return 8; }
        order = 0;
        try { ((Object[]) source(Integer.valueOf(3)))[index()] = rhs(); return 9; }
        catch (ClassCastException expected) { if (order != 1) return 10; }
        order = 0;
        try { ((Object[]) source(erased))[badIndex()] = rhs(); return 11; }
        catch (IndexOutOfBoundsException expected) { if (order != 123) return 12; }
        order = 0;
        try { ((Object[]) source(erased))[badIndex()] += rhs(); return 13; }
        catch (IndexOutOfBoundsException expected) { if (order != 12) return 14; }
        order = 0;
        ((Object[]) source(erased))[index()] += rhs();
        if (order != 123 || !"samechanged".equals(values[0])) return 15;
        Object assigned = assigned(erased, second);
        if (assigned != get(erased, 0) || assigned != second) return 16;
        Object primitive = new int[]{1};
        try { get(primitive, 0); return 17; }
        catch (ClassCastException expected) { }
        return 0;
    }
    public static void main(String[] args) { System.out.println(run()); }
}
