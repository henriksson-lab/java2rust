public class StringEqualsObjectContracts {
    static String receiver;
    static int calls;
    static Object argument() { calls++; receiver = "new"; return "old"; }
    static boolean equals(String text, Object value) { return text.equals(value); }
    static int run() {
        if (!equals("\ud800", "\ud800") || equals("x", Integer.valueOf(3)) || equals("x", null)) return 1;
        receiver = "old";
        if (!receiver.equals(argument()) || calls != 1 || !"new".equals(receiver)) return 2;
        receiver = null;
        try { receiver.equals(argument()); return 3; }
        catch (NullPointerException expected) { if (calls != 2) return 4; }
        return 0;
    }
    public static void main(String[] args) { System.out.println(run()); }
}
