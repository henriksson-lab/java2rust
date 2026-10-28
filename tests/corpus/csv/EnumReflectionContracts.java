public class EnumReflectionContracts {
    static int initialized;
    static int earlyNulls;
    static String lastName;
    enum Header {
        FIRST, SECOND;
        int count;
        Header() {
            initialized++;
            Header[] early = Header.class.getEnumConstants();
            if (early == null) earlyNulls++;
        }
        void increment() { count++; }
        int count() { return count; }
    }
    enum Other { VALUE }
    static Class<? extends Enum<?>> token() { return Header.class; }
    static String[] names(Class<? extends Enum<?>> type) {
        Enum<?>[] constants = type.getEnumConstants();
        String[] result = new String[constants.length];
        for (int i = 0; i < constants.length; i++) result[i] = constants[i].name();
        return result;
    }
    static Enum<?> retain(Header value) { return value; }
    static String consumeName(String value) { lastName = value; return value; }
    static String optionalName(Enum<?> value) { return consumeName(value == null ? null : value.name()); }
    static String requiredName(Enum<?> value) { return value.name(); }
    static void store(Enum<?>[] values, Enum<?> value) { values[0] = value; }
    static int run() {
        Class<? extends Enum<?>> type = token();
        if (initialized != 0) return 1;
        String[] names = names(type);
        if (initialized != 2 || earlyNulls != 2 || names.length != 2 || !names[1].equals("SECOND")) return 2;
        Enum<?>[] first = type.getEnumConstants();
        Enum<?>[] second = type.getEnumConstants();
        if (first == second || first[0] != second[0]) return 3;
        first[0] = null;
        if (second[0] == null) return 4;
        Enum<?> kept = retain(Header.FIRST);
        Header.FIRST.increment();
        if (((Header)kept).count() != 1) return 5;
        if (!kept.name().equals("FIRST") || kept.ordinal() != 0) return 6;
        if (kept.getDeclaringClass() != type || kept.getClass() != type) return 7;
        if (String.class.getEnumConstants() != null) return 8;
        try { names(null); return 9; } catch (NullPointerException expected) { }
        try { second[0] = Other.VALUE; return 10; } catch (ArrayStoreException expected) { }
        if (second[0] != kept) return 11;
        try { second[9] = Other.VALUE; return 12; } catch (ArrayIndexOutOfBoundsException expected) { }
        second[0] = Header.SECOND;
        if (second[0] != Header.SECOND) return 13;
        Header[] typed = Header.class.getEnumConstants();
        if (typed.length != 2 || typed[0] != Header.FIRST) return 14;
        Enum<?> missing = null;
        if ((Header)missing != null) return 15;
        Enum<?> other = Other.VALUE;
        try { Header wrong = (Header)other; return 16; } catch (ClassCastException expected) { }
        if (optionalName(missing) != null || !optionalName(kept).equals("FIRST") || !lastName.equals("FIRST")) return 17;
        store(second, kept);
        try { store(null, other); return 18; } catch (NullPointerException expected) { }
        try { requiredName(null); return 19; } catch (NullPointerException expected) { }
        return 0;
    }
    public static void main(String[] args) { System.out.println(run()); }
}
