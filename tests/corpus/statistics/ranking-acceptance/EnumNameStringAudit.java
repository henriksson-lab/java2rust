public class EnumNameStringAudit {
    enum Mode {
        FIRST, SECOND, THIRD;
        int name(int value) { return value + 1; }
    }

    public static int run() {
        String[] names = { "FIRST", null, new String("THIRD") };
        Mode[] modes = Mode.values();
        if (!modes[0].name().equals(names[0])) return 1;
        if (modes[1].name().equals(names[1])) return 2;
        if (!modes[2].name().equals(names[2])) return 3;
        Mode absent = null;
        try { absent.name(); return 4; }
        catch (NullPointerException expected) { }
        if (modes[2].ordinal() != 2) return 5;
        modes[1] = null;
        try { modes[1].name(); return 6; }
        catch (NullPointerException expected) { }
        if (Mode.FIRST.name(3) != 4) return 7;
        String copy = new String(names[0]);
        if (!copy.equals("FIRST") || copy == names[0]) return 8;
        if (!names[0].equals("FIRST")) return 9;
        try { new String(names[1]); return 10; }
        catch (NullPointerException expected) { }
        return 0;
    }

    public static void main(String[] args) { System.out.println(run()); }
}
