package audit.enums;

public class EnumObjectAudit {
    enum Mode { FIRST, SECOND }
    enum Other { FIRST }
    enum Special { PLAIN, SPECIAL {} }
    static int calls;
    static Object argument() { calls++; return Mode.FIRST; }

    public static int run() {
        Mode first = Mode.FIRST;
        Object erased = first;
        Object another = Mode.FIRST;
        if (!first.equals(erased) || !erased.equals(first)) return 1;
        if (first.equals(Other.FIRST) || first.equals(Mode.SECOND) || first.equals(null)) return 2;
        if (erased != another) return 3;
        if (((Mode) erased) != first) return 4;
        if (first.hashCode() != Mode.values()[0].hashCode()) return 5;
        if (!first.getDeclaringClass().getName().equals("audit.enums.EnumObjectAudit$Mode")) return 6;
        if (!first.getClass().getName().equals("audit.enums.EnumObjectAudit$Mode")) return 7;
        if (first.getDeclaringClass() != Mode.class || first.getClass() != Mode.class) return 8;
        if (!erased.getClass().getName().equals("audit.enums.EnumObjectAudit$Mode")) return 9;
        Mode absent = null;
        calls = 0;
        try { absent.equals(argument()); return 10; }
        catch (NullPointerException expected) { if (calls != 1) return 11; }
        if (Special.SPECIAL.getDeclaringClass() != Special.class) return 12;
        if (!Special.SPECIAL.getClass().getName().equals("audit.enums.EnumObjectAudit$Special$1")) return 13;
        Object special = Special.SPECIAL;
        if (!special.getClass().getName().equals("audit.enums.EnumObjectAudit$Special$1")) return 14;
        if (!first.toString().equals("FIRST") || !special.toString().equals("SPECIAL")) return 15;
        Mode[] modes = { Mode.FIRST, null };
        if (!modes[0].toString().equals("FIRST")) return 16;
        try { modes[1].toString(); return 17; }
        catch (NullPointerException expected) { }
        return 0;
    }

    public static void main(String[] args) { System.out.println(run()); }
}
