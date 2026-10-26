public class EnumErasedHashAudit {
    enum Mode { FIRST, SECOND }

    public static int run() {
        Object erased = Mode.FIRST;
        if (erased.hashCode() != Mode.FIRST.hashCode()) return 1;
        Mode absent = null;
        try { absent.hashCode(); return 2; }
        catch (NullPointerException expected) { }
        return 0;
    }

    public static void main(String[] args) { System.out.println(run()); }
}
