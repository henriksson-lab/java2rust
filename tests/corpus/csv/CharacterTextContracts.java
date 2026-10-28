public class CharacterTextContracts {
    static final String CR = Character.toString('\r');
    static String character(char value) { return Character.toString(value); }
    static String point(int value) { return Character.toString(value); }
    static int run() {
        if (!CR.equals("\r")) return 1;
        if (!character('\ud800').equals("\ud800")) return 2;
        if (!character('\udc00').equals("\udc00")) return 3;
        if (!character('\u0000').equals("\u0000")) return 4;
        if (!point(0x1f600).equals("\ud83d\ude00")) return 5;
        if (!point(0xd800).equals("\ud800")) return 6;
        if (!point(0x10ffff).equals("\udbff\udfff")) return 7;
        try { point(-1); return 8; } catch (IllegalArgumentException expected) { }
        try { point(0x110000); return 9; } catch (IllegalArgumentException expected) { }
        String[] values = new String[]{character('x'), point(0x1f600), character('\ud800')};
        if (!values[2].equals("\ud800")) return 10;
        return 0;
    }
    public static void main(String[] args) { System.out.println(run()); }
}
